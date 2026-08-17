use crate::module::scraper::{self, ContentPage, PostSegment};

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

/// Start a scrape of one post body if it is not already cached.
pub fn ensure_post(page_id: &str) {
    scraper::request_post(page_id);
}

/// Scraped post body, if the fetch has finished.
pub fn post_segments(page_id: &str) -> Option<Vec<PostSegment>> {
    scraper::current_post_segments(page_id)
}

/// Start a browser fetch of the Notion catalog. Call once per site load.
pub fn refresh() {
    scraper::start_fetch();
}
