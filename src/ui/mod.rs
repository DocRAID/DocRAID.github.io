mod about;
mod blog;
mod header;
mod intro;
mod not_found;

use crate::mouse::{HitMap, MouseState};
use crate::router::{Route, Router};
use crate::theme;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::Text;
use ratatui::widgets::{Block, BorderType, Paragraph, Wrap};
use ratatui::Frame;

/// Per-frame view of app state that widgets may read or write.
pub struct FrameCtx<'a> {
    pub router: &'a Router,
    pub mouse: &'a MouseState,
    pub hits: &'a mut HitMap,
    pub scroll: u16,
    pub scroll_metrics: Option<&'a mut (u16, u16)>,
    pub list_selected: Option<usize>,
    pub nav_items: &'a mut Vec<String>,
    pub filter: &'a str,
    pub filter_open: bool,
    pub copied: bool,
}

impl FrameCtx<'_> {
    pub fn report_scroll(&mut self, content_height: u16, viewport: u16) {
        if let Some(metrics) = self.scroll_metrics.as_mut() {
            **metrics = (content_height, viewport);
        }
    }
}

pub fn render(ctx: &mut FrameCtx<'_>, frame: &mut Frame<'_>) {
    let [header, body] =
        Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).areas(frame.area());

    header::render(ctx, frame, header);
    match ctx.router.route() {
        Route::Intro => intro::render(ctx, frame, body),
        Route::About => about::render(ctx, frame, body),
        Route::Blog => blog::render(ctx, frame, body),
        Route::NotFound => not_found::render(ctx, frame, body),
    }
}

fn page_panel<'a>(title: &str, text: impl Into<Text<'a>>) -> Paragraph<'a> {
    Paragraph::new(text)
        .block(
            Block::bordered()
                .title(format!("{{ {title} }}"))
                .title_alignment(Alignment::Center)
                .border_type(BorderType::Plain),
        )
        .wrap(Wrap { trim: false })
        .fg(theme::FG)
        .bg(theme::BG)
}

fn render_page(frame: &mut Frame<'_>, area: Rect, title: &str, text: impl Into<Text<'static>>) {
    frame.render_widget(page_panel(title, text).centered(), area);
}

fn render_scrolling_page(
    ctx: &mut FrameCtx<'_>,
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    text: &str,
    centered: bool,
) {
    let inner_h = area.height.saturating_sub(2);
    let inner_w = area.width.saturating_sub(2);
    let content_h = crate::width::wrapped_rows(text, inner_w.max(1));
    ctx.report_scroll(content_h, inner_h);
    let offset = ctx.scroll.min(content_h.saturating_sub(inner_h));
    let mut paragraph = page_panel(title, text.to_string()).scroll((offset, 0));
    if centered {
        paragraph = paragraph.centered();
    }
    frame.render_widget(paragraph, area);
}
