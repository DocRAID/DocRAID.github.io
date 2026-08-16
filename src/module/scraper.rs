//! Browser-only Notion scrape. This site is static WASM with no origin
//! server, so every visit uses `window.fetch` and keeps the result in
//! memory. Notion's own endpoints omit CORS headers, which blocks a
//! direct read of [`NOTION_PAGE_URL`]; the public Splitbee reader
//! returns that same page's block map and allows cross-origin GET.

use serde_json::Value;
use std::cell::RefCell;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestCache, RequestInit, RequestMode, Response};

/// Public Notion study-log page whose heading-2 sections become sidebar tags.
pub const NOTION_PAGE_URL: &str = "https://limdongju.notion.site/158ec5eb3d2280218426f12e40729e48";
pub const NOTION_PAGE_ID: &str = "158ec5eb-3d22-8021-8426-f12e40729e48";

const NOTION_LIVE_API: &str = "https://notion-api.splitbee.io/v1/page/";

/// Heading types that render as H2 on a published Notion site
/// (the page title is H1; Notion "Heading 1" / "Heading 2" follow it).
const H2_TYPES: &[&str] = &["header", "sub_header"];

thread_local! {
    static TAGS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Latest H2 titles from the in-memory fetch. Empty until the request finishes.
pub fn current_tags() -> Vec<String> {
    TAGS.with(|tags| tags.borrow().clone())
}

/// Start a fresh fetch. Safe to call once at page load.
pub fn start_fetch() {
    wasm_bindgen_futures::spawn_local(async {
        match fetch_h2_tags().await {
            Ok(titles) => {
                log::info!(
                    "fetched {} notion tags from {NOTION_PAGE_URL}",
                    titles.len()
                );
                TAGS.with(|tags| *tags.borrow_mut() = titles);
            }
            Err(err) => log::error!("notion tag fetch failed: {err}"),
        }
    });
}

async fn fetch_h2_tags() -> Result<Vec<String>, String> {
    let json = fetch_page_json().await?;
    Ok(extract_h2_titles(&json))
}

async fn fetch_page_json() -> Result<String, String> {
    let compact_id = NOTION_PAGE_ID.replace('-', "");
    let cache_bust = js_sys::Date::now() as u64;
    let url = format!("{NOTION_LIVE_API}{compact_id}?t={cache_bust}");

    let opts = RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(RequestMode::Cors);
    opts.set_cache(RequestCache::NoStore);

    let request = Request::new_with_str_and_init(&url, &opts).map_err(js_err)?;
    let window = web_sys::window().ok_or_else(|| "missing window".to_string())?;
    let response = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(js_err)?;
    let response: Response = response.dyn_into().map_err(js_err)?;
    if !response.ok() {
        return Err(format!("http {}", response.status()));
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

/// Walk the page's child list in document order and collect H2 text.
pub fn extract_h2_titles(json: &str) -> Vec<String> {
    let Ok(root) = serde_json::from_str::<Value>(json) else {
        log::error!("notion response is not valid JSON");
        return Vec::new();
    };

    let blocks = block_map(&root);
    if !blocks.is_object() {
        log::error!("notion response is missing a block map");
        return Vec::new();
    }

    let mut titles = Vec::new();
    for block_id in page_child_ids(blocks) {
        let value = block_value(&blocks[&block_id]);
        let Some(kind) = value["type"].as_str() else {
            continue;
        };
        if !H2_TYPES.contains(&kind) {
            continue;
        }
        let title = rich_text(&value["properties"]["title"]);
        if !title.is_empty() {
            titles.push(title);
        }
    }
    titles
}

fn block_map(root: &Value) -> &Value {
    let nested = &root["recordMap"]["block"];
    if nested.is_object() {
        nested
    } else {
        root
    }
}

fn page_child_ids(blocks: &Value) -> Vec<String> {
    let page = block_value(&blocks[NOTION_PAGE_ID]);
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
            (H2_TYPES.contains(&kind) && id != NOTION_PAGE_ID).then(|| id.clone())
        })
        .collect()
}

fn block_value(block: &Value) -> &Value {
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

#[cfg(test)]
mod tests {
    use super::{extract_h2_titles, NOTION_PAGE_ID};

    fn fixture() -> String {
        format!(
            r#"{{
              "recordMap": {{
                "block": {{
                  "{NOTION_PAGE_ID}": {{
                    "value": {{
                      "value": {{
                        "id": "{NOTION_PAGE_ID}",
                        "type": "page",
                        "content": [
                          "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa1",
                          "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa2",
                          "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa3"
                        ]
                      }}
                    }}
                  }},
                  "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa1": {{
                    "value": {{
                      "value": {{
                        "type": "header",
                        "properties": {{ "title": [["OS"]] }}
                      }}
                    }}
                  }},
                  "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa2": {{
                    "value": {{
                      "value": {{
                        "type": "page",
                        "properties": {{ "title": [["Linux"]] }}
                      }}
                    }}
                  }},
                  "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa3": {{
                    "value": {{
                      "value": {{
                        "type": "sub_header",
                        "properties": {{ "title": [["C/C++"]] }}
                      }}
                    }}
                  }}
                }}
              }}
            }}"#
        )
    }

    fn splitbee_fixture() -> String {
        format!(
            r#"{{
              "{NOTION_PAGE_ID}": {{
                "value": {{
                  "value": {{
                    "id": "{NOTION_PAGE_ID}",
                    "type": "page",
                    "content": ["aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa1"]
                  }}
                }}
              }},
              "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa1": {{
                "value": {{
                  "value": {{
                    "type": "header",
                    "properties": {{ "title": [["DB"]] }}
                  }}
                }}
              }}
            }}"#
        )
    }

    #[test]
    fn extracts_header_and_sub_header_in_order() {
        assert_eq!(extract_h2_titles(&fixture()), ["OS", "C/C++"]);
    }

    #[test]
    fn extracts_from_unwrapped_block_map() {
        assert_eq!(extract_h2_titles(&splitbee_fixture()), ["DB"]);
    }

    #[test]
    fn ignores_invalid_json() {
        assert!(extract_h2_titles("not-json").is_empty());
    }
}
