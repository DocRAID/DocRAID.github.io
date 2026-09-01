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
    Image(PostImage),
}

/// A Notion image (or image file) to overlay on the post body.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostImage {
    pub src: String,
    #[serde(default)]
    pub alt: String,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
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
            PostSegment::Image(image) => {
                if image.alt.is_empty() {
                    out.push_str("[image]");
                } else {
                    out.push_str("[image] ");
                    out.push_str(&image.alt);
                }
            }
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
        } else if let Some(image) = extract_image(kind, id, value) {
            out.push(PostSegment::Image(image));
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

fn extract_image(kind: &str, block_id: &str, value: &Value) -> Option<PostImage> {
    if kind != "image" && kind != "file" {
        return None;
    }
    let raw = image_raw_source(value);
    if raw.is_empty() {
        return None;
    }
    let caption = block_raw_text(&value["properties"]["caption"]);
    let title = block_raw_text(&value["properties"]["title"]);
    let alt = if caption.is_empty() { title } else { caption };
    if kind == "file" && !looks_like_image(&raw) && !looks_like_image(&alt) {
        return None;
    }
    let src = map_image_url(&raw, block_id, value["space_id"].as_str());
    if src.is_empty() {
        return None;
    }
    let width = json_u32(&value["format"]["block_width"]);
    let height = json_u32(&value["format"]["block_height"]).or_else(|| {
        let aspect = value["format"]["block_aspect_ratio"].as_f64()?;
        let w = width.unwrap_or(1000);
        Some((f64::from(w) * aspect).round().max(1.0) as u32)
    });
    Some(PostImage {
        src,
        alt,
        width,
        height,
    })
}

fn image_raw_source(value: &Value) -> String {
    if let Some(source) = value["format"]["display_source"].as_str() {
        if !source.is_empty() {
            return source.to_string();
        }
    }
    for key in ["source", "url"] {
        let text = block_raw_text(&value["properties"][key]);
        if !text.is_empty() {
            return text;
        }
    }
    String::new()
}

fn looks_like_image(name_or_url: &str) -> bool {
    let path = name_or_url
        .split(['?', '#'])
        .next()
        .unwrap_or(name_or_url)
        .rsplit(['/', ':'])
        .next()
        .unwrap_or(name_or_url);
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "avif" | "ico"
    )
}

fn map_image_url(raw: &str, block_id: &str, space_id: Option<&str>) -> String {
    let raw = raw.trim();
    if raw.is_empty() {
        return String::new();
    }
    if raw.starts_with("data:") || raw.starts_with("https://images.unsplash.com") {
        return raw.to_string();
    }
    if is_direct_http_image(raw) {
        return raw.to_string();
    }
    let source = if raw.starts_with("/images") {
        format!("https://www.notion.so{raw}")
    } else {
        raw.to_string()
    };
    let mut url = format!(
        "https://www.notion.so/image/{}?table=block&id={}&cache=v2",
        encode_uri_component(&source),
        super::config::dashed_id(block_id)
    );
    if let Some(space) = space_id.filter(|id| !id.is_empty()) {
        url.push_str("&spaceId=");
        url.push_str(space);
    }
    url
}

fn is_direct_http_image(raw: &str) -> bool {
    let lower = raw.to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://"))
        && !lower.contains("amazonaws.com")
        && !lower.contains("notion-static")
        && !lower.contains("prod-files-secure")
        && !lower.contains("notionusercontent.com")
        && !lower.contains("notion.so/")
        && !lower.contains("notion.site/")
}

fn encode_uri_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(byte as char),
            _ => {
                const HEX: &[u8; 16] = b"0123456789ABCDEF";
                out.push('%');
                out.push(HEX[(byte >> 4) as usize] as char);
                out.push(HEX[(byte & 0xf) as usize] as char);
            }
        }
    }
    out
}

fn json_u32(value: &Value) -> Option<u32> {
    value
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .or_else(|| {
            let n = value.as_f64()?;
            if n.is_finite() && n > 0.0 {
                Some(n.round() as u32)
            } else {
                None
            }
        })
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
    use super::{
        encode_uri_component, extract_catalog, extract_h2_titles, extract_segments, map_image_url,
        PostImage, PostSegment,
    };

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

    #[test]
    fn extracts_attachment_images_as_segments() {
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
                      "type": "text",
                      "properties": {{ "title": [["before"]] }}
                    }} }}
                  }},
                  "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa2": {{
                    "value": {{ "value": {{
                      "id": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa2",
                      "type": "image",
                      "properties": {{
                        "title": [["image.png"]],
                        "source": [["attachment:7a1e5ccd-30d2-4e13-a112-8d48b21b099b:image.png"]]
                      }},
                      "format": {{
                        "block_width": 676,
                        "block_height": 312,
                        "display_source": "attachment:7a1e5ccd-30d2-4e13-a112-8d48b21b099b:image.png",
                        "block_aspect_ratio": 0.4508670520231214
                      }},
                      "space_id": "9b1457a8-0dc4-4a55-a7a6-0d2d40822805"
                    }} }}
                  }},
                  "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa3": {{
                    "value": {{ "value": {{
                      "type": "text",
                      "properties": {{ "title": [["after"]] }}
                    }} }}
                  }}
                }}
              }}
            }}"#
        );
        let segments = extract_segments(&serde_json::from_str(&json).unwrap(), PAGE_ID);
        assert_eq!(
            segments,
            [
                PostSegment::Text("before".into()),
                PostSegment::Image(PostImage {
                    src: "https://www.notion.so/image/attachment%3A7a1e5ccd-30d2-4e13-a112-8d48b21b099b%3Aimage.png?table=block&id=aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa2&cache=v2&spaceId=9b1457a8-0dc4-4a55-a7a6-0d2d40822805".into(),
                    alt: "image.png".into(),
                    width: Some(676),
                    height: Some(312),
                }),
                PostSegment::Text("after".into()),
            ]
        );
    }

    #[test]
    fn keeps_external_image_urls() {
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
                    "value": {{ "value": {{
                      "type": "image",
                      "properties": {{
                        "source": [["https://example.com/pic.jpg"]]
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
            [PostSegment::Image(PostImage {
                src: "https://example.com/pic.jpg".into(),
                alt: String::new(),
                width: None,
                height: None,
            })]
        );
    }

    #[test]
    fn skips_non_image_files() {
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
                    "value": {{ "value": {{
                      "type": "file",
                      "properties": {{
                        "title": [["notes.pdf"]],
                        "source": [["https://example.com/notes.pdf"]]
                      }}
                    }} }}
                  }}
                }}
              }}
            }}"#
        );
        let segments = extract_segments(&serde_json::from_str(&json).unwrap(), PAGE_ID);
        assert!(segments.is_empty());
    }

    #[test]
    fn map_image_url_proxies_attachments() {
        let url = map_image_url(
            "attachment:7a1e5ccd-30d2-4e13-a112-8d48b21b099b:image.png",
            "3ceec5eb3d22802d90fccae64a6bd2bd",
            Some("9b1457a8-0dc4-4a55-a7a6-0d2d40822805"),
        );
        assert!(url.starts_with("https://www.notion.so/image/attachment%3A"));
        assert!(url.contains("id=3ceec5eb-3d22-802d-90fc-cae64a6bd2bd"));
        assert!(url.contains("spaceId=9b1457a8-0dc4-4a55-a7a6-0d2d40822805"));
        assert_eq!(
            encode_uri_component("attachment:x:y.png"),
            "attachment%3Ax%3Ay.png"
        );
    }
}
