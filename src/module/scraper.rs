//! Browser Notion fetch used when the compile-time snapshot is empty,
//! and for post bodies that were not snapshotted.

use super::catalog::{self, CatalogStatus};
use super::config::{dashed_id, page_id_from_url, NotionConfig, EMBEDDED_CONFIG};
use super::notion::{
    self, block_map_owned, extract_catalog_from_blocks, extract_segments, merge_block_maps,
    missing_content_ids, parse_block_map, MAX_CHILD_FETCHES,
};
use futures::future::join_all;
use serde_json::{Map, Value};
use std::cell::RefCell;
use std::collections::HashSet;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestCache, RequestInit, RequestMode, Response};

const NOTION_LIVE_API: &str = "https://notion-api.splitbee.io/v1/page/";
const RETRY_AFTER_MS: u64 = 3_000;

thread_local! {
    static LIVE_BUSY: RefCell<bool> = const { RefCell::new(false) };
    static POST_FETCHING: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

pub use notion::{same_page_id, tag_slug, ContentPage, PostSegment, TagSection};

pub fn current_tags() -> Vec<String> {
    catalog::current_tags()
}

pub fn current_posts(slug: Option<&str>) -> Vec<ContentPage> {
    catalog::current_posts(slug)
}

pub fn current_tagged_posts() -> Vec<(String, ContentPage)> {
    catalog::current_tagged_posts()
}

pub fn current_post_segments(page_id: &str) -> Option<Vec<PostSegment>> {
    match catalog::current_post_state(page_id) {
        Some(Ok(segments)) => Some(segments),
        Some(Err(_)) | None => None,
    }
}

pub fn current_post_state(page_id: &str) -> Option<Result<Vec<PostSegment>, String>> {
    catalog::current_post_state(page_id)
}

/// Kick off a scrape of one Notion post. Safe to call every frame.
pub fn request_post(page_id: &str) {
    catalog::bootstrap();
    let id = catalog::compact_id(page_id);
    if id.is_empty() {
        return;
    }
    if catalog::post_ready(&id) {
        return;
    }
    if let Some((_, at_ms)) = catalog::failed_at(&id) {
        if now_ms().saturating_sub(at_ms) < RETRY_AFTER_MS {
            return;
        }
    }
    if !POST_FETCHING.with(|busy| busy.borrow_mut().insert(id.clone())) {
        return;
    }
    wasm_bindgen_futures::spawn_local(async move {
        let result = fetch_post_plain(&id).await;
        POST_FETCHING.with(|busy| {
            busy.borrow_mut().remove(&id);
        });
        match result {
            Ok(segments) => catalog::insert_post(&id, segments),
            Err(err) => {
                log::error!("post scrape failed: {err}");
                catalog::insert_post_failure(&id, err, now_ms());
            }
        }
    });
}

/// Load the embedded snapshot, then live-fetch the catalog only if it is empty.
pub fn start_fetch() {
    catalog::bootstrap();
    if catalog::catalog_is_empty() {
        spawn_live_catalog();
    }
}

fn spawn_live_catalog() {
    if LIVE_BUSY.with(|busy| {
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
        LIVE_BUSY.with(|busy| *busy.borrow_mut() = false);
        match result {
            Ok((catalog, about)) => {
                if catalog.is_empty() && about.is_none() {
                    if catalog::catalog_is_empty() {
                        catalog::set_status(CatalogStatus::Ready);
                    }
                    return;
                }
                log::info!(
                    "notion catalog updated ({} tags / {} posts)",
                    catalog.len(),
                    catalog
                        .iter()
                        .map(|section| section.pages.len())
                        .sum::<usize>()
                );
                catalog::apply_catalog(catalog);
                if about.is_some() {
                    catalog::set_about(about);
                }
            }
            Err(err) => {
                log::error!("notion catalog fetch failed: {err}");
                if catalog::catalog_is_empty() {
                    catalog::set_status(CatalogStatus::Error(err));
                }
            }
        }
    });
}

async fn fetch_catalog() -> Result<(Vec<TagSection>, Option<String>), String> {
    let config = NotionConfig::parse(EMBEDDED_CONFIG)?;
    let sources: Vec<_> = config.tag_pages().cloned().collect();
    if sources.is_empty() {
        log::warn!("notion.json has no pages with role \"tags\"");
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
    let mut errors = Vec::new();
    for result in join_all(jobs).await {
        match result {
            Ok(sections) => catalog.extend(sections),
            Err(err) => {
                log::error!("tag page failed: {err}");
                errors.push(err);
            }
        }
    }
    let catalog = notion::dedupe_sections(catalog);

    let mut about = None;
    for page in config.about_pages() {
        match load_about_page(&page.url).await {
            Ok(text) => {
                about = Some(text);
                break;
            }
            Err(err) => log::warn!("about page failed: {err}"),
        }
    }

    if catalog.is_empty() && about.is_none() && !errors.is_empty() {
        return Err(errors.remove(0));
    }
    Ok((catalog, about))
}

async fn load_about_page(url: &str) -> Result<String, String> {
    let page_id = page_id_from_url(url)
        .ok_or_else(|| format!("could not parse Notion page id from {url}"))?;
    let segments = fetch_post_plain(&page_id).await?;
    Ok(notion::segments_plain_text(&segments))
}

async fn resolve_missing(blocks: &mut Map<String, Value>, root_id: &str) {
    let mut fetched = 0;
    loop {
        if fetched >= MAX_CHILD_FETCHES {
            let leftover = missing_content_ids(blocks, root_id).len();
            if leftover > 0 {
                log::warn!(
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
        let jobs = missing.into_iter().map(|id| async move {
            let result = fetch_blocks(&id).await;
            (id, result)
        });
        for (id, result) in join_all(jobs).await {
            fetched += 1;
            match result {
                Ok(extra) => merge_block_maps(blocks, extra),
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
    parse_block_map(&json).or_else(|_| {
        let root: Value =
            serde_json::from_str(&json).map_err(|err| format!("invalid notion json: {err}"))?;
        block_map_owned(root).ok_or_else(|| "notion response is missing a block map".to_string())
    })
}

async fn fetch_page_json(page_id: &str) -> Result<String, String> {
    let compact_id = page_id.replace('-', "");
    let url = format!("{NOTION_LIVE_API}{compact_id}");
    fetch_text(&url).await
}

async fn fetch_text(url: &str) -> Result<String, String> {
    let opts = RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(RequestMode::Cors);
    opts.set_cache(RequestCache::Default);

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

fn now_ms() -> u64 {
    js_sys::Date::now() as u64
}

#[cfg(test)]
pub fn set_catalog_for_tests(catalog: Vec<TagSection>) {
    catalog::set_catalog_for_tests(catalog);
}
