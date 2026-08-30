//! WASM TUI blog rendered with Ratatui and Ratzilla.

pub mod app;
pub mod content;
pub mod module;
pub mod mouse;
pub mod router;
pub mod theme;
pub mod ui;
pub mod width;

use std::io;
use std::rc::Rc;

use ratatui::Terminal;
use ratzilla::web_sys::window;
use ratzilla::{DomBackend, WebRenderer};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::AddEventListenerOptions;

use crate::app::App;
use crate::mouse::refresh_cell_metrics;

pub fn start() -> io::Result<()> {
    console_log::init().expect("console logger");

    let backend = DomBackend::new()?;
    let terminal = Terminal::new(backend)?;

    let window = window().expect("window");
    let path = window.location().pathname().expect("pathname");
    let state = Rc::new(App::new(path, window.clone()));
    crate::content::refresh();
    refresh_cell_metrics();

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
        refresh_cell_metrics();
        mouse_state.handle_mouse(event);
    });

    let key_state = Rc::clone(&state);
    terminal.on_key_event(move |event| {
        key_state.handle_key(event);
    });

    let passive_false = AddEventListenerOptions::new();
    passive_false.set_passive(false);

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
    if let Err(err) = window.add_event_listener_with_callback_and_add_event_listener_options(
        "wheel",
        on_wheel.as_ref().unchecked_ref(),
        &passive_false,
    ) {
        log::error!("failed to listen for wheel: {err:?}");
    }
    on_wheel.forget();

    let touch_state = Rc::clone(&state);
    let last_y = Rc::new(std::cell::Cell::new(None::<f64>));
    let last_y_start = Rc::clone(&last_y);
    let on_touch_start =
        Closure::<dyn FnMut(web_sys::TouchEvent)>::new(move |event: web_sys::TouchEvent| {
            if let Some(touch) = event.touches().item(0) {
                last_y_start.set(Some(touch.client_y() as f64));
            }
        });
    let last_y_move = Rc::clone(&last_y);
    let on_touch_move =
        Closure::<dyn FnMut(web_sys::TouchEvent)>::new(move |event: web_sys::TouchEvent| {
            let Some(touch) = event.touches().item(0) else {
                return;
            };
            let y = touch.client_y() as f64;
            if let Some(prev) = last_y_move.get() {
                let delta_px = prev - y;
                let cell_h = f64::from(crate::mouse::cell_height_px()).max(1.0);
                let lines = (delta_px / cell_h).round() as i32;
                if lines != 0 {
                    event.prevent_default();
                    touch_state.scroll_by(lines);
                    last_y_move.set(Some(y));
                }
            } else {
                last_y_move.set(Some(y));
            }
        });
    let last_y_end = Rc::clone(&last_y);
    let on_touch_end = Closure::<dyn FnMut(web_sys::TouchEvent)>::new(move |_event| {
        last_y_end.set(None);
    });
    for (name, closure) in [
        ("touchstart", &on_touch_start),
        ("touchmove", &on_touch_move),
        ("touchend", &on_touch_end),
        ("touchcancel", &on_touch_end),
    ] {
        if let Err(err) = window.add_event_listener_with_callback_and_add_event_listener_options(
            name,
            closure.as_ref().unchecked_ref(),
            &passive_false,
        ) {
            log::error!("failed to listen for {name}: {err:?}");
        }
    }
    on_touch_start.forget();
    on_touch_move.forget();
    on_touch_end.forget();

    let on_resize = Closure::<dyn FnMut(web_sys::Event)>::new(move |_event: web_sys::Event| {
        refresh_cell_metrics();
    });
    if let Err(err) =
        window.add_event_listener_with_callback("resize", on_resize.as_ref().unchecked_ref())
    {
        log::error!("failed to listen for resize: {err:?}");
    }
    on_resize.forget();

    let render_state = Rc::clone(&state);
    terminal.draw_web(move |frame| {
        refresh_cell_metrics();
        render_state.render(frame);
    });

    Ok(())
}

fn current_path() -> Option<String> {
    window()?.location().pathname().ok()
}
