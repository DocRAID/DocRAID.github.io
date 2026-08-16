/// Sidebar tags as a string array.
///
/// Filled in the browser on each page load from every `notion.json`
/// page whose `role` is `"tags"`.
pub fn tags() -> Vec<String> {
    crate::module::scraper::current_tags()
}

/// Start a browser fetch of the Notion H2s. Call once per site load.
pub fn refresh() {
    crate::module::scraper::start_fetch();
}
