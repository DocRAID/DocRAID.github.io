use crate::mouse::{HitAction, HitMap, MouseState};
use crate::router::{Route, Router};
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
    list_selected: RefCell<Option<usize>>,
    nav_items: RefCell<Vec<String>>,
    filter: RefCell<String>,
    filter_open: RefCell<bool>,
    copied_until: RefCell<f64>,
    window: Window,
}

impl App {
    pub fn new(path: impl Into<String>, window: Window) -> Self {
        let app = Self {
            router: RefCell::new(Router::parse(path.into())),
            mouse: RefCell::new(MouseState::default()),
            hits: RefCell::new(HitMap::default()),
            scroll: RefCell::new(0),
            scroll_post: RefCell::new(None),
            content_height: RefCell::new(0),
            viewport_height: RefCell::new(0),
            list_selected: RefCell::new(None),
            nav_items: RefCell::new(Vec::new()),
            filter: RefCell::new(String::new()),
            filter_open: RefCell::new(false),
            copied_until: RefCell::new(0.0),
            window,
        };
        app.sync_document_title();
        app
    }

    pub fn set_path(&self, path: impl Into<String>) {
        let router = Router::parse(path.into());
        let context = scroll_context(&router);
        if self.scroll_post.borrow().as_deref() != context.as_deref() {
            *self.scroll.borrow_mut() = 0;
            *self.scroll_post.borrow_mut() = context;
            *self.list_selected.borrow_mut() = None;
            self.filter.borrow_mut().clear();
            *self.filter_open.borrow_mut() = false;
        }
        *self.router.borrow_mut() = router;
        self.sync_document_title();
    }

    pub fn scroll_by(&self, delta: i32) {
        if *self.filter_open.borrow() {
            return;
        }
        if is_list(&self.router.borrow()) {
            self.move_list(delta);
            return;
        }
        if !is_scrollable(&self.router.borrow()) {
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
        if *self.filter_open.borrow() {
            self.handle_filter_key(&key_event);
            return;
        }

        match key_event.code {
            KeyCode::Char('/')
                if is_list(&self.router.borrow()) && !key_event.ctrl && !key_event.alt =>
            {
                *self.filter_open.borrow_mut() = true;
            }
            KeyCode::Char('j') | KeyCode::Down => self.scroll_by(1),
            KeyCode::Char('k') | KeyCode::Up => self.scroll_by(-1),
            KeyCode::Char('d') | KeyCode::PageDown => self.scroll_by(10),
            KeyCode::Char('u') | KeyCode::PageUp => self.scroll_by(-10),
            KeyCode::Char('g') | KeyCode::Home => self.scroll_home(),
            KeyCode::Char('G') | KeyCode::End => self.scroll_end(),
            KeyCode::Char('1') => self.navigate(Route::Intro.path()),
            KeyCode::Char('2') => self.navigate(Route::Blog.path()),
            KeyCode::Char('3') => self.navigate(Route::About.path()),
            KeyCode::Tab if key_event.shift => {
                let next = self.router.borrow().route().prev();
                self.navigate(next.path());
            }
            KeyCode::Tab => {
                let next = self.router.borrow().route().next();
                self.navigate(next.path());
            }
            KeyCode::Enter => self.activate_selection(),
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace => self.go_back(),
            _ => {}
        }
    }

    pub fn render(&self, frame: &mut Frame<'_>) {
        let hovering = self.paint(frame);
        crate::mouse::sync_hover_cursor(hovering);
    }

    fn paint(&self, frame: &mut Frame<'_>) -> bool {
        let router = self.router.borrow();
        let mouse = self.mouse.borrow();
        let mut hits = self.hits.borrow_mut();
        hits.clear();
        let mut metrics = (0_u16, 0_u16);
        let mut nav_items = self.nav_items.borrow_mut();
        nav_items.clear();
        let filter = self.filter.borrow();
        let copied = js_sys::Date::now() < *self.copied_until.borrow();

        let mut ctx = FrameCtx {
            router: &router,
            mouse: &mouse,
            hits: &mut hits,
            scroll: *self.scroll.borrow(),
            scroll_metrics: Some(&mut metrics),
            list_selected: *self.list_selected.borrow(),
            nav_items: &mut nav_items,
            filter: &filter,
            filter_open: *self.filter_open.borrow(),
            copied,
        };
        ui::render(&mut ctx, frame);
        *self.content_height.borrow_mut() = metrics.0;
        *self.viewport_height.borrow_mut() = metrics.1;
        let max = metrics.0.saturating_sub(metrics.1);
        if *self.scroll.borrow() > max {
            *self.scroll.borrow_mut() = max;
        }
        let len = nav_items.len();
        if let Some(selected) = *self.list_selected.borrow() {
            if len == 0 {
                *self.list_selected.borrow_mut() = None;
            } else if selected >= len {
                *self.list_selected.borrow_mut() = Some(len - 1);
            }
        }
        hits.hovering(mouse.pos())
    }

    pub fn handle_mouse(&self, mouse_event: MouseEvent) {
        self.mouse.borrow_mut().update(&mouse_event);
        self.sync_cursor();
        if mouse_event.event == MouseEventKind::Pressed {
            let action = self
                .hits
                .borrow()
                .action_at((mouse_event.x, mouse_event.y))
                .cloned();
            match action {
                Some(HitAction::Go(href)) => self.navigate(&href),
                Some(HitAction::Copy(text)) => self.copy_to_clipboard(&text),
                None => {}
            }
        }
    }

    fn handle_filter_key(&self, key_event: &KeyEvent) {
        match key_event.code {
            KeyCode::Esc => {
                *self.filter_open.borrow_mut() = false;
                self.filter.borrow_mut().clear();
                *self.list_selected.borrow_mut() = None;
            }
            KeyCode::Enter => {
                *self.filter_open.borrow_mut() = false;
                self.activate_selection();
            }
            KeyCode::Backspace => {
                self.filter.borrow_mut().pop();
                *self.list_selected.borrow_mut() = None;
            }
            KeyCode::Char(ch) if !key_event.ctrl && !key_event.alt => {
                self.filter.borrow_mut().push(ch);
                *self.list_selected.borrow_mut() = None;
            }
            _ => {}
        }
    }

    fn activate_selection(&self) {
        let items = self.nav_items.borrow();
        if items.is_empty() {
            return;
        }
        let index = self
            .list_selected
            .borrow()
            .unwrap_or(0)
            .min(items.len() - 1);
        let href = items[index].clone();
        drop(items);
        self.navigate(&href);
    }

    fn go_back(&self) {
        if *self.filter_open.borrow() {
            *self.filter_open.borrow_mut() = false;
            self.filter.borrow_mut().clear();
            return;
        }
        if let Some(href) = self.router.borrow().parent_href() {
            self.navigate(&href);
        }
    }

    fn scroll_home(&self) {
        if is_list(&self.router.borrow()) {
            if !self.nav_items.borrow().is_empty() {
                *self.list_selected.borrow_mut() = Some(0);
            }
            return;
        }
        if is_scrollable(&self.router.borrow()) {
            *self.scroll.borrow_mut() = 0;
        }
    }

    fn scroll_end(&self) {
        if is_list(&self.router.borrow()) {
            let len = self.nav_items.borrow().len();
            if len > 0 {
                *self.list_selected.borrow_mut() = Some(len - 1);
            }
            return;
        }
        if is_scrollable(&self.router.borrow()) {
            let max =
                (*self.content_height.borrow()).saturating_sub(*self.viewport_height.borrow());
            *self.scroll.borrow_mut() = max;
        }
    }

    fn move_list(&self, delta: i32) {
        let len = self.nav_items.borrow().len();
        if len == 0 {
            return;
        }
        let current = self.list_selected.borrow().unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, (len - 1) as i32) as usize;
        *self.list_selected.borrow_mut() = Some(next);
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

    fn copy_to_clipboard(&self, text: &str) {
        *self.copied_until.borrow_mut() = js_sys::Date::now() + 1_500.0;
        let clipboard = self.window.navigator().clipboard();
        let promise = clipboard.write_text(text);
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(err) = wasm_bindgen_futures::JsFuture::from(promise).await {
                log::error!("clipboard write failed: {err:?}");
            }
        });
    }

    fn sync_cursor(&self) {
        let hovering = self.hits.borrow().hovering(self.mouse.borrow().pos());
        crate::mouse::sync_hover_cursor(hovering);
    }

    fn sync_document_title(&self) {
        let title = self.router.borrow().document_title();
        if let Some(document) = self.window.document() {
            document.set_title(&title);
        }
    }
}

fn is_scrollable(router: &Router) -> bool {
    matches!(
        router.route(),
        Route::Intro | Route::About | Route::NotFound
    ) || router.post().is_some()
}

fn is_list(router: &Router) -> bool {
    router.route() == Route::Blog && router.post().is_none()
}

fn scroll_context(router: &Router) -> Option<String> {
    if matches!(router.route(), Route::Intro) {
        Some("intro".to_string())
    } else if matches!(router.route(), Route::About) {
        Some("about".to_string())
    } else if router.post().is_some() {
        router.post().map(str::to_string)
    } else if router.route() == Route::Blog {
        Some(format!("blog:{}", router.slug().unwrap_or("")))
    } else {
        Some(router.route().path().to_string())
    }
}

fn clamp_scroll(offset: i32, content_height: u16, viewport_height: u16) -> u16 {
    let max = i32::from(content_height.saturating_sub(viewport_height));
    offset.clamp(0, max) as u16
}
