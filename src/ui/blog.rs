use super::{page_panel, FrameCtx};
use crate::content::TAGS;
use crate::mouse::{list_row_y, CellSpan};
use crate::router::Router;
use crate::theme;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::widgets::{Block, BorderType, HighlightSpacing, List, ListState};
use ratatui::Frame;

const BODY: &str = "my blog site. powered by ratatui\n\
             now page is \n\
             ";

pub fn render(ctx: &mut FrameCtx<'_>, frame: &mut Frame<'_>, area: Rect) {
    let [sidebar, content] =
        Layout::horizontal([Constraint::Percentage(15), Constraint::Percentage(85)]).areas(area);

    let tags_block = Block::bordered()
        .title("{{ tags }}")
        .title_alignment(Alignment::Center)
        .border_type(BorderType::Plain);

    let tag_list = List::new(TAGS.iter().copied())
        .block(tags_block)
        .bg(theme::BG)
        .highlight_style(Style::new().fg(theme::HOVER).add_modifier(Modifier::BOLD))
        .highlight_spacing(HighlightSpacing::WhenSelected);

    let mut tags_state = ListState::default();
    for (index, tag) in TAGS.iter().enumerate() {
        let (y0, y1) = list_row_y(sidebar, index, 1);
        let x0 = sidebar.x;
        let x1 = sidebar.x.saturating_add(sidebar.width);
        let span = CellSpan::new(x0, x1, y0, y1);
        ctx.hits.add(span, Router::tag_href(tag));
        if ctx.mouse.hits_rect(sidebar) && ctx.mouse.hits(span) {
            tags_state.select(Some(index));
        }
    }

    frame.render_widget(page_panel(&ctx.router.title(), BODY), content);
    frame.render_stateful_widget(tag_list, sidebar, &mut tags_state);
}
