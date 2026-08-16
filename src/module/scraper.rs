//! Browser-only Notion scrape. Page URLs live in `notion.json`.
//!
//! H2 headings are tags. Pages nested under those headings (including
//! inside toggle / collapsible blocks) are the blog content pages.

use crate::module::config::{page_id_from_url, NotionConfig, CONFIG_PATH, EMBEDDED_CONFIG};
use serde_json::{Map, Value};
use std::cell::RefCell;
use std::collections::HashSet;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestCache, RequestInit, RequestMode, Response};

const NOTION_LIVE_API: &str = "https://notion-api.splitbee.io/v1/page/";
const H2_TYPES: &[&str] = &["header", "sub_header"];
const CONTAINER_TYPES: &[&str] = &[
    "toggle",
    "column_list",
    "column",
    "bulleted_list",
    "numbered_list",
    "to_do",
    "transclusion_container",
];
const MAX_CHILD_FETCHES: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentPage {
    pub title: String,
    pub href: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagSection {
    pub tag: String,
    pub pages: Vec<ContentPage>,
}

thread_local! {
    static CATALOG: RefCell<Vec<TagSection>> = const { RefCell::new(Vec::new()) };
}

pub fn current_tags() -> Vec<String> {
    CATALOG.with(|catalog| {
        catalog
            .borrow()
            .iter()
            .map(|section| section.tag.clone())
            .collect()
    })
}

pub fn current_posts(slug: Option<&str>) -> Vec<ContentPage> {
    CATALOG.with(|catalog| {
        let catalog = catalog.borrow();
        match slug {
            None => catalog
                .iter()
                .flat_map(|section| {
                    section.pages.iter().map(|page| ContentPage {
                        title: format!("{} — {}", section.tag, page.title),
                        href: page.href.clone(),
                    })
                })
                .collect(),
            Some(slug) => catalog
                .iter()
                .filter(|section| tag_slug(&section.tag) == slug)
                .flat_map(|section| section.pages.iter().cloned())
                .collect(),
        }
    })
}

pub fn tag_slug(tag: &str) -> String {
    tag.replace('/', "-")
}

pub fn start_fetch() {
    wasm_bindgen_futures::spawn_local(async {
        match fetch_catalog().await {
            Ok(catalog) => {
                log::info!(
                    "fetched {} tags / {} posts from Notion",
                    catalog.len(),
                    catalog
                        .iter()
                        .map(|section| section.pages.len())
                        .sum::<usize>()
                );
                CATALOG.with(|slot| *slot.borrow_mut() = catalog);
            }
            Err(err) => log::error!("notion catalog fetch failed: {err}"),
        }
    });
}

async fn fetch_catalog() -> Result<Vec<TagSection>, String> {
    let config = load_config().await?;
    let mut catalog = Vec::new();
    let mut any_tag_page = false;

    for page in config.tag_pages() {
        any_tag_page = true;
        let page_id = page_id_from_url(&page.url)
            .ok_or_else(|| format!("could not parse Notion page id from {}", page.url))?;
        let mut blocks = fetch_blocks(&page_id).await?;
        resolve_missing(&mut blocks, &page_id).await;
        catalog.extend(extract_catalog_from_blocks(
            &Value::Object(blocks),
            &page_id,
            &site_origin(&page.url),
        ));
    }

    if !any_tag_page {
        log::warn!("notion.json has no pages with role \"tags\"");
    }

    Ok(dedupe_sections(catalog))
}

async fn load_config() -> Result<NotionConfig, String> {
    match fetch_text(CONFIG_PATH).await {
        Ok(text) => match NotionConfig::parse(&text) {
            Ok(config) => Ok(config),
            Err(err) => {
                log::warn!("falling back to embedded notion.json ({err})");
                NotionConfig::parse(EMBEDDED_CONFIG)
            }
        },
        Err(err) => {
            log::warn!("falling back to embedded {CONFIG_PATH} ({err})");
            NotionConfig::parse(EMBEDDED_CONFIG)
        }
    }
}

async fn resolve_missing(blocks: &mut Map<String, Value>, root_id: &str) {
    let mut fetched = 0;
    loop {
        if fetched >= MAX_CHILD_FETCHES {
            break;
        }
        let missing = missing_content_ids(blocks, root_id);
        if missing.is_empty() {
            break;
        }
        let Some(id) = missing.into_iter().next() else {
            break;
        };
        match fetch_blocks(&id).await {
            Ok(extra) => {
                for (key, value) in extra {
                    blocks.entry(key).or_insert(value);
                }
            }
            Err(err) => {
                log::warn!("skipping nested Notion block {id}: {err}");
                blocks.insert(id, Value::Null);
            }
        }
        fetched += 1;
    }
}

async fn fetch_blocks(page_id: &str) -> Result<Map<String, Value>, String> {
    let json = fetch_page_json(page_id).await?;
    let root: Value =
        serde_json::from_str(&json).map_err(|err| format!("invalid notion json: {err}"))?;
    block_map_owned(root).ok_or_else(|| "notion response is missing a block map".to_string())
}

async fn fetch_page_json(page_id: &str) -> Result<String, String> {
    let compact_id = page_id.replace('-', "");
    let cache_bust = js_sys::Date::now() as u64;
    let url = format!("{NOTION_LIVE_API}{compact_id}?t={cache_bust}");
    fetch_text(&url).await
}

async fn fetch_text(url: &str) -> Result<String, String> {
    let opts = RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(RequestMode::Cors);
    opts.set_cache(RequestCache::NoStore);

    let request = Request::new_with_str_and_init(url, &opts).map_err(js_err)?;
    let window = web_sys::window().ok_or_else(|| "missing window".to_string())?;
    let response = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(js_err)?;
    let response: Response = response.dyn_into().map_err(js_err)?;
    if !response.ok() {
        return Err(format!("http {} from {url}", response.status()));
    }
    let text = JsFuture::from(response.text().map_err(js_err)?)
        .await
        .map_err(js_err)?;
    text.as_string()
        .ok_or_else(|| "response was not text".to_string())
}

fn js_err(err: wasm_bindgen::JsValue) -> String {
    err.as_string().unwrap_or_else(|| format!("{err:?}"))
}

#[cfg(test)]
pub fn extract_h2_titles(json: &str, page_id: &str) -> Vec<String> {
    extract_catalog(json, page_id)
        .into_iter()
        .map(|section| section.tag)
        .collect()
}

#[cfg(test)]
pub fn extract_catalog(json: &str, page_id: &str) -> Vec<TagSection> {
    let Ok(root) = serde_json::from_str::<Value>(json) else {
        log::error!("notion response is not valid JSON");
        return Vec::new();
    };
    extract_catalog_from_blocks(&root, page_id, "https://limdongju.notion.site")
}

fn extract_catalog_from_blocks(root: &Value, page_id: &str, origin: &str) -> Vec<TagSection> {
    let blocks = block_map(root);
    if !blocks.is_object() {
        log::error!("notion response is missing a block map");
        return Vec::new();
    }

    let mut sections = Vec::new();
    for child_id in page_child_ids(blocks, page_id) {
        let value = block_value(&blocks[&child_id]);
        let Some(kind) = value["type"].as_str() else {
            continue;
        };
        if !H2_TYPES.contains(&kind) {
            continue;
        }
        let tag = rich_text(&value["properties"]["title"]);
        if tag.is_empty() {
            continue;
        }
        let mut pages = Vec::new();
        collect_pages(
            blocks,
            &child_content_ids(value),
            page_id,
            origin,
            &mut pages,
        );
        sections.push(TagSection { tag, pages });
    }
    sections
}

fn collect_pages(
    blocks: &Value,
    ids: &[String],
    root_id: &str,
    origin: &str,
    out: &mut Vec<ContentPage>,
) {
    for id in ids {
        if id == root_id {
            continue;
        }
        let value = block_value(&blocks[id]);
        let Some(kind) = value["type"].as_str() else {
            continue;
        };
        if kind == "page" {
            let title = rich_text(&value["properties"]["title"]);
            if !title.is_empty() {
                out.push(ContentPage {
                    title,
                    href: format!("{origin}/{}", id.replace('-', "")),
                });
            }
            continue;
        }
        if kind == "toggle" || CONTAINER_TYPES.contains(&kind) || H2_TYPES.contains(&kind) {
            collect_pages(blocks, &child_content_ids(value), root_id, origin, out);
        }
    }
}

fn missing_content_ids(blocks: &Map<String, Value>, root_id: &str) -> Vec<String> {
    let root = Value::Object(blocks.clone());
    let mut missing = Vec::new();
    let mut seen = HashSet::new();
    let mut stack = page_child_ids(&root, root_id);
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        match blocks.get(&id) {
            None => missing.push(id),
            Some(Value::Null) => {}
            Some(block) => {
                let value = block_value(block);
                let kind = value["type"].as_str().unwrap_or("");
                if kind == "page" && id != root_id {
                    continue;
                }
                if H2_TYPES.contains(&kind) || kind == "toggle" || CONTAINER_TYPES.contains(&kind) {
                    stack.extend(child_content_ids(value));
                }
            }
        }
    }
    missing
}

fn child_content_ids(value: &Value) -> Vec<String> {
    value["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|id| id.as_str().map(str::to_owned))
        .collect()
}

fn block_map(root: &Value) -> &Value {
    let nested = &root["recordMap"]["block"];
    if nested.is_object() {
        nested
    } else {
        root
    }
}

fn block_map_owned(root: Value) -> Option<Map<String, Value>> {
    match root {
        Value::Object(map) => {
            if let Some(Value::Object(record_map)) = map.get("recordMap").cloned() {
                if let Some(Value::Object(block)) = record_map.get("block").cloned() {
                    return Some(block);
                }
            }
            Some(map)
        }
        _ => None,
    }
}

fn page_child_ids(blocks: &Value, page_id: &str) -> Vec<String> {
    let page = block_value(&blocks[page_id]);
    if let Some(ids) = page["content"].as_array() {
        return ids
            .iter()
            .filter_map(|id| id.as_str().map(str::to_owned))
            .collect();
    }

    blocks
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(id, block)| {
            let value = block_value(block);
            let kind = value["type"].as_str()?;
            (H2_TYPES.contains(&kind) && id != page_id).then(|| id.clone())
        })
        .collect()
}

fn block_value(block: &Value) -> &Value {
    if block.is_null() {
        return block;
    }
    let value = &block["value"];
    if value.get("type").is_some() {
        value
    } else if value
        .get("value")
        .and_then(|inner| inner.get("type"))
        .is_some()
    {
        &value["value"]
    } else {
        value
    }
}

fn rich_text(title: &Value) -> String {
    let Some(parts) = title.as_array() else {
        return String::new();
    };
    parts
        .iter()
        .filter_map(|part| part.get(0).and_then(Value::as_str))
        .collect()
}

fn site_origin(url: &str) -> String {
    let rest = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let host = rest.split('/').next().unwrap_or("limdongju.notion.site");
    format!("https://{host}")
}

fn dedupe_sections(sections: Vec<TagSection>) -> Vec<TagSection> {
    let mut seen_tags = HashSet::new();
    let mut out: Vec<TagSection> = Vec::new();
    for mut section in sections {
        if !seen_tags.insert(section.tag.clone()) {
            if let Some(existing) = out.iter_mut().find(|item| item.tag == section.tag) {
                existing.pages.append(&mut section.pages);
            }
            continue;
        }
        out.push(section);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{extract_catalog, extract_h2_titles};

    const PAGE_ID: &str = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa0";

    fn tree_fixture() -> String {
        format!(
            r#"{{
              "recordMap": {{
                "block": {{
                  "{PAGE_ID}": {{
                    "value": {{
                      "value": {{
                        "type": "page",
                        "content": [
                          "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa1",
                          "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa2"
                        ]
                      }}
                    }}
                  }},
                  "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa1": {{
                    "value": {{
                      "value": {{
                        "type": "header",
                        "properties": {{ "title": [["TEST1"]] }},
                        "content": ["aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa3"]
                      }}
                    }}
                  }},
                  "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa2": {{
                    "value": {{
                      "value": {{
                        "type": "header",
                        "properties": {{ "title": [["TEST2"]] }},
                        "content": ["aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa4"]
                      }}
                    }}
                  }},
                  "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa3": {{
                    "value": {{
                      "value": {{
                        "type": "toggle",
                        "properties": {{ "title": [["notes"]] }},
                        "content": ["aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa5"]
                      }}
                    }}
                  }},
                  "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa4": {{
                    "value": {{
                      "value": {{
                        "type": "page",
                        "properties": {{ "title": [["Direct page"]] }}
                      }}
                    }}
                  }},
                  "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa5": {{
                    "value": {{
                      "value": {{
                        "type": "page",
                        "properties": {{ "title": [["TEST-contents"]] }}
                      }}
                    }}
                  }}
                }}
              }}
            }}"#
        )
    }

    #[test]
    fn extracts_pages_under_toggles_and_headings() {
        let catalog = extract_catalog(&tree_fixture(), PAGE_ID);
        assert_eq!(catalog.len(), 2);
        assert_eq!(catalog[0].tag, "TEST1");
        assert_eq!(catalog[0].pages[0].title, "TEST-contents");
        assert!(catalog[0].pages[0]
            .href
            .ends_with(&"aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa5".replace('-', "")));
        assert_eq!(catalog[1].tag, "TEST2");
        assert_eq!(catalog[1].pages[0].title, "Direct page");
    }

    #[test]
    fn extracts_header_titles_as_tags() {
        assert_eq!(
            extract_h2_titles(&tree_fixture(), PAGE_ID),
            ["TEST1", "TEST2"]
        );
    }

    #[test]
    fn ignores_invalid_json() {
        assert!(extract_h2_titles("not-json", PAGE_ID).is_empty());
    }
}
