use super::{render_page, render_scrolling_page, FrameCtx};
use crate::content;
use ratatui::layout::Rect;
use ratatui::Frame;

const BODY: &str = "about me & contact\n\
                    \n\
                    DocRAID\n\
                    l01062506145@gmail.com\n";

pub fn render(ctx: &mut FrameCtx<'_>, frame: &mut Frame<'_>, area: Rect) {
    let owned;
    let text = match content::about_text() {
        Some(from_notion) if !from_notion.trim().is_empty() => {
            owned = from_notion;
            owned.as_str()
        }
        _ => BODY,
    };
    if text.lines().count() > 8 || area.height < 16 {
        render_scrolling_page(ctx, frame, area, &ctx.router.title(), text, false);
    } else {
        render_page(frame, area, &ctx.router.title(), text.to_string());
    }
}
