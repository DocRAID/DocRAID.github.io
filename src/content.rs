use crate::module::catalog;
use crate::module::notion::{ContentPage, PostSegment};
use crate::module::scraper;
use std::collections::HashSet;

pub use crate::module::catalog::CatalogStatus;

/// Sidebar tags as a string array.
///
/// Filled from `localStorage` when present, otherwise by a Notion scrape.
pub fn tags() -> Vec<String> {
    scraper::current_tags()
}

/// Content pages nested under the given tag, or every tag when `slug` is `None`.
pub fn posts(slug: Option<&str>) -> Vec<ContentPage> {
    scraper::current_posts(slug)
}

/// A catalog post with its tag and a date parsed from the title, if any.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentPost {
    pub title: String,
    pub date: Option<String>,
    pub href: String,
    pub tag: String,
}

/// The `limit` newest posts, ordered by the trailing date in each title.
pub fn recent_posts(limit: usize) -> Vec<RecentPost> {
    select_recent(scraper::current_tagged_posts(), limit)
}

fn select_recent(
    pages: impl IntoIterator<Item = (String, ContentPage)>,
    limit: usize,
) -> Vec<RecentPost> {
    let mut posts = Vec::new();
    let mut seen = HashSet::new();
    for (tag, page) in pages {
        if !seen.insert(page.id.clone()) {
            continue;
        }
        posts.push(recent_from_page(tag, page));
    }
    posts.sort_by(|left, right| {
        date_key(right.date.as_deref())
            .cmp(&date_key(left.date.as_deref()))
            .then_with(|| left.title.cmp(&right.title))
    });
    posts.truncate(limit);
    posts
}

fn recent_from_page(tag: String, page: ContentPage) -> RecentPost {
    match split_trailing_date(&page.title) {
        Some((name, date)) => RecentPost {
            title: name.to_string(),
            date: Some(date.to_string()),
            href: page.href,
            tag,
        },
        None => RecentPost {
            title: page.title,
            date: None,
            href: page.href,
            tag,
        },
    }
}

/// Split `Title - date` so the date can be shown or sorted separately.
pub fn split_trailing_date(title: &str) -> Option<(&str, &str)> {
    let (name, date) = title
        .rsplit_once(" - ")
        .or_else(|| title.rsplit_once('-'))?;
    let name = name.trim_end();
    let date = date.trim();
    if name.is_empty() || !looks_like_date(date) {
        None
    } else {
        Some((name, date))
    }
}

fn looks_like_date(text: &str) -> bool {
    let digits = text.chars().filter(|ch| ch.is_ascii_digit()).count();
    digits >= 4
        && text
            .chars()
            .all(|ch| ch.is_ascii_digit() || matches!(ch, '.' | '-' | '/' | ' '))
}

fn date_key(date: Option<&str>) -> Option<(u16, u8, u8)> {
    let date = date?;
    let mut parts = date
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty());
    let year: u16 = parts.next()?.parse().ok()?;
    let month: u8 = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    let day: u8 = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    Some((year, month, day))
}

/// Start a scrape of one post body if it is not already cached.
pub fn ensure_post(page_id: &str) {
    scraper::request_post(page_id);
}

/// Scraped post body, if the fetch has finished successfully.
pub fn post_segments(page_id: &str) -> Option<Vec<PostSegment>> {
    scraper::current_post_segments(page_id)
}

/// `None` = still loading, `Some(Ok)` = body, `Some(Err)` = last failure.
pub fn post_state(page_id: &str) -> Option<Result<Vec<PostSegment>, String>> {
    scraper::current_post_state(page_id)
}

pub fn catalog_status() -> CatalogStatus {
    catalog::bootstrap();
    catalog::status()
}

pub fn about_text() -> Option<String> {
    catalog::bootstrap();
    catalog::about()
}

/// Hydrate from `localStorage` if possible, then scrape Notion to refresh it.
pub fn refresh() {
    scraper::start_fetch();
}

#[cfg(test)]
mod tests {
    use super::{date_key, select_recent, split_trailing_date, RecentPost};
    use crate::module::scraper::ContentPage;

    fn page(title: &str, id: &str) -> (String, ContentPage) {
        (
            "linux".to_string(),
            ContentPage {
                title: title.to_string(),
                id: id.to_string(),
                href: format!("/blog/linux/{id}"),
            },
        )
    }

    #[test]
    fn splits_date_after_dash() {
        assert_eq!(
            split_trailing_date("TEST-contents - 2025.12.12"),
            Some(("TEST-contents", "2025.12.12"))
        );
        assert_eq!(split_trailing_date("TEST-contents"), None);
    }

    #[test]
    fn date_key_reads_common_separators() {
        assert_eq!(date_key(Some("2025.12.12")), Some((2025, 12, 12)));
        assert_eq!(date_key(Some("2025-06-01")), Some((2025, 6, 1)));
        assert_eq!(date_key(Some("2024/01/02")), Some((2024, 1, 2)));
        assert_eq!(date_key(None), None);
    }

    #[test]
    fn recent_posts_are_the_five_latest_dates() {
        let pages = vec![
            page("old - 2024.01.01", "1"),
            page("mid - 2025.06.01", "2"),
            page("new - 2025.12.12", "3"),
            page("undated", "4"),
            page("newer - 2026.01.01", "5"),
            page("also - 2025.12.13", "6"),
            page("older - 2023.05.05", "7"),
        ];
        let recent = select_recent(pages, 5);
        let titles: Vec<&str> = recent.iter().map(|post| post.title.as_str()).collect();
        assert_eq!(titles, ["newer", "also", "new", "mid", "old"]);
        assert!(recent.iter().all(|post| post.date.is_some()));
    }

    #[test]
    fn undated_posts_fill_after_dated_ones() {
        let pages = vec![
            page("alpha", "1"),
            page("dated - 2025.01.01", "2"),
            page("beta", "3"),
        ];
        let recent = select_recent(pages, 5);
        assert_eq!(
            recent,
            vec![
                RecentPost {
                    title: "dated".into(),
                    date: Some("2025.01.01".into()),
                    href: "/blog/linux/2".into(),
                    tag: "linux".into(),
                },
                RecentPost {
                    title: "alpha".into(),
                    date: None,
                    href: "/blog/linux/1".into(),
                    tag: "linux".into(),
                },
                RecentPost {
                    title: "beta".into(),
                    date: None,
                    href: "/blog/linux/3".into(),
                    tag: "linux".into(),
                },
            ]
        );
    }

    #[test]
    fn duplicate_ids_are_kept_once() {
        let pages = vec![
            page("same - 2025.02.02", "aa"),
            (
                "other".to_string(),
                ContentPage {
                    title: "same - 2025.02.02".into(),
                    id: "aa".into(),
                    href: "/blog/other/aa".into(),
                },
            ),
        ];
        let recent = select_recent(pages, 5);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].tag, "linux");
    }
}
