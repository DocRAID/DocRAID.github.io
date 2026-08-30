//! Notion unofficial-API block map parsing.

use super::config::dashed_id;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashSet;

const H2_TYPES: &[&str] = &["header", "sub_header"];
pub const CONTAINER_TYPES: &[&str] = &[
    "toggle",
    "column_list",
    "column",
    "bulleted_list",
    "numbered_list",
    "to_do",
    "transclusion_container",
];

pub const MAX_CHILD_FETCHES: usize = 256;

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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PostSegment {
    Text(String),
    Code(String),
}

pub fn tag_slug(tag: &str) -> String {
    tag.replace(['/', ' '], "-")
}

pub fn same_page_id(left: &str, right: &str) -> bool {
    left.replace('-', "") == right.replace('-', "")
}

pub fn extract_catalog_from_blocks(root: &Value, page_id: &str) -> Vec<TagSection> {
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

pub fn extract_segments(root: &Value, page_id: &str) -> Vec<PostSegment> {
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

pub fn segments_plain_text(segments: &[PostSegment]) -> String {
    let mut out = String::new();
    for segment in segments {
        if !out.is_empty() {
            out.push('\n');
        }
        match segment {
            PostSegment::Text(text) | PostSegment::Code(text) => out.push_str(text),
        }
    }
    out
}

pub fn merge_block_maps(dest: &mut Map<String, Value>, extra: Map<String, Value>) {
    for (key, value) in extra {
        dest.entry(key).or_insert(value);
    }
}

pub fn missing_content_ids(blocks: &Map<String, Value>, root_id: &str) -> Vec<String> {
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

pub fn block_map_owned(root: Value) -> Option<Map<String, Value>> {
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

pub fn parse_block_map(json: &str) -> Result<Map<String, Value>, String> {
    let root: Value =
        serde_json::from_str(json).map_err(|err| format!("invalid notion json: {err}"))?;
    block_map_owned(root).ok_or_else(|| "notion response is missing a block map".to_string())
}

pub fn dedupe_sections(sections: Vec<TagSection>) -> Vec<TagSection> {
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

fn collect_segments(blocks: &Value, ids: &[String], root_id: &str, out: &mut Vec<PostSegment>) {
    let mut numbered = 0_u32;
    for id in ids {
        let value = block_value(&blocks[id]);
        let Some(kind) = value["type"].as_str() else {
            continue;
        };
        if kind == "page" && !same_page_id(id, root_id) {
            continue;
        }
        if kind == "numbered_list" {
            numbered += 1;
        } else {
            numbered = 0;
        }
        if kind == "code" {
            out.push(PostSegment::Code(block_source_text(value)));
        } else if let Some(text) = block_display_text(kind, value, numbered) {
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

fn block_display_text(kind: &str, value: &Value, numbered: u32) -> Option<String> {
    let text = block_raw_text(&value["properties"]["title"]);
    match kind {
        "text" | "quote" | "callout" | "header" | "sub_header" | "sub_sub_header"
        | "bulleted_list" | "toggle" => Some(text),
        "numbered_list" => {
            if numbered == 0 {
                Some(text)
            } else {
                Some(format!("{numbered}. {text}"))
            }
        }
        "to_do" => {
            let checked = is_checked(value);
            let mark = if checked { "[x]" } else { "[ ]" };
            Some(format!("{mark} {text}"))
        }
        "divider" => Some("────────".to_string()),
        "image" => Some(format!("[image] {}", first_link(value)).trim().to_string()),
        "bookmark" | "link_preview" | "embed" => {
            let link = first_link(value);
            if link.is_empty() && text.is_empty() {
                Some(format!("[{kind}]"))
            } else if text.is_empty() {
                Some(link)
            } else if link.is_empty() {
                Some(text)
            } else {
                Some(format!("{text} ({link})"))
            }
        }
        "table" => Some("[table]".to_string()),
        "table_row" => {
            let cells = table_row_text(value);
            if cells.is_empty() {
                None
            } else {
                Some(cells)
            }
        }
        _ => None,
    }
}

fn is_checked(value: &Value) -> bool {
    value["properties"]["checked"]
        .as_array()
        .and_then(|parts| parts.first())
        .and_then(|part| part.get(0))
        .and_then(Value::as_str)
        .is_some_and(|flag| flag.eq_ignore_ascii_case("Yes") || flag == "true")
}

fn first_link(value: &Value) -> String {
    for key in ["source", "link", "url", "title"] {
        let text = block_raw_text(&value["properties"][key]);
        if !text.is_empty() {
            return text;
        }
    }
    String::new()
}

fn table_row_text(value: &Value) -> String {
    let Some(cells) = value["properties"].as_object() else {
        return String::new();
    };
    let mut cols: Vec<(String, String)> = cells
        .iter()
        .map(|(key, val)| (key.clone(), block_raw_text(val)))
        .collect();
    cols.sort_by(|a, b| a.0.cmp(&b.0));
    cols.into_iter()
        .map(|(_, text)| text)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" | ")
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

#[cfg(test)]
mod tests {
    use super::{extract_catalog, extract_h2_titles, extract_segments, PostSegment};

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
        let segments = extract_segments(&serde_json::from_str(&json).unwrap(), PAGE_ID);
        assert_eq!(
            segments,
            [PostSegment::Text("created: 2025.12.12\nhello".into())]
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
        let segments = extract_segments(&serde_json::from_str(&json).unwrap(), PAGE_ID);
        assert_eq!(
            segments,
            [PostSegment::Code("int a;\n    std::cin>>a;".into())]
        );
    }

    #[test]
    fn numbers_lists_and_todos() {
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
                          "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa2",
                          "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa3"
                        ]
                      }}
                    }}
                  }},
                  "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa1": {{
                    "value": {{ "value": {{
                      "type": "numbered_list",
                      "properties": {{ "title": [["one"]] }}
                    }} }}
                  }},
                  "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa2": {{
                    "value": {{ "value": {{
                      "type": "numbered_list",
                      "properties": {{ "title": [["two"]] }}
                    }} }}
                  }},
                  "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa3": {{
                    "value": {{ "value": {{
                      "type": "to_do",
                      "properties": {{
                        "title": [["done"]],
                        "checked": [["Yes"]]
                      }}
                    }} }}
                  }}
                }}
              }}
            }}"#
        );
        let segments = extract_segments(&serde_json::from_str(&json).unwrap(), PAGE_ID);
        assert_eq!(
            segments,
            [PostSegment::Text("1. one\n2. two\n[x] done".into())]
        );
    }
}
