use super::FrameCtx;
use crate::content;
use crate::module::scraper::tag_slug;
use crate::mouse::{list_row_y, CellSpan};
use crate::router::Router;
use crate::theme;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::widgets::{Block, BorderType, HighlightSpacing, List, ListState};
use ratatui::Frame;

pub fn render(ctx: &mut FrameCtx<'_>, frame: &mut Frame<'_>, area: Rect) {
    let [sidebar, body] =
        Layout::horizontal([Constraint::Percentage(15), Constraint::Percentage(85)]).areas(area);

    render_tag_list(ctx, frame, sidebar);
    render_post_list(ctx, frame, body);
}

fn render_tag_list(ctx: &mut FrameCtx<'_>, frame: &mut Frame<'_>, area: Rect) {
    let tags_block = Block::bordered()
        .title("{{ tags }}")
        .title_alignment(Alignment::Center)
        .border_type(BorderType::Plain);

    let tags = content::tags();
    let tag_list = List::new(tags.iter().map(String::as_str))
        .block(tags_block)
        .bg(theme::BG)
        .highlight_style(Style::new().fg(theme::HOVER))
        .highlight_spacing(HighlightSpacing::WhenSelected);

    let selected_slug = ctx.router.slug();
    let mut hovered = None;
    let mut selected = None;
    for (index, tag) in tags.iter().enumerate() {
        let (y0, y1) = list_row_y(area, index, 1);
        let x0 = area.x;
        let x1 = area.x.saturating_add(area.width);
        let span = CellSpan::new(x0, x1, y0, y1);
        ctx.hits.add(span, Router::tag_href(tag));
        if ctx.mouse.hits_rect(area) && ctx.mouse.hits(span) {
            hovered = Some(index);
        }
        if selected_slug == Some(tag_slug(tag).as_str()) {
            selected = Some(index);
        }
    }
    let mut tags_state = ListState::default();
    tags_state.select(hovered.or(selected));

    frame.render_stateful_widget(tag_list, area, &mut tags_state);
}

fn render_post_list(ctx: &mut FrameCtx<'_>, frame: &mut Frame<'_>, area: Rect) {
    let posts = content::posts(ctx.router.slug());
    let inner_width = area.width.saturating_sub(2);
    let labels: Vec<String> = if posts.is_empty() {
        vec!["(no posts)".to_string()]
    } else {
        posts
            .iter()
            .map(|post| format_post_label(&post.title, inner_width))
            .collect()
    };

    let body_block = Block::bordered()
        .title(format!("{{ {} }}", ctx.router.title()))
        .title_alignment(Alignment::Center)
        .border_type(BorderType::Plain);

    let post_list = List::new(labels)
        .block(body_block)
        .bg(theme::BG)
        .highlight_style(Style::new().fg(theme::HOVER).add_modifier(Modifier::REVERSED))
        .highlight_spacing(HighlightSpacing::WhenSelected);

    let mut posts_state = ListState::default();
    if !posts.is_empty() {
        for (index, post) in posts.iter().enumerate() {
            let (y0, y1) = list_row_y(area, index, 1);
            let x0 = area.x;
            let x1 = area.x.saturating_add(area.width);
            let span = CellSpan::new(x0, x1, y0, y1);
            ctx.hits.add(span, post.href.clone());
            if ctx.mouse.hits_rect(area) && ctx.mouse.hits(span) {
                posts_state.select(Some(index));
            }
        }
    }

    frame.render_stateful_widget(post_list, area, &mut posts_state);
}

/// Split `Title - date` so the date is shown as `(date)` on the right.
fn format_post_label(title: &str, width: u16) -> String {
    let Some((name, date)) = split_trailing_date(title) else {
        return title.to_string();
    };
    let right = format!("({date})");
    let name_w = display_width(name);
    let right_w = display_width(&right);
    let width = width as usize;
    if name_w + 1 + right_w >= width {
        format!("{name} {right}")
    } else {
        let pad = width - name_w - right_w;
        format!("{name}{:pad$}{right}", "")
    }
}

fn split_trailing_date(title: &str) -> Option<(&str, &str)> {
    let (name, date) = title
        .rsplit_once(" - ")
        .or_else(|| title.rsplit_once('-'))?;
    let name = name.trim_end();
    let date = date.trim();
    if name.is_empty() || !looks_like_date(date) {
        None
    } else {
        Some((name, date))
    }
}

fn looks_like_date(text: &str) -> bool {
    let digits = text.chars().filter(|ch| ch.is_ascii_digit()).count();
    digits >= 4
        && text
            .chars()
            .all(|ch| ch.is_ascii_digit() || matches!(ch, '.' | '-' | '/' | ' '))
}

fn display_width(text: &str) -> usize {
    text.chars()
        .map(|ch| if ch.is_ascii() { 1 } else { 2 })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::{format_post_label, split_trailing_date};

    #[test]
    fn splits_date_after_dash() {
        assert_eq!(
            split_trailing_date("TEST-contents - 2025.12.12"),
            Some(("TEST-contents", "2025.12.12"))
        );
        assert_eq!(split_trailing_date("TEST-contents"), None);
    }

    #[test]
    fn wraps_date_and_right_aligns() {
        let label = format_post_label("TEST-contents - 2025.12.12", 40);
        assert!(label.starts_with("TEST-contents"));
        assert!(label.ends_with("(2025.12.12)"));
        assert_eq!(label.chars().count(), 40);
        assert!(!label.contains(" - "));
    }
}
