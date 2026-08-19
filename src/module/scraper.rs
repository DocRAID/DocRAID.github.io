//! Browser-only Notion scrape. Page URLs live in `notion.json`.
//!
//! H2 headings are tags. Pages nested under those headings (including
//! inside toggle / collapsible blocks) are the blog content pages.

use crate::module::config::{dashed_id, page_id_from_url, NotionConfig, EMBEDDED_CONFIG};
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentPage {
    pub title: String,
    pub id: String,
    pub href: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagSection {
    pub tag: String,
    pub pages: Vec<ContentPage>,
}

/// A run of post body content. Whitespace inside each string is kept as-is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PostSegment {
    Text(String),
    Code(String),
}

const CACHE_KEY: &str = "tui_blog.catalog.v3";
const REVALIDATE_MS: i32 = 45_000;

thread_local! {
    static CATALOG: RefCell<Vec<TagSection>> = const { RefCell::new(Vec::new()) };
    static REVALIDATE_BUSY: RefCell<bool> = const { RefCell::new(false) };
    static POLL_STARTED: RefCell<bool> = const { RefCell::new(false) };
    static POST_BODIES: RefCell<HashMap<String, Vec<PostSegment>>> = RefCell::new(HashMap::new());
    static POST_FETCHING: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
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
                        id: page.id.clone(),
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

/// Every content page with its parent tag, titles left as scraped.
pub fn current_tagged_posts() -> Vec<(String, ContentPage)> {
    CATALOG.with(|catalog| {
        catalog
            .borrow()
            .iter()
            .flat_map(|section| {
                section
                    .pages
                    .iter()
                    .cloned()
                    .map(|page| (section.tag.clone(), page))
            })
            .collect()
    })
}

pub fn tag_slug(tag: &str) -> String {
    tag.replace('/', "-")
}

pub fn same_page_id(left: &str, right: &str) -> bool {
    left.replace('-', "") == right.replace('-', "")
}

/// Kick off a scrape of one Notion post. Safe to call every frame.
pub fn request_post(page_id: &str) {
    let id = page_id.replace('-', "");
    if id.is_empty() {
        return;
    }
    let already = POST_BODIES.with(|bodies| bodies.borrow().contains_key(&id))
        || POST_FETCHING.with(|busy| !busy.borrow_mut().insert(id.clone()));
    if already {
        return;
    }
    wasm_bindgen_futures::spawn_local(async move {
        let result = fetch_post_plain(&id).await;
        POST_FETCHING.with(|busy| {
            busy.borrow_mut().remove(&id);
        });
        match result {
            Ok(segments) => {
                POST_BODIES.with(|bodies| {
                    bodies.borrow_mut().insert(id, segments);
                });
            }
            Err(err) => {
                log::error!("post scrape failed: {err}");
                POST_BODIES.with(|bodies| {
                    bodies.borrow_mut().insert(
                        id,
                        vec![PostSegment::Text(format!("(failed to load)\n{err}"))],
                    );
                });
            }
        }
    });
}

pub fn current_post_segments(page_id: &str) -> Option<Vec<PostSegment>> {
    let id = page_id.replace('-', "");
    POST_BODIES.with(|bodies| bodies.borrow().get(&id).cloned())
}

pub fn start_fetch() {
    if catalog_is_empty() {
        if let Some(cached) = load_session_cache() {
            log::info!("showing cached notion catalog ({} tags)", cached.len());
            apply_catalog(cached);
        }
    }
    spawn_revalidate();
    ensure_poller();
}

fn catalog_is_empty() -> bool {
    CATALOG.with(|catalog| catalog.borrow().is_empty())
}

fn apply_catalog(catalog: Vec<TagSection>) {
    CATALOG.with(|slot| *slot.borrow_mut() = catalog);
}

#[cfg(test)]
pub fn set_catalog_for_tests(catalog: Vec<TagSection>) {
    apply_catalog(catalog);
}

fn spawn_revalidate() {
    if REVALIDATE_BUSY.with(|busy| {
        if *busy.borrow() {
            true
        } else {
            *busy.borrow_mut() = true;
            false
        }
    }) {
        return;
    }
    wasm_bindgen_futures::spawn_local(async {
        let result = fetch_catalog().await;
        REVALIDATE_BUSY.with(|busy| *busy.borrow_mut() = false);
        match result {
            Ok(catalog) => {
                let changed = CATALOG.with(|slot| *slot.borrow() != catalog);
                if changed {
                    log::info!(
                        "notion catalog updated ({} tags / {} posts)",
                        catalog.len(),
                        catalog
                            .iter()
                            .map(|section| section.pages.len())
                            .sum::<usize>()
                    );
                    save_session_cache(&catalog);
                    apply_catalog(catalog);
                } else {
                    log::info!("notion catalog unchanged");
                    save_session_cache(&catalog);
                }
            }
            Err(err) => log::error!("notion catalog fetch failed: {err}"),
        }
    });
}

fn ensure_poller() {
    if POLL_STARTED.with(|started| {
        if *started.borrow() {
            true
        } else {
            *started.borrow_mut() = true;
            false
        }
    }) {
        return;
    }
    let Some(window) = web_sys::window() else {
        return;
    };

    let on_tick = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(spawn_revalidate);
    if let Err(err) = window.set_interval_with_callback_and_timeout_and_arguments_0(
        on_tick.as_ref().unchecked_ref(),
        REVALIDATE_MS,
    ) {
        log::error!("failed to start notion poller: {err:?}");
    }
    on_tick.forget();

    if let Some(document) = window.document() {
        let on_visible = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::new(
            move |_event: web_sys::Event| {
                if document_is_visible() {
                    spawn_revalidate();
                }
            },
        );
        if let Err(err) = document.add_event_listener_with_callback(
            "visibilitychange",
            on_visible.as_ref().unchecked_ref(),
        ) {
            log::error!("failed to listen for visibilitychange: {err:?}");
        }
        on_visible.forget();
    }
}

fn document_is_visible() -> bool {
    web_sys::window()
        .and_then(|window| window.document())
        .map(|document| document.hidden())
        .map(|hidden| !hidden)
        .unwrap_or(true)
}

async fn fetch_catalog() -> Result<Vec<TagSection>, String> {
    let config = load_config()?;
    let sources: Vec<_> = config.tag_pages().cloned().collect();
    if sources.is_empty() {
        log::warn!("notion.json has no pages with role \"tags\"");
        return Ok(Vec::new());
    }

    let jobs = sources.into_iter().map(|page| async move {
        let page_id = page_id_from_url(&page.url)
            .ok_or_else(|| format!("could not parse Notion page id from {}", page.url))?;
        let mut blocks = fetch_blocks(&page_id).await?;
        resolve_missing(&mut blocks, &page_id).await;
        Ok::<_, String>(extract_catalog_from_blocks(
            &Value::Object(blocks),
            &page_id,
        ))
    });

    let mut catalog = Vec::new();
    for result in join_all(jobs).await {
        catalog.extend(result?);
    }
    Ok(dedupe_sections(catalog))
}

fn load_config() -> Result<NotionConfig, String> {
    NotionConfig::parse(EMBEDDED_CONFIG)
}

async fn resolve_missing(blocks: &mut Map<String, Value>, root_id: &str) {
    let mut fetched = 0;
    loop {
        if fetched >= MAX_CHILD_FETCHES {
            break;
        }
        let mut missing = missing_content_ids(blocks, root_id);
        if missing.is_empty() {
            break;
        }
        missing.truncate(MAX_CHILD_FETCHES - fetched);
        let jobs = missing.into_iter().map(|id| async move {
            let result = fetch_blocks(&id).await;
            (id, result)
        });
        for (id, result) in join_all(jobs).await {
            fetched += 1;
            match result {
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
        }
    }
}

async fn fetch_post_plain(page_id: &str) -> Result<Vec<PostSegment>, String> {
    let root_id = dashed_id(page_id);
    log::info!("fetching post body {root_id}");
    let mut blocks = fetch_blocks(&root_id).await?;
    resolve_missing(&mut blocks, &root_id).await;
    let segments = extract_segments(&Value::Object(blocks), &root_id);
    if segments.is_empty() {
        Ok(vec![PostSegment::Text("(no content)".to_string())])
    } else {
        Ok(segments)
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
    opts.set_cache(RequestCache::Reload);

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

#[derive(Serialize, Deserialize)]
struct CachedCatalog {
    saved_at: f64,
    sections: Vec<TagSection>,
}

fn load_session_cache() -> Option<Vec<TagSection>> {
    let window = web_sys::window()?;
    let storage = window.session_storage().ok().flatten()?;
    let raw = storage.get_item(CACHE_KEY).ok().flatten()?;
    let cached: CachedCatalog = serde_json::from_str(&raw).ok()?;
    Some(cached.sections)
}

fn save_session_cache(sections: &[TagSection]) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(Some(storage)) = window.session_storage() else {
        return;
    };
    let cached = CachedCatalog {
        saved_at: js_sys::Date::now(),
        sections: sections.to_vec(),
    };
    if let Ok(raw) = serde_json::to_string(&cached) {
        let _ = storage.set_item(CACHE_KEY, &raw);
    }
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
    extract_catalog_from_blocks(&root, page_id)
}

fn extract_catalog_from_blocks(root: &Value, page_id: &str) -> Vec<TagSection> {
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
        collect_pages(blocks, &child_content_ids(value), page_id, &tag, &mut pages);
        sections.push(TagSection { tag, pages });
    }
    sections
}

fn collect_pages(
    blocks: &Value,
    ids: &[String],
    root_id: &str,
    tag: &str,
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
                let compact = id.replace('-', "");
                out.push(ContentPage {
                    title,
                    id: compact.clone(),
                    href: format!("/blog/{}/{}", tag_slug(tag), compact),
                });
            }
            continue;
        }
        if kind == "toggle" || CONTAINER_TYPES.contains(&kind) || H2_TYPES.contains(&kind) {
            collect_pages(blocks, &child_content_ids(value), root_id, tag, out);
        }
    }
}

fn extract_segments(root: &Value, page_id: &str) -> Vec<PostSegment> {
    let blocks = block_map(root);
    if !blocks.is_object() {
        return Vec::new();
    }
    let mut segments = Vec::new();
    collect_segments(
        blocks,
        &page_child_ids(blocks, page_id),
        page_id,
        &mut segments,
    );
    merge_text_segments(segments)
}

fn collect_segments(blocks: &Value, ids: &[String], root_id: &str, out: &mut Vec<PostSegment>) {
    for id in ids {
        let value = block_value(&blocks[id]);
        let Some(kind) = value["type"].as_str() else {
            continue;
        };
        if kind == "page" && !same_page_id(id, root_id) {
            continue;
        }
        if kind == "code" {
            out.push(PostSegment::Code(block_source_text(value)));
        } else if let Some(text) = block_display_text(kind, value) {
            out.push(PostSegment::Text(text));
        }
        if kind == "toggle" || CONTAINER_TYPES.contains(&kind) || H2_TYPES.contains(&kind) {
            collect_segments(blocks, &child_content_ids(value), root_id, out);
        }
    }
}

fn block_source_text(value: &Value) -> String {
    block_raw_text(&value["properties"]["title"])
}

fn block_display_text(kind: &str, value: &Value) -> Option<String> {
    let text = block_raw_text(&value["properties"]["title"]);
    match kind {
        "text" | "quote" | "callout" | "header" | "sub_header" | "sub_sub_header"
        | "bulleted_list" | "numbered_list" | "to_do" | "toggle" => Some(text),
        _ => None,
    }
}

fn merge_text_segments(segments: Vec<PostSegment>) -> Vec<PostSegment> {
    let mut out = Vec::new();
    for segment in segments {
        match (out.last_mut(), segment) {
            (Some(PostSegment::Text(existing)), PostSegment::Text(next)) => {
                existing.push('\n');
                existing.push_str(&next);
            }
            (_, other) => out.push(other),
        }
    }
    out
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
                if kind == "page" && !same_page_id(&id, root_id) {
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

fn resolve_block_key(blocks: &Value, page_id: &str) -> String {
    if blocks.get(page_id).is_some() {
        return page_id.to_string();
    }
    let dashed = dashed_id(page_id);
    if blocks.get(&dashed).is_some() {
        return dashed;
    }
    let compact = page_id.replace('-', "");
    if let Some(key) = blocks.as_object().and_then(|map| {
        map.keys()
            .find(|key| key.replace('-', "") == compact)
            .cloned()
    }) {
        return key;
    }
    dashed
}

fn page_child_ids(blocks: &Value, page_id: &str) -> Vec<String> {
    let key = resolve_block_key(blocks, page_id);
    let page = block_value(&blocks[&key]);
    if let Some(ids) = page["content"].as_array() {
        return ids
            .iter()
            .filter_map(|id| id.as_str().map(str::to_owned))
            .collect();
    }

    let owned: Vec<String> = blocks
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(id, block)| {
            let value = block_value(block);
            let parent = value["parent_id"].as_str()?;
            same_page_id(parent, &key).then(|| id.clone())
        })
        .collect();
    if !owned.is_empty() {
        return owned;
    }

    blocks
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(id, block)| {
            let value = block_value(block);
            let kind = value["type"].as_str()?;
            (H2_TYPES.contains(&kind) && !same_page_id(id, &key)).then(|| id.clone())
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
    block_raw_text(title)
}

fn block_raw_text(title: &Value) -> String {
    let Some(parts) = title.as_array() else {
        return String::new();
    };
    parts
        .iter()
        .filter_map(|part| part.get(0).and_then(Value::as_str))
        .collect()
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
        assert!(catalog[0].pages[0].href.starts_with("/blog/TEST1/"));
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

    #[test]
    fn extracts_plain_text_in_order() {
        let json = format!(
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
                        "type": "quote",
                        "properties": {{ "title": [["created: 2025.12.12"]] }}
                      }}
                    }}
                  }},
                  "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa2": {{
                    "value": {{
                      "value": {{
                        "type": "text",
                        "properties": {{ "title": [["hello"]] }}
                      }}
                    }}
                  }}
                }}
              }}
            }}"#
        );
        let segments = super::extract_segments(&serde_json::from_str(&json).unwrap(), PAGE_ID);
        assert_eq!(
            segments,
            [super::PostSegment::Text(
                "created: 2025.12.12\nhello".into()
            )]
        );
    }

    #[test]
    fn keeps_code_and_indentation() {
        let json = format!(
            r#"{{
              "recordMap": {{
                "block": {{
                  "{PAGE_ID}": {{
                    "value": {{
                      "value": {{
                        "type": "page",
                        "content": ["aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa1"]
                      }}
                    }}
                  }},
                  "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa1": {{
                    "value": {{
                      "value": {{
                        "type": "code",
                        "properties": {{ "title": [["int a;\n    std::cin>>a;"]] }}
                      }}
                    }}
                  }}
                }}
              }}
            }}"#
        );
        let segments = super::extract_segments(&serde_json::from_str(&json).unwrap(), PAGE_ID);
        assert_eq!(
            segments,
            [super::PostSegment::Code("int a;\n    std::cin>>a;".into())]
        );
    }
}
