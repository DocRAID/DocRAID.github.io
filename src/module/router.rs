use crate::app::App;
use crate::pages::blog_page::blog_page;
use crate::pages::intro_page::intro_page;
use crate::pages::not_found_page::not_found_page;
use ratatui::layout::Rect;
use ratatui::{Frame};

#[derive(Clone, Debug, PartialEq)]
pub enum Pages {
    Intro,  //blog intro page
    About,  //about me & contact page
    Blog,   //blog page
    Err404, //error page
}

impl Pages {
    pub fn page_select(path: String) -> Self {
        match path.as_ref() {
            "" => Pages::Intro,
            "about" => Pages::About,
            "blog" => Pages::Blog,
            _ => Pages::Err404,
        }
    }

    pub fn to_href(&self) -> &str {
        match self {
            Pages::Intro => "/",
            Pages::About => "/about",
            Pages::Blog => "/blog",
            Pages::Err404 => "/err404",
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            Pages::Intro => "Intro".to_owned(),
            Pages::About => "About".to_owned(),
            Pages::Blog => "Blog".to_owned(),
            Pages::Err404 => "Err404".to_owned(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Router {
    pub pages: Pages,
    pub sub_query: Option<String>,
}

impl Router {
    pub fn new(path: String) -> Self {
        let mut parts = path.trim_start_matches('/').splitn(2, '/');
        let first = parts.next().unwrap_or("").to_string();
        let second = parts.next().map(|s| s.to_string());
        Self {
            pages: Pages::page_select(first),
            sub_query: second,
        }
    }

    pub fn label(&self) -> String {
        match self.pages {
            Pages::Intro => "Intro".to_string(),
            Pages::About => "About".to_string(),
            Pages::Blog => {
                if self.sub_query.is_some() {
                    return format!("Blog -> [{}]", self.sub_query.as_ref().unwrap()).to_string();
                }
                "Blog".to_string()
            }
            _ => "Err404".to_string(),
        }
    }
    pub fn nav_bar(&self) -> Vec<Pages> {
        [Pages::Intro, Pages::Blog, Pages::About].to_vec()
    }

    pub fn show_page(&self, frame: &mut Frame, layout: Rect, app: &App) {
        match self.pages {
            Pages::Intro => {
                intro_page(self.label(), frame, layout);
            }
            Pages::About => {}
            Pages::Blog => {
                blog_page(self.label(), frame, layout, app);
            }
            Pages::Err404 => not_found_page(self.label(), frame, layout),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::module::router::Router;

    #[test]
    fn test_path() {
        let router = Router::new("/".to_string());
        assert_eq!(router.label(), "Intro".to_string());
        let router = Router::new("/a".to_string());
        assert_eq!(router.label(), "404".to_string());
    }
}
