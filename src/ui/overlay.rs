//! HTML image overlays positioned over the Ratzilla cell grid.

use crate::mouse;
use ratatui::layout::Rect;
use std::cell::RefCell;
use web_sys::Element;

const CONTAINER_ID: &str = "post-images";
const INNER_BG: &str = "#1c1916";

#[derive(Clone, Debug, PartialEq)]
pub struct ImageSlot {
    pub src: String,
    pub alt: String,
    pub clip: Rect,
    pub x: i32,
    pub y: i32,
    pub width: u16,
    pub height: u16,
}

thread_local! {
    static SLOTS: RefCell<Vec<ImageSlot>> = const { RefCell::new(Vec::new()) };
}

pub fn begin_frame() {
    SLOTS.with(|slots| slots.borrow_mut().clear());
}

pub fn push(slot: ImageSlot) {
    if slot.src.is_empty() || slot.width == 0 || slot.height == 0 {
        return;
    }
    SLOTS.with(|slots| slots.borrow_mut().push(slot));
}

pub fn sync() {
    SLOTS.with(|slots| apply(&slots.borrow()));
}

#[cfg(test)]
pub fn slots() -> Vec<ImageSlot> {
    SLOTS.with(|slots| slots.borrow().clone())
}

fn apply(slots: &[ImageSlot]) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let Some(container) = ensure_container(&document) else {
        return;
    };
    if slots.is_empty() {
        container.set_inner_html("");
        let _ = container.set_attribute("style", "display:none");
        return;
    }

    let clip = slots[0].clip;
    let (left, top, width, height) = mouse::rect_css_px(clip);
    let _ = container.set_attribute(
        "style",
        &format!(
            "display:block;position:fixed;left:{left}px;top:{top}px;width:{width}px;height:{height}px;overflow:hidden;pointer-events:none;z-index:5;"
        ),
    );

    let (cell_w, cell_h) = mouse::cell_size_px();
    let mut child = container.first_element_child();
    for slot in slots {
        let node = match child {
            Some(existing) => existing,
            None => match create_image(&document) {
                Some(created) => {
                    let _ = container.append_child(&created);
                    created
                }
                None => continue,
            },
        };
        update_image(&node, slot, cell_w, cell_h);
        child = node.next_element_sibling();
    }
    while let Some(extra) = child {
        let next = extra.next_element_sibling();
        extra.remove();
        child = next;
    }
}

fn ensure_container(document: &web_sys::Document) -> Option<Element> {
    if let Some(existing) = document.get_element_by_id(CONTAINER_ID) {
        return Some(existing);
    }
    let container = document.create_element("div").ok()?;
    container.set_id(CONTAINER_ID);
    document.body()?.append_child(&container).ok()?;
    Some(container)
}

fn create_image(document: &web_sys::Document) -> Option<Element> {
    let anchor = document.create_element("a").ok()?;
    let _ = anchor.set_attribute("target", "_blank");
    let _ = anchor.set_attribute("rel", "noopener noreferrer");
    let img = document.create_element("img").ok()?;
    let _ = img.set_attribute("referrerpolicy", "no-referrer");
    let _ = img.set_attribute("decoding", "async");
    let _ = img.set_attribute(
        "style",
        &format!("width:100%;height:100%;object-fit:contain;object-position:center;display:block;background:{INNER_BG};"),
    );
    anchor.append_child(&img).ok()?;
    Some(anchor)
}

fn update_image(anchor: &Element, slot: &ImageSlot, cell_w: f32, cell_h: f32) {
    let left = slot.x as f32 * cell_w;
    let top = slot.y as f32 * cell_h;
    let width = f32::from(slot.width) * cell_w;
    let height = f32::from(slot.height) * cell_h;
    let _ = anchor.set_attribute("href", &slot.src);
    let _ = anchor.set_attribute(
        "style",
        &format!(
            "position:absolute;left:{left}px;top:{top}px;width:{width}px;height:{height}px;pointer-events:auto;cursor:pointer;"
        ),
    );
    let Some(img) = anchor.query_selector("img").ok().flatten() else {
        return;
    };
    if img.get_attribute("src").as_deref() != Some(slot.src.as_str()) {
        let _ = img.set_attribute("src", &slot.src);
    }
    let _ = img.set_attribute("alt", &slot.alt);
}
