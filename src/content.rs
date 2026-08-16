/// Sidebar tags as a string array.
///
/// Filled in the browser on each page load. There is no backend: the
/// scraper module fetches the Notion page from WASM and caches the
/// H2 titles in memory for this visit.
pub fn tags() -> Vec<String> {
    crate::module::scraper::current_tags()
}

/// Start a browser fetch of the Notion H2s. Call once per site load.
pub fn refresh() {
    crate::module::scraper::start_fetch();
}
