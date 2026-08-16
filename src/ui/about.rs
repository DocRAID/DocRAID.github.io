use super::{render_page, FrameCtx};
use ratatui::layout::Rect;
use ratatui::Frame;

const BODY: &str = "about me & contact\n\
                    \n\
                    DocRAID\n\
                    l01062506145@gmail.com\n";

pub fn render(ctx: &FrameCtx<'_>, frame: &mut Frame<'_>, area: Rect) {
    render_page(frame, area, &ctx.router.title(), BODY);
}
