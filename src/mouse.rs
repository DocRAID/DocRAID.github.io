use ratatui::layout::Rect;
use ratzilla::event::MouseEvent;

/// Approximate CSS-pixel size of one terminal cell under the current
/// `index.html` stylesheet (Fira Code 16px). These are calibrated values
/// and should be replaced once Ratzilla exposes real cell metrics.
const CELL_WIDTH_PX: f32 = 9.84998;
const CELL_HEIGHT_PX: u16 = 22;

/// Extra cells added to a [`Rect`]'s right edge so sidebar hit-testing
/// still matches the previous sloppy bounds.
const RECT_WIDTH_SLOP: u16 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MouseState {
    pos: (u32, u32),
}

impl MouseState {
    pub fn update(&mut self, event: &MouseEvent) {
        self.pos = (event.x, event.y);
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
/// `x1`/`y1` follow the previous helpers: they are the far cell indices
/// whose left/top pixel edges form the inclusive max bound.
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
        let px0 = f32::from(self.x0) * CELL_WIDTH_PX;
        let px1 = f32::from(self.x1) * CELL_WIDTH_PX;
        let py0 = self.y0.saturating_mul(CELL_HEIGHT_PX);
        let py1 = self.y1.saturating_mul(CELL_HEIGHT_PX);

        (mouse.0 as f32) >= px0
            && (mouse.0 as f32) <= px1
            && (mouse.1 as u16) >= py0
            && (mouse.1 as u16) <= py1
    }
}

pub fn rect_span(area: Rect) -> CellSpan {
    CellSpan::new(
        area.x,
        area.x
            .saturating_add(area.width)
            .saturating_add(RECT_WIDTH_SLOP),
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

    #[cfg(test)]
    pub fn href_at(&self, mouse: (u32, u32)) -> Option<&str> {
        match self.action_at(mouse) {
            Some(HitAction::Go(href)) => Some(href.as_str()),
            _ => None,
        }
    }
}

/// Inclusive cell-x ranges for a centered row of labeled buttons.
///
/// The `+ HEADER_HIT_NUDGE` fudge matches the visual center of a
/// `Paragraph::centered` row inside a padded bordered block.
const HEADER_HIT_NUDGE: usize = 2;
const BUTTON_PAD: usize = 4; // `" [{}] "` around the label

pub fn header_button_ranges(area: Rect, labels: &[&str]) -> Vec<(u16, u16)> {
    let widths: Vec<usize> = labels
        .iter()
        .map(|label| label.len().saturating_add(BUTTON_PAD))
        .collect();
    let total: usize = widths.iter().sum();
    let mut offset = (area.width as usize / 2)
        .saturating_sub(total / 2)
        .saturating_add(HEADER_HIT_NUDGE)
        .saturating_add(area.x as usize);

    let mut ranges = Vec::with_capacity(widths.len());
    for width in widths {
        let start = offset;
        let end = offset.saturating_add(width.saturating_sub(1));
        ranges.push((start as u16, end as u16));
        offset = offset.saturating_add(width);
    }
    ranges
}

/// Vertical cell span of the `index`th list row inside a bordered list.
pub fn list_row_y(area: Rect, index: usize, height: u16) -> (u16, u16) {
    let top = area.y.saturating_add(1).saturating_add(index as u16);
    (top, top.saturating_add(height))
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
        assert_eq!(ranges[1].0, ranges[0].1 + 1);
        assert_eq!(ranges[2].0, ranges[1].1 + 1);
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
    }
}
