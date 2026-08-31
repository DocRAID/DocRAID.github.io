use super::notion::{ContentPage, PostSegment, TagSection};
use super::snapshot::Snapshot;
use super::storage;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogStatus {
    Loading,
    Ready,
    Error(String),
}

#[derive(Clone, Debug)]
enum PostSlot {
    Ready(Vec<PostSegment>),
    Failed { message: String, at_ms: u64 },
}

thread_local! {
    static CATALOG: RefCell<Vec<TagSection>> = const { RefCell::new(Vec::new()) };
    static POSTS: RefCell<HashMap<String, PostSlot>> = RefCell::new(HashMap::new());
    static ABOUT: RefCell<Option<String>> = const { RefCell::new(None) };
    static STATUS: RefCell<CatalogStatus> = const { RefCell::new(CatalogStatus::Loading) };
    static BOOTSTRAPPED: RefCell<bool> = const { RefCell::new(false) };
}

pub fn bootstrap() {
    if BOOTSTRAPPED.with(|slot| {
        if *slot.borrow() {
            true
        } else {
            *slot.borrow_mut() = true;
            false
        }
    }) {
        return;
    }
    if let Some(snapshot) = storage::load() {
        apply_snapshot(snapshot);
    } else {
        set_status(CatalogStatus::Loading);
    }
}

pub fn apply_snapshot(snapshot: Snapshot) {
    CATALOG.with(|slot| *slot.borrow_mut() = snapshot.sections);
    ABOUT.with(|slot| *slot.borrow_mut() = snapshot.about);
    POSTS.with(|slot| {
        let mut posts = slot.borrow_mut();
        posts.clear();
        for (id, segments) in snapshot.posts {
            posts.insert(compact_id(&id), PostSlot::Ready(segments));
        }
    });
    set_status(CatalogStatus::Ready);
}

/// Apply a live scrape. Catalog and about are replaced; post bodies that
/// were refetched overwrite the cache; posts no longer in the catalog are
/// dropped. Bodies the scrape failed to refetch stay until the next success.
pub fn merge_live_snapshot(snapshot: Snapshot) {
    let catalog_ids: HashSet<String> = snapshot
        .sections
        .iter()
        .flat_map(|section| section.pages.iter())
        .map(|page| compact_id(&page.id))
        .collect();
    if !snapshot.sections.is_empty() {
        CATALOG.with(|slot| *slot.borrow_mut() = snapshot.sections);
    }
    if snapshot.about.is_some() {
        ABOUT.with(|slot| *slot.borrow_mut() = snapshot.about);
    }
    POSTS.with(|slot| {
        let mut posts = slot.borrow_mut();
        for (id, segments) in snapshot.posts {
            posts.insert(compact_id(&id), PostSlot::Ready(segments));
        }
        if !catalog_ids.is_empty() {
            posts.retain(|id, _| catalog_ids.contains(id));
        }
    });
    set_status(CatalogStatus::Ready);
}

pub fn export_snapshot(saved_at: u64) -> Snapshot {
    let sections = CATALOG.with(|slot| slot.borrow().clone());
    let about = ABOUT.with(|slot| slot.borrow().clone());
    let posts = POSTS.with(|slot| {
        slot.borrow()
            .iter()
            .filter_map(|(id, post)| match post {
                PostSlot::Ready(segments) => Some((id.clone(), segments.clone())),
                PostSlot::Failed { .. } => None,
            })
            .collect()
    });
    Snapshot {
        saved_at,
        sections,
        posts,
        about,
    }
}

pub fn set_status(status: CatalogStatus) {
    STATUS.with(|slot| *slot.borrow_mut() = status);
}

pub fn status() -> CatalogStatus {
    STATUS.with(|slot| slot.borrow().clone())
}

pub fn about() -> Option<String> {
    ABOUT.with(|slot| slot.borrow().clone())
}

pub fn catalog_is_empty() -> bool {
    CATALOG.with(|catalog| catalog.borrow().is_empty())
}

pub fn current_tags() -> Vec<String> {
    CATALOG.with(|catalog| {
        catalog
            .borrow()
            .iter()
            .map(|section| section.tag.clone())
            .collect()
    })
}

pub fn current_posts(slug: Option<&str>) -> Vec<ContentPage> {
    CATALOG.with(|catalog| {
        let catalog = catalog.borrow();
        match slug {
            None => catalog
                .iter()
                .flat_map(|section| {
                    section.pages.iter().map(|page| ContentPage {
                        title: format!("{} — {}", section.tag, page.title),
                        id: page.id.clone(),
                        href: page.href.clone(),
                    })
                })
                .collect(),
            Some(slug) => catalog
                .iter()
                .filter(|section| super::notion::tag_slug(&section.tag) == slug)
                .flat_map(|section| section.pages.iter().cloned())
                .collect(),
        }
    })
}

pub fn current_tagged_posts() -> Vec<(String, ContentPage)> {
    CATALOG.with(|catalog| {
        catalog
            .borrow()
            .iter()
            .flat_map(|section| {
                section
                    .pages
                    .iter()
                    .cloned()
                    .map(|page| (section.tag.clone(), page))
            })
            .collect()
    })
}

pub fn post_ready(page_id: &str) -> bool {
    let id = compact_id(page_id);
    POSTS.with(|posts| matches!(posts.borrow().get(&id), Some(PostSlot::Ready(_))))
}

pub fn failed_at(page_id: &str) -> Option<(String, u64)> {
    let id = compact_id(page_id);
    POSTS.with(|posts| match posts.borrow().get(&id) {
        Some(PostSlot::Failed { message, at_ms }) => Some((message.clone(), *at_ms)),
        _ => None,
    })
}

pub fn insert_post(page_id: &str, segments: Vec<PostSegment>) {
    let id = compact_id(page_id);
    POSTS.with(|posts| {
        posts.borrow_mut().insert(id, PostSlot::Ready(segments));
    });
}

pub fn insert_post_failure(page_id: &str, message: String, at_ms: u64) {
    let id = compact_id(page_id);
    POSTS.with(|posts| {
        posts
            .borrow_mut()
            .insert(id, PostSlot::Failed { message, at_ms });
    });
}

pub fn current_post_state(page_id: &str) -> Option<Result<Vec<PostSegment>, String>> {
    let id = compact_id(page_id);
    POSTS.with(|posts| match posts.borrow().get(&id) {
        Some(PostSlot::Ready(segments)) => Some(Ok(segments.clone())),
        Some(PostSlot::Failed { message, .. }) => Some(Err(message.clone())),
        None => None,
    })
}

pub fn compact_id(page_id: &str) -> String {
    page_id.replace('-', "")
}

#[cfg(test)]
pub fn set_catalog_for_tests(catalog: Vec<TagSection>) {
    BOOTSTRAPPED.with(|slot| *slot.borrow_mut() = true);
    CATALOG.with(|slot| *slot.borrow_mut() = catalog);
    POSTS.with(|slot| slot.borrow_mut().clear());
    ABOUT.with(|slot| *slot.borrow_mut() = None);
    set_status(CatalogStatus::Ready);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::notion::{ContentPage, PostSegment, TagSection};
    use std::collections::HashMap;

    fn page(tag: &str, id: &str) -> TagSection {
        TagSection {
            tag: tag.to_string(),
            pages: vec![ContentPage {
                title: "post".to_string(),
                id: id.to_string(),
                href: format!("/blog/{tag}/{id}"),
            }],
        }
    }

    #[test]
    fn export_snapshot_skips_failed_posts() {
        set_catalog_for_tests(vec![page("Rust", "aa")]);
        insert_post("aa", vec![PostSegment::Text("ok".into())]);
        insert_post_failure("bb", "nope".into(), 1);
        let snapshot = export_snapshot(9);
        assert_eq!(snapshot.saved_at, 9);
        assert_eq!(snapshot.sections.len(), 1);
        assert_eq!(
            snapshot.posts.get("aa"),
            Some(&vec![PostSegment::Text("ok".into())])
        );
        assert!(!snapshot.posts.contains_key("bb"));
    }

    #[test]
    fn merge_live_snapshot_replaces_refetched_posts() {
        set_catalog_for_tests(vec![page("Rust", "aa")]);
        insert_post("aa", vec![PostSegment::Text("cached".into())]);
        merge_live_snapshot(Snapshot {
            saved_at: 2,
            sections: vec![page("Rust", "aa")],
            posts: HashMap::from([("aa".to_string(), vec![PostSegment::Text("fresh".into())])]),
            about: Some("about".into()),
        });
        assert_eq!(
            current_post_state("aa"),
            Some(Ok(vec![PostSegment::Text("fresh".into())]))
        );
        assert_eq!(about().as_deref(), Some("about"));
    }

    #[test]
    fn merge_live_snapshot_keeps_unrefetched_catalog_posts() {
        set_catalog_for_tests(vec![page("Rust", "aa"), page("Linux", "cc")]);
        insert_post("aa", vec![PostSegment::Text("cached".into())]);
        merge_live_snapshot(Snapshot {
            saved_at: 2,
            sections: vec![page("Rust", "aa"), page("Linux", "cc")],
            posts: HashMap::from([("cc".to_string(), vec![PostSegment::Text("fresh".into())])]),
            about: None,
        });
        assert_eq!(
            current_post_state("aa"),
            Some(Ok(vec![PostSegment::Text("cached".into())]))
        );
        assert_eq!(
            current_post_state("cc"),
            Some(Ok(vec![PostSegment::Text("fresh".into())]))
        );
    }

    #[test]
    fn merge_live_snapshot_drops_removed_posts() {
        set_catalog_for_tests(vec![page("Rust", "aa")]);
        insert_post("aa", vec![PostSegment::Text("cached".into())]);
        merge_live_snapshot(Snapshot {
            saved_at: 2,
            sections: vec![page("Linux", "cc")],
            posts: HashMap::from([("cc".to_string(), vec![PostSegment::Text("fresh".into())])]),
            about: Some("about".into()),
        });
        assert_eq!(current_tags(), vec!["Linux".to_string()]);
        assert_eq!(current_post_state("aa"), None);
        assert_eq!(
            current_post_state("cc"),
            Some(Ok(vec![PostSegment::Text("fresh".into())]))
        );
    }
}
