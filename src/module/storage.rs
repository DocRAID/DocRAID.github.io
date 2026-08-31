//! Browser `localStorage` cache of the Notion catalog and post bodies.

use super::snapshot::Snapshot;
use std::cell::RefCell;

const KEY: &str = "tui_blog.snapshot";

thread_local! {
    /// `true` until a read/write proves `localStorage` cannot be used.
    static USABLE: RefCell<bool> = const { RefCell::new(true) };
}

/// Cached snapshot if `localStorage` has usable catalog or post content.
pub fn load() -> Option<Snapshot> {
    let storage = local_storage()?;
    let json = match storage.get_item(KEY) {
        Ok(Some(json)) => json,
        Ok(None) => return None,
        Err(err) => {
            log::warn!("localStorage read failed: {err:?}");
            mark_unusable();
            return None;
        }
    };
    match parse_cached(&json) {
        Some(snapshot) => Some(snapshot),
        None => {
            let _ = storage.remove_item(KEY);
            None
        }
    }
}

/// Persist the catalog and post bodies. Returns `false` when storage cannot
/// hold the payload; later saves are skipped for this page session.
pub fn save(snapshot: &Snapshot) -> bool {
    if !usable() {
        return false;
    }
    if !snapshot.has_content() {
        return true;
    }
    let Some(storage) = local_storage() else {
        mark_unusable();
        return false;
    };
    let json = match snapshot.to_json_compact() {
        Ok(json) => json,
        Err(err) => {
            log::warn!("{err}");
            return false;
        }
    };
    match storage.set_item(KEY, &json) {
        Ok(()) => true,
        Err(err) => {
            log::warn!("localStorage save failed: {err:?}");
            mark_unusable();
            false
        }
    }
}

pub fn usable() -> bool {
    USABLE.with(|slot| *slot.borrow())
}

pub fn parse_cached(json: &str) -> Option<Snapshot> {
    Snapshot::parse(json).ok().filter(Snapshot::has_content)
}

fn local_storage() -> Option<web_sys::Storage> {
    if !usable() {
        return None;
    }
    let window = web_sys::window()?;
    match window.local_storage() {
        Ok(Some(storage)) => Some(storage),
        Ok(None) | Err(_) => {
            mark_unusable();
            None
        }
    }
}

fn mark_unusable() {
    USABLE.with(|slot| *slot.borrow_mut() = false);
}

#[cfg(test)]
mod tests {
    use super::{parse_cached, KEY};
    use crate::module::notion::{ContentPage, PostSegment, TagSection};
    use crate::module::snapshot::Snapshot;
    use std::collections::HashMap;

    #[test]
    fn storage_key_is_stable() {
        assert_eq!(KEY, "tui_blog.snapshot");
    }

    #[test]
    fn parse_cached_rejects_empty_and_invalid() {
        assert!(parse_cached("not json").is_none());
        assert!(parse_cached("{}").is_none());
        assert!(parse_cached(r#"{"sections":[],"posts":{},"about":null}"#).is_none());
    }

    #[test]
    fn parse_cached_keeps_catalog_or_posts() {
        let snapshot = Snapshot {
            saved_at: 1,
            sections: vec![TagSection {
                tag: "Rust".into(),
                pages: vec![ContentPage {
                    title: "hi".into(),
                    id: "aa".into(),
                    href: "/blog/Rust/aa".into(),
                }],
            }],
            posts: HashMap::from([("aa".to_string(), vec![PostSegment::Text("body".into())])]),
            about: None,
        };
        let json = snapshot.to_json_compact().unwrap();
        let loaded = parse_cached(&json).unwrap();
        assert_eq!(loaded.sections.len(), 1);
        assert_eq!(loaded.posts["aa"], vec![PostSegment::Text("body".into())]);
    }
}
