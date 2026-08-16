use serde::Deserialize;

/// Embedded copy used when the same-origin `notion.json` cannot be fetched.
pub const EMBEDDED_CONFIG: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/notion.json"));

/// Root-absolute so `/blog/linux` does not resolve this to `/blog/notion.json`.
pub const CONFIG_PATH: &str = "/notion.json";

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct NotionConfig {
    pub pages: Vec<NotionPage>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct NotionPage {
    pub name: String,
    pub url: String,
    pub role: String,
}

impl NotionConfig {
    pub fn parse(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|err| format!("invalid notion.json: {err}"))
    }

    pub fn tag_pages(&self) -> impl Iterator<Item = &NotionPage> {
        self.pages.iter().filter(|page| page.role == "tags")
    }
}

/// Pull the 32-digit Notion page id out of a public site URL.
pub fn page_id_from_url(url: &str) -> Option<String> {
    let last = url.split(['/', '?']).rfind(|segment| !segment.is_empty())?;
    let hex: String = last.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() < 32 {
        return None;
    }
    let id = hex.get(hex.len() - 32..)?;
    Some(format!(
        "{}-{}-{}-{}-{}",
        &id[0..8],
        &id[8..12],
        &id[12..16],
        &id[16..20],
        &id[20..32]
    ))
}

#[cfg(test)]
mod tests {
    use super::{page_id_from_url, NotionConfig};

    #[test]
    fn config_path_is_root_absolute() {
        assert!(super::CONFIG_PATH.starts_with('/'));
    }

    #[test]
    fn parses_embedded_config() {
        let config = NotionConfig::parse(super::EMBEDDED_CONFIG).unwrap();
        assert!(!config.pages.is_empty());
        assert!(config.tag_pages().next().is_some());
    }

    #[test]
    fn extracts_id_from_public_url() {
        let url = "https://limdongju.notion.site/158ec5eb3d2280218426f12e40729e48";
        assert_eq!(
            page_id_from_url(url).as_deref(),
            Some("158ec5eb-3d22-8021-8426-f12e40729e48")
        );
    }

    #[test]
    fn extracts_id_from_slug_url() {
        let url = "https://limdongju.notion.site/Linux-158ec5eb3d22807ea341c9e5604113c3?pvs=25";
        assert_eq!(
            page_id_from_url(url).as_deref(),
            Some("158ec5eb-3d22-807e-a341-c9e5604113c3")
        );
    }
}
