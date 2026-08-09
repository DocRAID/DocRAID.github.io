use crate::app::App;
use crate::module::mouse_tool::*;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::prelude::{Color, Stylize};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, HighlightSpacing, List, ListState, Paragraph};
use ratatui::Frame;
use ratzilla::event::MouseEventKind;

pub fn blog_page(label: String, frame: &mut Frame, layout: Rect, app: &App) {
    let split_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(15), Constraint::Percentage(85)])
        .split(layout);

    let block = Block::bordered()
        .title(format!("{{ {} }}", label))
        .title_alignment(Alignment::Center)
        .border_type(BorderType::Plain);

    let text = "my blog site. powered by ratatui\n\
             now page is \n\
             ";

    let paragraph = Paragraph::new(text)
        .block(block)
        .fg(Color::White)
        .bg(crate::app::BG_RGB)
        .centered();
    // tag
    let mut tags_status = ListState::default();
    let tags_block = Block::bordered()
        .title("{{ tags }}")
        .title_alignment(Alignment::Center)
        .border_type(BorderType::Plain);

    //todo: module에서 가져오게 하기
    let tags_vec = ["linux", "gcc", "knowledge"];

    // tag stateful
    let tag_list = List::new(tags_vec)
        .block(tags_block)
        .bg(crate::app::BG_RGB)
        .highlight_style(Style::new().fg(Color::Green).add_modifier(Modifier::BOLD))
        .highlight_spacing(HighlightSpacing::WhenSelected);

    if is_rects_hovered(split_layout[0], *app.mouse_pos.borrow()) {
        let btn_ranges_y = calc_topdown_list_button_ranges(split_layout[0], tags_vec.len(), 1);
        // info!("!!{:?} {:?} {:?}",split_layout[0].x, split_layout[0].x+split_layout[0].width, btn_ranges_y);
        if let Some(index) = btn_ranges_y.iter().position(
            |&(y1, y2)| {
                is_points_hovered(
                    split_layout[0].x,
                    split_layout[0].x + split_layout[0].width,
                    y1 as u16,
                    y2 as u16,
                    *app.mouse_pos.borrow(),
                )
            },
        ) {
            tags_status.select(Some(index));
            if *app.mouse_status.borrow() == MouseEventKind::Pressed {
                let tag_href = if app.router.sub_query.is_none() {
                    format!("blog/{}", tags_vec[index])
                } else {
                    tags_vec[index].to_string()
                };
                app.window
                    .location()
                    .set_href(&tag_href)
                    .expect("panic on redirect");
            }
        }
    }

    frame.render_widget(paragraph, split_layout[1]);
    frame.render_stateful_widget(tag_list, split_layout[0], &mut tags_status);
}
