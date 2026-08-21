//! WASM TUI blog rendered with Ratatui and Ratzilla.

use std::io;
use std::rc::Rc;

use ratatui::Terminal;
use ratzilla::web_sys::window;
use ratzilla::{DomBackend, WebRenderer};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

use crate::app::App;

mod app;
mod content;
mod module;
mod mouse;
mod router;
mod theme;
mod ui;

fn main() -> io::Result<()> {
    console_log::init().expect("console logger");

    let backend = DomBackend::new()?;
    let terminal = Terminal::new(backend)?;

    let window = window().expect("window");
    let path = window.location().pathname().expect("pathname");
    let state = Rc::new(App::new(path, window.clone()));
    // Static host only: scrape Notion from the browser, not a server.
    crate::content::refresh();

    let pop_state = Rc::clone(&state);
    let on_pop = Closure::<dyn FnMut(web_sys::Event)>::new(move |_event: web_sys::Event| {
        if let Some(path) = current_path() {
            pop_state.set_path(path);
        }
    });
    if let Err(err) =
        window.add_event_listener_with_callback("popstate", on_pop.as_ref().unchecked_ref())
    {
        log::error!("failed to listen for popstate: {err:?}");
    }
    on_pop.forget();

    let mouse_state = Rc::clone(&state);
    terminal.on_mouse_event(move |event| {
        mouse_state.handle_mouse(event);
    });

    let key_state = Rc::clone(&state);
    terminal.on_key_event(move |event| {
        key_state.handle_key(event);
    });

    let wheel_state = Rc::clone(&state);
    let on_wheel =
        Closure::<dyn FnMut(web_sys::WheelEvent)>::new(move |event: web_sys::WheelEvent| {
            event.prevent_default();
            let delta = event.delta_y();
            let mut lines = (delta / 40.0).round() as i32;
            if lines == 0 {
                lines = if delta > 0.0 { 1 } else { -1 };
            }
            wheel_state.scroll_by(lines);
        });
    if let Err(err) =
        window.add_event_listener_with_callback("wheel", on_wheel.as_ref().unchecked_ref())
    {
        log::error!("failed to listen for wheel: {err:?}");
    }
    on_wheel.forget();

    let render_state = Rc::clone(&state);
    terminal.draw_web(move |frame| {
        render_state.render(frame);
    });

    Ok(())
}

fn current_path() -> Option<String> {
    window()?.location().pathname().ok()
}
