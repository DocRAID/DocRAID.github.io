//! WASM TUI blog rendered with Ratatui and Ratzilla.

use std::io;
use std::rc::Rc;

use ratatui::Terminal;
use ratzilla::web_sys::window;
use ratzilla::{DomBackend, WebRenderer};

use crate::app::App;

mod app;
mod content;
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
    let state = Rc::new(App::new(path, window));

    let mouse_state = Rc::clone(&state);
    terminal.on_mouse_event(move |event| {
        mouse_state.handle_mouse(event);
    });

    let render_state = Rc::clone(&state);
    terminal.draw_web(move |frame| {
        render_state.render(frame);
    });

    Ok(())
}
