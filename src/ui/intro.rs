use super::{render_page, FrameCtx};
use ratatui::layout::Rect;
use ratatui::Frame;

const BODY: &str = "my blog site. powered by ratatui\n\
             now page is \n\
             ";

pub fn render(ctx: &FrameCtx<'_>, frame: &mut Frame<'_>, area: Rect) {
    render_page(frame, area, &ctx.router.title(), BODY);
}
