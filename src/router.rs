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
}

impl fmt::Display for Route {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Parsed location: a route plus an optional trailing slug (`/blog/linux`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Router {
    route: Route,
    slug: Option<String>,
}

impl Router {
    pub const NAV: [Route; 3] = [Route::Intro, Route::Blog, Route::About];

    pub fn parse(path: impl AsRef<str>) -> Self {
        let path = path.as_ref();
        let mut parts = path.trim_start_matches('/').splitn(2, '/');
        let first = parts.next().unwrap_or("");
        let second = parts
            .next()
            .filter(|segment| !segment.is_empty())
            .map(str::to_string);

        Self {
            route: Route::from_segment(first),
            slug: second,
        }
    }

    pub fn route(&self) -> Route {
        self.route
    }

    pub fn slug(&self) -> Option<&str> {
        self.slug.as_deref()
    }

    pub fn title(&self) -> String {
        match (self.route, self.slug()) {
            (Route::Blog, Some(slug)) => format!("Blog -> [{slug}]"),
            (route, _) => route.label().to_string(),
        }
    }

    pub fn tag_href(tag: &str) -> String {
        format!("/blog/{tag}")
    }
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
        assert_eq!(router.title(), "Blog -> [linux]");
    }

    #[test]
    fn parse_unknown_is_not_found() {
        let router = Router::parse("/a");
        assert_eq!(router.route(), Route::NotFound);
        assert_eq!(router.title(), "Err404");
    }

    #[test]
    fn tag_href_is_absolute() {
        assert_eq!(Router::tag_href("gcc"), "/blog/gcc");
    }
}
