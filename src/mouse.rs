use crate::width::display_width;
use ratatui::layout::Rect;
use ratzilla::event::MouseEvent;
use std::cell::{Cell, RefCell};
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

/// Fallback cell size used before the DOM grid can be measured.
/// Matches Ratzilla's `get_window_size` heuristic (innerWidth/10 × innerHeight/20).
const FALLBACK_CELL_WIDTH_PX: f32 = 10.0;
const FALLBACK_CELL_HEIGHT_PX: f32 = 20.0;

#[derive(Clone, Copy, Debug)]
struct CellMetrics {
    origin_x: f32,
    origin_y: f32,
    cell_w: f32,
    cell_h: f32,
}

impl Default for CellMetrics {
    fn default() -> Self {
        Self {
            origin_x: 0.0,
            origin_y: 0.0,
            cell_w: FALLBACK_CELL_WIDTH_PX,
            cell_h: FALLBACK_CELL_HEIGHT_PX,
        }
    }
}

thread_local! {
    static METRICS: RefCell<CellMetrics> = const { RefCell::new(CellMetrics {
        origin_x: 0.0,
        origin_y: 0.0,
        cell_w: FALLBACK_CELL_WIDTH_PX,
        cell_h: FALLBACK_CELL_HEIGHT_PX,
    }) };
    static CURSOR: Cell<&'static str> = const { Cell::new("default") };
}

pub fn cell_height_px() -> f32 {
    METRICS.with(|slot| slot.borrow().cell_h)
}

/// Measure the Ratzilla `#grid` so hit-testing tracks zoom, font, and centering.
pub fn refresh_cell_metrics() {
    let Some(metrics) = measure_grid() else {
        return;
    };
    if metrics.cell_w < 1.0 || metrics.cell_h < 1.0 {
        return;
    }
    METRICS.with(|slot| *slot.borrow_mut() = metrics);
}

fn measure_grid() -> Option<CellMetrics> {
    let document = web_sys::window()?.document()?;
    let grid = document.get_element_by_id("grid")?;
    let grid_rect = grid.get_bounding_client_rect();
    let pre = grid
        .query_selector("pre")
        .ok()
        .flatten()
        .or_else(|| grid.first_element_child())?;
    let pre_rect = pre.get_bounding_client_rect();
    let span = pre.query_selector("span").ok().flatten();
    let cell_w = span
        .map(|el| el.get_bounding_client_rect().width() as f32)
        .filter(|w| *w > 1.0)
        .unwrap_or(FALLBACK_CELL_WIDTH_PX);
    let cell_h = if pre_rect.height() > 1.0 {
        pre_rect.height() as f32
    } else {
        FALLBACK_CELL_HEIGHT_PX
    };
    Some(CellMetrics {
        origin_x: grid_rect.left() as f32,
        origin_y: grid_rect.top() as f32,
        cell_w,
        cell_h,
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MouseState {
    pos: (u32, u32),
}

impl MouseState {
    pub fn update(&mut self, event: &MouseEvent) {
        self.pos = (event.x, event.y);
    }

    pub fn pos(&self) -> (u32, u32) {
        self.pos
    }

    pub fn hits(&self, span: CellSpan) -> bool {
        span.contains(self.pos)
    }

    pub fn hits_rect(&self, area: Rect) -> bool {
        rect_span(area).contains(self.pos)
    }
}

/// Cell-space box used for hover and click tests.
///
/// `x1`/`y1` are exclusive far-edge cell indices (same as a Ratatui `Rect`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellSpan {
    pub x0: u16,
    pub x1: u16,
    pub y0: u16,
    pub y1: u16,
}

impl CellSpan {
    pub fn new(x0: u16, x1: u16, y0: u16, y1: u16) -> Self {
        Self { x0, x1, y0, y1 }
    }

    pub fn contains(self, mouse: (u32, u32)) -> bool {
        let metrics = METRICS.with(|slot| *slot.borrow());
        let px0 = metrics.origin_x + f32::from(self.x0) * metrics.cell_w;
        let px1 = metrics.origin_x + f32::from(self.x1) * metrics.cell_w;
        let py0 = metrics.origin_y + f32::from(self.y0) * metrics.cell_h;
        let py1 = metrics.origin_y + f32::from(self.y1) * metrics.cell_h;
        let x = mouse.0 as f32;
        let y = mouse.1 as f32;
        x >= px0 && x < px1 && y >= py0 && y < py1
    }
}

pub fn rect_span(area: Rect) -> CellSpan {
    CellSpan::new(
        area.x,
        area.x.saturating_add(area.width),
        area.y,
        area.y.saturating_add(area.height),
    )
}

/// Clickable regions registered during render, queried on the next press.
#[derive(Clone, Debug, Default)]
pub struct HitMap {
    regions: Vec<HitTarget>,
}

#[derive(Clone, Debug)]
pub enum HitAction {
    Go(String),
    Copy(String),
}

#[derive(Clone, Debug)]
struct HitTarget {
    span: CellSpan,
    action: HitAction,
}

impl HitMap {
    pub fn clear(&mut self) {
        self.regions.clear();
    }

    pub fn add(&mut self, span: CellSpan, href: impl Into<String>) {
        self.regions.push(HitTarget {
            span,
            action: HitAction::Go(href.into()),
        });
    }

    pub fn add_copy(&mut self, span: CellSpan, text: impl Into<String>) {
        self.regions.push(HitTarget {
            span,
            action: HitAction::Copy(text.into()),
        });
    }

    pub fn action_at(&self, mouse: (u32, u32)) -> Option<&HitAction> {
        self.regions
            .iter()
            .rev()
            .find(|region| region.span.contains(mouse))
            .map(|region| &region.action)
    }

    pub fn hovering(&self, mouse: (u32, u32)) -> bool {
        self.action_at(mouse).is_some()
    }

    #[cfg(test)]
    pub fn href_at(&self, mouse: (u32, u32)) -> Option<&str> {
        match self.action_at(mouse) {
            Some(HitAction::Go(href)) => Some(href.as_str()),
            _ => None,
        }
    }

    #[cfg(test)]
    pub fn hrefs(&self) -> Vec<&str> {
        self.regions
            .iter()
            .filter_map(|region| match &region.action {
                HitAction::Go(href) => Some(href.as_str()),
                HitAction::Copy(_) => None,
            })
            .collect()
    }
}

/// Inclusive cell-x ranges for a centered row of labeled buttons.
const HEADER_HIT_NUDGE: usize = 2;
const BUTTON_PAD: usize = 4; // `" [{}] "` around the label

pub fn header_button_ranges(area: Rect, labels: &[&str]) -> Vec<(u16, u16)> {
    let widths: Vec<usize> = labels
        .iter()
        .map(|label| display_width(label).saturating_add(BUTTON_PAD))
        .collect();
    let total: usize = widths.iter().sum();
    let mut offset = (area.width as usize / 2)
        .saturating_sub(total / 2)
        .saturating_add(HEADER_HIT_NUDGE)
        .saturating_add(area.x as usize);

    let mut ranges = Vec::with_capacity(widths.len());
    for width in widths {
        let start = offset;
        let end = offset.saturating_add(width);
        ranges.push((start as u16, end as u16));
        offset = end;
    }
    ranges
}

/// Vertical cell span of the `index`th list row inside a bordered list.
pub fn list_row_y(area: Rect, index: usize, height: u16) -> (u16, u16) {
    let top = area.y.saturating_add(1).saturating_add(index as u16);
    (top, top.saturating_add(height))
}

/// Show a pointer over clickable hits (nav, lists, cards, copy).
pub fn sync_hover_cursor(hovering: bool) {
    let next = if hovering { "pointer" } else { "default" };
    if CURSOR.with(|slot| {
        if slot.get() == next {
            true
        } else {
            slot.set(next);
            false
        }
    }) {
        return;
    }
    apply_cursor(next);
}

fn apply_cursor(name: &str) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    for element in [
        document.document_element(),
        document.body().map(Into::into),
        document.get_element_by_id("grid"),
    ]
    .into_iter()
    .flatten()
    {
        if let Ok(html) = element.dyn_into::<HtmlElement>() {
            let _ = html.style().set_property("cursor", name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{header_button_ranges, list_row_y, CellSpan, HitMap};
    use ratatui::layout::Rect;

    #[test]
    fn origin_cell_contains_origin_pixel() {
        let span = CellSpan::new(0, 1, 0, 1);
        assert!(span.contains((0, 0)));
        assert!(!span.contains((1_000, 0)));
    }

    #[test]
    fn header_ranges_are_contiguous_and_nudged() {
        let area = Rect::new(0, 0, 80, 3);
        let ranges = header_button_ranges(area, &["Intro", "Blog", "About"]);
        assert_eq!(ranges.len(), 3);
        assert!(ranges[0].0 < ranges[0].1);
        assert_eq!(ranges[1].0, ranges[0].1);
        assert_eq!(ranges[2].0, ranges[1].1);
    }

    #[test]
    fn list_row_skips_the_border() {
        let area = Rect::new(0, 3, 20, 10);
        assert_eq!(list_row_y(area, 0, 1), (4, 5));
        assert_eq!(list_row_y(area, 2, 1), (6, 7));
    }

    #[test]
    fn hit_map_prefers_later_regions() {
        let mut hits = HitMap::default();
        hits.add(CellSpan::new(0, 10, 0, 2), "/first");
        hits.add(CellSpan::new(0, 10, 0, 2), "/second");
        assert_eq!(hits.href_at((0, 0)), Some("/second"));
        assert!(hits.hovering((0, 0)));
        assert!(!hits.hovering((1_000, 1_000)));
    }
}
