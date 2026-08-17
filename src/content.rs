use crate::module::scraper::{self, ContentPage};

/// Sidebar tags as a string array.
///
/// Filled in the browser on each page load from every `notion.json`
/// page whose `role` is `"tags"`.
pub fn tags() -> Vec<String> {
    scraper::current_tags()
}

/// Content pages nested under the given tag, or every tag when `slug` is `None`.
pub fn posts(slug: Option<&str>) -> Vec<ContentPage> {
    scraper::current_posts(slug)
}

/// Start a browser fetch of the Notion catalog. Call once per site load.
pub fn refresh() {
    scraper::start_fetch();
}
