use super::FrameCtx;
use crate::content;
use crate::module::scraper::{same_page_id, tag_slug, PostSegment};
use crate::mouse::{list_row_y, CellSpan};
use crate::router::Router;
use crate::theme;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, HighlightSpacing, List, ListState, Paragraph, Scrollbar,
    ScrollbarOrientation, ScrollbarState, Wrap,
};
use ratatui::Frame;

pub fn render(ctx: &mut FrameCtx<'_>, frame: &mut Frame<'_>, area: Rect) {
    let [sidebar, body] =
        Layout::horizontal([Constraint::Percentage(15), Constraint::Percentage(85)]).areas(area);

    if let Some(post_id) = ctx.router.post() {
        content::ensure_post(post_id);
        render_category_posts(ctx, frame, sidebar);
        render_post_body(ctx, frame, body);
    } else {
        render_tag_list(ctx, frame, sidebar);
        render_post_list(ctx, frame, body);
    }
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
        .highlight_style(
            Style::new()
                .fg(theme::HOVER)
                .add_modifier(Modifier::REVERSED),
        )
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

fn render_category_posts(ctx: &mut FrameCtx<'_>, frame: &mut Frame<'_>, area: Rect) {
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

    let list = List::new(labels)
        .block(
            Block::bordered()
                .title("{{ posts }}")
                .title_alignment(Alignment::Center)
                .border_type(BorderType::Plain),
        )
        .bg(theme::BG)
        .highlight_style(Style::new().fg(theme::HOVER))
        .highlight_spacing(HighlightSpacing::WhenSelected);

    let selected_post = ctx.router.post();
    let mut hovered = None;
    let mut selected = None;
    for (index, post) in posts.iter().enumerate() {
        let (y0, y1) = list_row_y(area, index, 1);
        let span = CellSpan::new(area.x, area.x.saturating_add(area.width), y0, y1);
        ctx.hits.add(span, post.href.clone());
        if ctx.mouse.hits_rect(area) && ctx.mouse.hits(span) {
            hovered = Some(index);
        }
        if selected_post.is_some_and(|id| same_page_id(id, &post.id)) {
            selected = Some(index);
        }
    }
    let mut state = ListState::default();
    state.select(hovered.or(selected));
    frame.render_stateful_widget(list, area, &mut state);
}

const COPY_LABEL: &str = " Copy ";

fn render_post_body(ctx: &mut FrameCtx<'_>, frame: &mut Frame<'_>, area: Rect) {
    let post_id = ctx.router.post().unwrap_or("");
    let title = content::posts(ctx.router.slug())
        .into_iter()
        .find(|post| same_page_id(&post.id, post_id))
        .map(|post| post.title)
        .unwrap_or_else(|| "post".to_string());

    let outer = Block::bordered()
        .title(format!("{{ {title} }}"))
        .title_alignment(Alignment::Center)
        .border_type(BorderType::Plain)
        .fg(theme::FG)
        .bg(theme::BG);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let Some(segments) = content::post_segments(post_id) else {
        report_scroll(ctx, 0, inner.height);
        frame.render_widget(
            Paragraph::new("(loading…)").fg(theme::FG).bg(theme::BG),
            inner,
        );
        return;
    };
    if segments.is_empty() {
        report_scroll(ctx, 0, inner.height);
        frame.render_widget(
            Paragraph::new("(no content)").fg(theme::FG).bg(theme::BG),
            inner,
        );
        return;
    }

    let content_height = content_height(&segments);
    let viewport = inner.height;
    report_scroll(ctx, content_height, viewport);
    let offset = ctx.scroll.min(content_height.saturating_sub(viewport));
    let body_area = Rect {
        width: inner.width.saturating_sub(1),
        ..inner
    };

    let mut cursor = 0_u16;
    for (index, segment) in segments.iter().enumerate() {
        if index > 0 {
            cursor = cursor.saturating_add(1);
        }
        let content_y = cursor;
        let height = segment_height(segment);
        cursor = cursor.saturating_add(height);
        let Some(dest) = visible_rect(body_area, content_y, height, offset, viewport) else {
            continue;
        };
        match segment {
            PostSegment::Text(text) => {
                frame.render_widget(
                    Paragraph::new(text.as_str())
                        .wrap(Wrap { trim: false })
                        .scroll((offset.saturating_sub(content_y), 0))
                        .fg(theme::FG)
                        .bg(theme::BG),
                    dest,
                );
            }
            PostSegment::Code(code) => {
                render_code_block(ctx, frame, dest, code);
            }
        }
    }

    if content_height > viewport {
        let mut state = ScrollbarState::new(content_height.saturating_sub(viewport) as usize)
            .position(offset as usize);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight).style(Style::new().fg(theme::DIM)),
            inner,
            &mut state,
        );
    }
}

fn report_scroll(ctx: &mut FrameCtx<'_>, content_height: u16, viewport: u16) {
    if let Some(metrics) = ctx.scroll_metrics.as_mut() {
        **metrics = (content_height, viewport);
    }
}

fn content_height(segments: &[PostSegment]) -> u16 {
    let mut height = 0_u16;
    for (index, segment) in segments.iter().enumerate() {
        if index > 0 {
            height = height.saturating_add(1);
        }
        height = height.saturating_add(segment_height(segment));
    }
    height
}

fn visible_rect(
    area: Rect,
    content_y: u16,
    height: u16,
    offset: u16,
    viewport: u16,
) -> Option<Rect> {
    let start = content_y.max(offset);
    let end = content_y
        .saturating_add(height)
        .min(offset.saturating_add(viewport));
    if end <= start {
        return None;
    }
    Some(Rect {
        x: area.x,
        y: area.y.saturating_add(start.saturating_sub(offset)),
        width: area.width,
        height: end.saturating_sub(start),
    })
}

fn segment_height(segment: &PostSegment) -> u16 {
    match segment {
        PostSegment::Text(text) => text.split('\n').count().max(1) as u16,
        PostSegment::Code(code) => (code.split('\n').count().max(1) as u16).saturating_add(2),
    }
}

fn render_code_block(ctx: &mut FrameCtx<'_>, frame: &mut Frame<'_>, area: Rect, code: &str) {
    let sample = theme::CODE_STYLE_SAMPLE.unwrap_or(1);
    let style = theme::code_block_style(sample);
    let copy_hovered = copy_hit_span(area)
        .map(|span| ctx.mouse.hits(span))
        .unwrap_or(false);
    let copy_style = if copy_hovered {
        Style::new().fg(theme::BG).bg(theme::HOVER)
    } else {
        Style::new()
            .fg(theme::DIM)
            .bg(style.bg.unwrap_or(theme::BG))
    };

    let block = Block::bordered()
        .border_type(BorderType::Plain)
        .border_style(style)
        .title("code")
        .title(Line::from(Span::styled(COPY_LABEL, copy_style)).right_aligned())
        .style(style);

    if let Some(span) = copy_hit_span(area) {
        ctx.hits.add_copy(span, code.to_string());
    }

    frame.render_widget(
        Paragraph::new(code)
            .wrap(Wrap { trim: false })
            .style(style)
            .block(block),
        area,
    );
}

fn copy_hit_span(area: Rect) -> Option<CellSpan> {
    if area.width < 8 || area.height == 0 {
        return None;
    }
    let width = COPY_LABEL.len() as u16;
    let x1 = area.x.saturating_add(area.width).saturating_sub(1);
    let x0 = x1.saturating_sub(width);
    Some(CellSpan::new(x0, x1, area.y, area.y.saturating_add(1)))
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
    fn visible_slice_clips_to_viewport() {
        let area = Rect::new(0, 10, 20, 8);
        let dest = super::visible_rect(area, 12, 6, 10, 8).unwrap();
        assert_eq!(dest.y, 12);
        assert_eq!(dest.height, 6);
        assert!(super::visible_rect(area, 0, 2, 10, 8).is_none());
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
