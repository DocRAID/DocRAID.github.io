//! Native snapshot builder. Fetches public Notion pages and writes
//! `snapshot.json` plus `rss.xml` for the WASM app to embed.

use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tui_blog::module::config::{dashed_id, page_id_from_url, NotionConfig, EMBEDDED_CONFIG};
use tui_blog::module::notion::{
    block_map_owned, extract_catalog_from_blocks, extract_segments, merge_block_maps,
    missing_content_ids, parse_block_map, segments_plain_text, TagSection, MAX_CHILD_FETCHES,
};
use tui_blog::module::snapshot::Snapshot;

const NOTION_LIVE_API: &str = "https://notion-api.splitbee.io/v1/page/";
const DEFAULT_SITE: &str = "https://docraid.github.io";

fn main() {
    if let Err(err) = run() {
        eprintln!("fetch_content: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config = NotionConfig::parse(EMBEDDED_CONFIG)?;
    let mut snapshot = Snapshot {
        saved_at: unix_ms(),
        sections: Vec::new(),
        posts: HashMap::new(),
        about: None,
    };

    let mut errors = Vec::new();
    for page in config.tag_pages() {
        match load_tag_page(&page.url) {
            Ok(sections) => snapshot.sections.extend(sections),
            Err(err) => {
                eprintln!("tag page {}: {err}", page.url);
                errors.push(err);
            }
        }
    }
    snapshot.sections = tui_blog::module::notion::dedupe_sections(snapshot.sections);

    for page in config.about_pages() {
        match load_about(&page.url) {
            Ok(text) => {
                snapshot.about = Some(text);
                break;
            }
            Err(err) => eprintln!("about page {}: {err}", page.url),
        }
    }

    if snapshot.sections.is_empty() && snapshot.about.is_none() && !errors.is_empty() {
        return Err(errors.remove(0));
    }

    for section in &snapshot.sections {
        for page in &section.pages {
            match load_post(&page.id) {
                Ok(segments) => {
                    snapshot.posts.insert(page.id.replace('-', ""), segments);
                }
                Err(err) => eprintln!("post {}: {err}", page.id),
            }
        }
    }

    let snapshot_path = root.join("snapshot.json");
    fs::write(&snapshot_path, snapshot.to_json()?).map_err(|err| err.to_string())?;
    println!(
        "wrote {} ({} tags, {} posts)",
        snapshot_path.display(),
        snapshot.sections.len(),
        snapshot.posts.len()
    );

    let rss_path = root.join("rss.xml");
    fs::write(&rss_path, render_rss(&snapshot)).map_err(|err| err.to_string())?;
    println!("wrote {}", rss_path.display());
    Ok(())
}

fn load_tag_page(url: &str) -> Result<Vec<TagSection>, String> {
    let page_id = page_id_from_url(url)
        .ok_or_else(|| format!("could not parse Notion page id from {url}"))?;
    let blocks = load_blocks(&page_id)?;
    Ok(extract_catalog_from_blocks(
        &Value::Object(blocks),
        &page_id,
    ))
}

fn load_about(url: &str) -> Result<String, String> {
    let page_id = page_id_from_url(url)
        .ok_or_else(|| format!("could not parse Notion page id from {url}"))?;
    let segments = load_post(&page_id)?;
    Ok(segments_plain_text(&segments))
}

fn load_post(page_id: &str) -> Result<Vec<tui_blog::module::notion::PostSegment>, String> {
    let root_id = dashed_id(page_id);
    let blocks = load_blocks(&root_id)?;
    let segments = extract_segments(&Value::Object(blocks), &root_id);
    if segments.is_empty() {
        Ok(vec![tui_blog::module::notion::PostSegment::Text(
            "(no content)".to_string(),
        )])
    } else {
        Ok(segments)
    }
}

fn load_blocks(page_id: &str) -> Result<serde_json::Map<String, Value>, String> {
    let json = fetch_page_json(page_id)?;
    let mut blocks = parse_block_map(&json).or_else(|_| {
        let root: Value =
            serde_json::from_str(&json).map_err(|err| format!("invalid notion json: {err}"))?;
        block_map_owned(root).ok_or_else(|| "notion response is missing a block map".to_string())
    })?;
    resolve_missing(&mut blocks, page_id)?;
    Ok(blocks)
}

fn resolve_missing(
    blocks: &mut serde_json::Map<String, Value>,
    root_id: &str,
) -> Result<(), String> {
    let mut fetched = 0;
    loop {
        if fetched >= MAX_CHILD_FETCHES {
            let leftover = missing_content_ids(blocks, root_id).len();
            if leftover > 0 {
                eprintln!(
                    "stopped resolving Notion children after {MAX_CHILD_FETCHES} fetches ({leftover} still missing)"
                );
            }
            break;
        }
        let mut missing = missing_content_ids(blocks, root_id);
        if missing.is_empty() {
            break;
        }
        missing.truncate(MAX_CHILD_FETCHES - fetched);
        for id in missing {
            fetched += 1;
            match fetch_page_json(&id).and_then(|json| parse_block_map(&json)) {
                Ok(extra) => merge_block_maps(blocks, extra),
                Err(err) => {
                    eprintln!("skipping nested Notion block {id}: {err}");
                    blocks.insert(id, Value::Null);
                }
            }
        }
    }
    Ok(())
}

fn fetch_page_json(page_id: &str) -> Result<String, String> {
    let compact = page_id.replace('-', "");
    let url = format!("{NOTION_LIVE_API}{compact}");
    let response = ureq::get(&url)
        .set("User-Agent", "tui_blog-fetch_content/0.1")
        .call()
        .map_err(|err| format!("{url}: {err}"))?;
    if response.status() >= 400 {
        return Err(format!("http {} from {url}", response.status()));
    }
    response.into_string().map_err(|err| err.to_string())
}

fn render_rss(snapshot: &Snapshot) -> String {
    let site = std::env::var("SITE_URL").unwrap_or_else(|_| DEFAULT_SITE.to_string());
    let site = site.trim_end_matches('/');
    let mut items = String::new();
    for section in &snapshot.sections {
        for page in &section.pages {
            let title = xml_escape(&page.title);
            let link = format!("{site}{}", page.href);
            items.push_str(&format!(
                "    <item>\n      <title>{title}</title>\n      <link>{}</link>\n      <guid>{}</guid>\n      <category>{}</category>\n    </item>\n",
                xml_escape(&link),
                xml_escape(&link),
                xml_escape(&section.tag)
            ));
        }
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>tui_blog</title>
    <link>{site}/</link>
    <description>Dongju Lim's personal technical blog</description>
{items}  </channel>
</rss>
"#
    )
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
