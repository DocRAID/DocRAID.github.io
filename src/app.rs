use crate::mouse::{HitAction, HitMap, MouseState};
use crate::router::Router;
use crate::ui::{self, FrameCtx};
use ratatui::Frame;
use ratzilla::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratzilla::web_sys::Window;
use std::cell::RefCell;
use wasm_bindgen::JsValue;

/// Shared application state. Ratzilla's draw/event callbacks are `Fn`,
/// so mutable fields live in [`RefCell`]s.
pub struct App {
    router: RefCell<Router>,
    mouse: RefCell<MouseState>,
    hits: RefCell<HitMap>,
    scroll: RefCell<u16>,
    scroll_post: RefCell<Option<String>>,
    content_height: RefCell<u16>,
    viewport_height: RefCell<u16>,
    window: Window,
}

impl App {
    pub fn new(path: impl Into<String>, window: Window) -> Self {
        Self {
            router: RefCell::new(Router::parse(path.into())),
            mouse: RefCell::new(MouseState::default()),
            hits: RefCell::new(HitMap::default()),
            scroll: RefCell::new(0),
            scroll_post: RefCell::new(None),
            content_height: RefCell::new(0),
            viewport_height: RefCell::new(0),
            window,
        }
    }

    pub fn set_path(&self, path: impl Into<String>) {
        let router = Router::parse(path.into());
        let post = router.post().map(str::to_string);
        if self.scroll_post.borrow().as_deref() != post.as_deref() {
            *self.scroll.borrow_mut() = 0;
            *self.scroll_post.borrow_mut() = post;
        }
        *self.router.borrow_mut() = router;
    }

    pub fn scroll_by(&self, delta: i32) {
        if self.router.borrow().post().is_none() {
            return;
        }
        let next = i32::from(*self.scroll.borrow()) + delta;
        *self.scroll.borrow_mut() = clamp_scroll(
            next,
            *self.content_height.borrow(),
            *self.viewport_height.borrow(),
        );
    }

    pub fn handle_key(&self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Up => self.scroll_by(-1),
            KeyCode::Down => self.scroll_by(1),
            KeyCode::PageUp => self.scroll_by(-10),
            KeyCode::PageDown => self.scroll_by(10),
            KeyCode::Home => *self.scroll.borrow_mut() = 0,
            KeyCode::End => {
                let max =
                    (*self.content_height.borrow()).saturating_sub(*self.viewport_height.borrow());
                *self.scroll.borrow_mut() = max;
            }
            _ => {}
        }
    }

    pub fn render(&self, frame: &mut Frame<'_>) {
        let router = self.router.borrow();
        let mouse = self.mouse.borrow();
        let mut hits = self.hits.borrow_mut();
        hits.clear();
        let mut metrics = (0_u16, 0_u16);

        let mut ctx = FrameCtx {
            router: &router,
            mouse: &mouse,
            hits: &mut hits,
            scroll: *self.scroll.borrow(),
            scroll_metrics: Some(&mut metrics),
        };
        ui::render(&mut ctx, frame);
        *self.content_height.borrow_mut() = metrics.0;
        *self.viewport_height.borrow_mut() = metrics.1;
        let max = metrics.0.saturating_sub(metrics.1);
        if *self.scroll.borrow() > max {
            *self.scroll.borrow_mut() = max;
        }
    }

    pub fn handle_mouse(&self, mouse_event: MouseEvent) {
        self.mouse.borrow_mut().update(&mouse_event);
        if mouse_event.event == MouseEventKind::Pressed {
            let action = self
                .hits
                .borrow()
                .action_at((mouse_event.x, mouse_event.y))
                .cloned();
            match action {
                Some(HitAction::Go(href)) => self.navigate(&href),
                Some(HitAction::Copy(text)) => copy_to_clipboard(&text),
                None => {}
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

fn clamp_scroll(offset: i32, content_height: u16, viewport_height: u16) -> u16 {
    let max = i32::from(content_height.saturating_sub(viewport_height));
    offset.clamp(0, max) as u16
}

fn copy_to_clipboard(text: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let clipboard = window.navigator().clipboard();
    let promise = clipboard.write_text(text);
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(err) = wasm_bindgen_futures::JsFuture::from(promise).await {
            log::error!("clipboard write failed: {err:?}");
        } else {
            log::info!("copied code block to clipboard");
        }
    });
}
