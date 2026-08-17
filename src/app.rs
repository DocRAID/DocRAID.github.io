use crate::mouse::{HitMap, MouseState};
use crate::router::Router;
use crate::ui::{self, FrameCtx};
use ratatui::Frame;
use ratzilla::event::{MouseEvent, MouseEventKind};
use ratzilla::web_sys::Window;
use std::cell::RefCell;
use wasm_bindgen::JsValue;

/// Shared application state. Ratzilla's draw/event callbacks are `Fn`,
/// so mutable fields live in [`RefCell`]s.
pub struct App {
    router: RefCell<Router>,
    mouse: RefCell<MouseState>,
    hits: RefCell<HitMap>,
    window: Window,
}

impl App {
    pub fn new(path: impl Into<String>, window: Window) -> Self {
        Self {
            router: RefCell::new(Router::parse(path.into())),
            mouse: RefCell::new(MouseState::default()),
            hits: RefCell::new(HitMap::default()),
            window,
        }
    }

    pub fn set_path(&self, path: impl Into<String>) {
        *self.router.borrow_mut() = Router::parse(path.into());
    }

    pub fn render(&self, frame: &mut Frame<'_>) {
        let router = self.router.borrow();
        let mouse = self.mouse.borrow();
        let mut hits = self.hits.borrow_mut();
        hits.clear();

        let mut ctx = FrameCtx {
            router: &router,
            mouse: &mouse,
            hits: &mut hits,
        };
        ui::render(&mut ctx, frame);
    }

    pub fn handle_mouse(&self, mouse_event: MouseEvent) {
        self.mouse.borrow_mut().update(&mouse_event);
        if mouse_event.event == MouseEventKind::Pressed {
            if let Some(href) = self
                .hits
                .borrow()
                .href_at((mouse_event.x, mouse_event.y))
                .map(str::to_owned)
            {
                self.navigate(&href);
            }
        }
    }

    fn navigate(&self, href: &str) {
        if href.starts_with("https://") || href.starts_with("http://") {
            if let Err(err) = self.window.location().set_href(href) {
                log::error!("failed to navigate to {href}: {err:?}");
            }
            return;
        }

        if let Ok(history) = self.window.history() {
            if let Err(err) = history.push_state_with_url(&JsValue::NULL, "", Some(href)) {
                log::error!("failed to push history {href}: {err:?}");
            }
        }
        self.set_path(href);
    }
}
