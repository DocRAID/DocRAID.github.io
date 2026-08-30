use super::notion::{ContentPage, PostSegment, TagSection};
use super::snapshot::{Snapshot, EMBEDDED_SNAPSHOT};
use std::cell::RefCell;
use std::collections::HashMap;

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
    match Snapshot::parse(EMBEDDED_SNAPSHOT) {
        Ok(snapshot) if !snapshot.sections.is_empty() => {
            apply_snapshot(snapshot);
        }
        Ok(_) => set_status(CatalogStatus::Loading),
        Err(err) => {
            log::error!("embedded snapshot: {err}");
            set_status(CatalogStatus::Loading);
        }
    }
}

pub fn apply_snapshot(snapshot: Snapshot) {
    CATALOG.with(|slot| *slot.borrow_mut() = snapshot.sections);
    ABOUT.with(|slot| *slot.borrow_mut() = snapshot.about);
    POSTS.with(|slot| {
        let mut posts = slot.borrow_mut();
        for (id, segments) in snapshot.posts {
            posts.insert(compact_id(&id), PostSlot::Ready(segments));
        }
    });
    set_status(CatalogStatus::Ready);
}

pub fn apply_catalog(catalog: Vec<TagSection>) {
    CATALOG.with(|slot| *slot.borrow_mut() = catalog);
    set_status(CatalogStatus::Ready);
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

pub fn set_about(text: Option<String>) {
    ABOUT.with(|slot| *slot.borrow_mut() = text);
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
    set_status(CatalogStatus::Ready);
}
