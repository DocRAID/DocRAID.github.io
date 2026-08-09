use std::{io, rc::Rc};

use ratatui::{Terminal};

use ratzilla::{web_sys::window, DomBackend, WebRenderer};

use crate::app::App;

mod app;
mod module;
mod pages;

fn main() -> io::Result<()> {
    console_log::init().unwrap();
    let backend = DomBackend::new()?;
    let terminal = Terminal::new(backend)?;

    let window = window().expect("No window");
    let path = window.location().pathname().expect("No path");

    let state = Rc::new(App::new(path, window));

    let key_event_state = Rc::clone(&state);
    let mouse_event_state = Rc::clone(&state);
    terminal.on_key_event(move |key_event| {
        key_event_state.key_handle_events(key_event);
    });

    terminal.on_mouse_event(move |mouse_event| {
        mouse_event_state.mouse_handle_events(mouse_event);
    });

    let render_state = Rc::clone(&state);
    terminal.draw_web(move |frame| {
        render_state.render(frame);
    });

    Ok(())
}
