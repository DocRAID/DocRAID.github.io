use super::notion::{PostSegment, TagSection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
        serde_json::from_str(json).map_err(|err| format!("invalid snapshot: {err}"))
    }

    pub fn to_json_compact(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|err| format!("serialize snapshot: {err}"))
    }

    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }

    pub fn has_content(&self) -> bool {
        !self.sections.is_empty() || self.about.is_some() || !self.posts.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::Snapshot;

    #[test]
    fn parse_rejects_invalid_json() {
        assert!(Snapshot::parse("not json").is_err());
    }

    #[test]
    fn has_content_detects_posts_or_about() {
        let empty = Snapshot::default();
        assert!(!empty.has_content());
        assert!(empty.is_empty());

        let with_about = Snapshot {
            about: Some("hi".into()),
            ..Snapshot::default()
        };
        assert!(with_about.has_content());

        let json = with_about.to_json_compact().unwrap();
        assert!(!json.contains('\n'));
        assert_eq!(Snapshot::parse(&json).unwrap(), with_about);
    }
}
