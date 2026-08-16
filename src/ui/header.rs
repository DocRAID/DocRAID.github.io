use super::FrameCtx;
use crate::mouse::{header_button_ranges, CellSpan};
use crate::router::Router;
use crate::theme;
use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Padding, Paragraph};
use ratatui::Frame;

pub fn render(ctx: &mut FrameCtx<'_>, frame: &mut Frame<'_>, area: Rect) {
    let labels: Vec<&str> = Router::NAV.iter().map(|route| route.label()).collect();
    let ranges = header_button_ranges(area, &labels);
    let y0 = area.y.saturating_add(1);
    let y1 = area.y.saturating_add(2);

    let mut menu = Vec::with_capacity(Router::NAV.len());
    for (route, &(x0, x1)) in Router::NAV.iter().zip(ranges.iter()) {
        let span = CellSpan::new(x0, x1, y0, y1);
        ctx.hits.add(span, route.path());

        let color = if ctx.mouse.hits(span) {
            theme::HOVER
        } else {
            theme::FG
        };
        menu.push(Span::styled(format!(" [{route}] "), Style::new().fg(color)));
    }

    let header = Paragraph::new(Line::from(menu))
        .block(
            Block::bordered()
                .border_type(BorderType::Plain)
                .padding(Padding::horizontal(1)),
        )
        .fg(theme::FG)
        .bg(theme::BG)
        .centered();

    frame.render_widget(header, area);
}
