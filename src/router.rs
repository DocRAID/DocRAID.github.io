use std::fmt;

/// Top-level site destinations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Intro,
    About,
    Blog,
    NotFound,
}

impl Route {
    pub fn from_segment(segment: &str) -> Self {
        match segment {
            "" => Self::Intro,
            "about" => Self::About,
            "blog" => Self::Blog,
            _ => Self::NotFound,
        }
    }

    pub const fn path(self) -> &'static str {
        match self {
            Self::Intro => "/",
            Self::About => "/about",
            Self::Blog => "/blog",
            Self::NotFound => "/404",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Intro => "Intro",
            Self::About => "About",
            Self::Blog => "Blog",
            Self::NotFound => "Err404",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Intro => Self::Blog,
            Self::Blog => Self::About,
            Self::About | Self::NotFound => Self::Intro,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Intro | Self::NotFound => Self::About,
            Self::Blog => Self::Intro,
            Self::About => Self::Blog,
        }
    }
}

impl fmt::Display for Route {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Parsed location: `/blog/{tag}` or `/blog/{tag}/{post}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Router {
    route: Route,
    slug: Option<String>,
    post: Option<String>,
}

impl Router {
    pub const NAV: [Route; 3] = [Route::Intro, Route::Blog, Route::About];

    pub fn parse(path: impl AsRef<str>) -> Self {
        let path = path.as_ref();
        let mut parts = path.trim_start_matches('/').split('/');
        let first = percent_decode(parts.next().unwrap_or(""));
        let second = parts
            .next()
            .filter(|segment| !segment.is_empty())
            .map(percent_decode);
        let third = parts
            .next()
            .filter(|segment| !segment.is_empty())
            .map(percent_decode);
        let extra = parts.next().filter(|segment| !segment.is_empty());

        let route = if extra.is_some() {
            Route::NotFound
        } else {
            Route::from_segment(&first)
        };

        Self {
            route,
            slug: second,
            post: third,
        }
    }

    pub fn route(&self) -> Route {
        self.route
    }

    pub fn slug(&self) -> Option<&str> {
        self.slug.as_deref()
    }

    pub fn post(&self) -> Option<&str> {
        self.post.as_deref()
    }

    pub fn title(&self) -> String {
        match (self.route, self.slug()) {
            (Route::Blog, Some(slug)) => format!("Blog -> [{slug}]"),
            (route, _) => route.label().to_string(),
        }
    }

    pub fn document_title(&self) -> String {
        match (self.route, self.slug(), self.post()) {
            (Route::Intro, _, _) => "Dongju Lim — tui_blog".to_string(),
            (Route::About, _, _) => "About — tui_blog".to_string(),
            (Route::Blog, Some(slug), Some(_)) => format!("{slug} — tui_blog"),
            (Route::Blog, Some(slug), None) => format!("{slug} — tui_blog"),
            (Route::Blog, None, _) => "Blog — tui_blog".to_string(),
            (Route::NotFound, _, _) => "Not found — tui_blog".to_string(),
        }
    }

    pub fn parent_href(&self) -> Option<String> {
        match (self.route, self.slug(), self.post()) {
            (Route::Blog, Some(slug), Some(_)) => Some(Self::tag_href(slug)),
            (Route::Blog, Some(_), None) => Some("/blog".to_string()),
            (Route::Blog, None, _) => Some("/".to_string()),
            (Route::About | Route::NotFound, _, _) => Some("/".to_string()),
            (Route::Intro, _, _) => None,
        }
    }

    pub fn tag_href(tag: &str) -> String {
        format!("/blog/{}", crate::module::scraper::tag_slug(tag))
    }
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(value) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                out.push(value);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| input.to_string())
}

#[cfg(test)]
mod tests {
    use super::{Route, Router};

    #[test]
    fn parse_root_is_intro() {
        let router = Router::parse("/");
        assert_eq!(router.route(), Route::Intro);
        assert_eq!(router.slug(), None);
        assert_eq!(router.title(), "Intro");
    }

    #[test]
    fn parse_empty_is_intro() {
        let router = Router::parse("");
        assert_eq!(router.route(), Route::Intro);
    }

    #[test]
    fn parse_blog_with_tag() {
        let router = Router::parse("/blog/linux");
        assert_eq!(router.route(), Route::Blog);
        assert_eq!(router.slug(), Some("linux"));
        assert_eq!(router.post(), None);
        assert_eq!(router.title(), "Blog -> [linux]");
    }

    #[test]
    fn parse_blog_with_post() {
        let router = Router::parse("/blog/TEST1/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa5");
        assert_eq!(router.route(), Route::Blog);
        assert_eq!(router.slug(), Some("TEST1"));
        assert_eq!(router.post(), Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa5"));
    }

    #[test]
    fn parse_unknown_is_not_found() {
        let router = Router::parse("/a");
        assert_eq!(router.route(), Route::NotFound);
        assert_eq!(router.title(), "Err404");
    }

    #[test]
    fn extra_segments_are_not_found() {
        let router = Router::parse("/blog/linux/id/extra");
        assert_eq!(router.route(), Route::NotFound);
    }

    #[test]
    fn percent_decodes_segments() {
        let router = Router::parse("/blog/C%2FC%2B%2B");
        assert_eq!(router.slug(), Some("C/C++"));
    }

    #[test]
    fn tag_href_is_absolute() {
        assert_eq!(Router::tag_href("gcc"), "/blog/gcc");
        assert_eq!(Router::tag_href("C/C++"), "/blog/C-C++");
    }
}
