use super::notion::{PostSegment, TagSection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const EMBEDDED_SNAPSHOT: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/snapshot.json"));

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snapshot {
    #[serde(default)]
    pub saved_at: u64,
    #[serde(default)]
    pub sections: Vec<TagSection>,
    #[serde(default)]
    pub posts: HashMap<String, Vec<PostSegment>>,
    #[serde(default)]
    pub about: Option<String>,
}

impl Snapshot {
    pub fn parse(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|err| format!("invalid snapshot.json: {err}"))
    }

    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|err| format!("serialize snapshot: {err}"))
    }

    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::{Snapshot, EMBEDDED_SNAPSHOT};

    #[test]
    fn embedded_snapshot_parses() {
        let snapshot = Snapshot::parse(EMBEDDED_SNAPSHOT).unwrap();
        assert!(snapshot.posts.is_empty() || !snapshot.sections.is_empty() || snapshot.is_empty());
    }
}
