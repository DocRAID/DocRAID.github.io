use super::FrameCtx;
use crate::content::{self, CatalogStatus};
use crate::module::notion::PostSegment;
use crate::module::scraper::{same_page_id, tag_slug};
use crate::mouse::{list_row_y, CellSpan};
use crate::router::Router;
use crate::theme;
use crate::width::{display_width, wrapped_rows};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, HighlightSpacing, List, ListState, Paragraph, Scrollbar,
    ScrollbarOrientation, ScrollbarState, Wrap,
};
use ratatui::Frame;

/// Hide the tag/post sidebar below this many terminal columns.
const COMPACT_WIDTH: u16 = 72;
const COPY_LABEL: &str = " Copy ";
const COPIED_LABEL: &str = " Copied ";

pub fn render(ctx: &mut FrameCtx<'_>, frame: &mut Frame<'_>, area: Rect) {
    if is_compact(area) {
        if let Some(post_id) = ctx.router.post() {
            content::ensure_post(post_id);
            render_post_body(ctx, frame, area, true);
        } else {
            render_post_list(ctx, frame, area);
        }
        return;
    }

    let [sidebar, body] =
        Layout::horizontal([Constraint::Percentage(15), Constraint::Percentage(85)]).areas(area);

    if let Some(post_id) = ctx.router.post() {
        content::ensure_post(post_id);
        render_category_posts(ctx, frame, sidebar);
        render_post_body(ctx, frame, body, false);
    } else {
        render_tag_list(ctx, frame, sidebar);
        render_post_list(ctx, frame, body);
    }
}

fn is_compact(area: Rect) -> bool {
    area.width < COMPACT_WIDTH
}

fn status_placeholder() -> String {
    match content::catalog_status() {
        CatalogStatus::Loading => "(loading…)".to_string(),
        CatalogStatus::Error(err) => format!("(failed to load)\n{err}"),
        CatalogStatus::Ready => "(no posts)".to_string(),
    }
}

fn render_tag_list(ctx: &mut FrameCtx<'_>, frame: &mut Frame<'_>, area: Rect) {
    let tags_block = Block::bordered()
        .title("{{ tags }}")
        .title_alignment(Alignment::Center)
        .border_type(BorderType::Plain);

    let tags = content::tags();
    let labels: Vec<String> = if tags.is_empty() {
        vec![status_placeholder()]
    } else {
        tags.clone()
    };
    let tag_list = List::new(labels.iter().cloned())
        .block(tags_block)
        .bg(theme::BG)
        .highlight_style(Style::new().fg(theme::HOVER))
        .highlight_spacing(HighlightSpacing::Never);

    let selected_slug = ctx.router.slug();
    let mut hovered = None;
    let mut selected = None;
    ctx.nav_items.clear();
    for (index, tag) in tags.iter().enumerate() {
        let (y0, y1) = list_row_y(area, index, 1);
        if y0 >= area.y.saturating_add(area.height.saturating_sub(1)) {
            break;
        }
        let span = CellSpan::new(area.x, area.x.saturating_add(area.width), y0, y1);
        let href = Router::tag_href(tag);
        ctx.hits.add(span, href.clone());
        ctx.nav_items.push(href);
        if ctx.mouse.hits_rect(area) && ctx.mouse.hits(span) {
            hovered = Some(index);
        }
        if selected_slug == Some(tag_slug(tag).as_str()) {
            selected = Some(index);
        }
    }
    let mut tags_state = ListState::default();
    tags_state.select(hovered.or(ctx.list_selected).or(selected));

    frame.render_stateful_widget(tag_list, area, &mut tags_state);
}

fn filtered_posts(ctx: &FrameCtx<'_>) -> Vec<crate::module::notion::ContentPage> {
    let mut posts = content::posts(ctx.router.slug());
    if !ctx.filter.is_empty() {
        let needle = ctx.filter.to_lowercase();
        posts.retain(|post| post.title.to_lowercase().contains(&needle));
    }
    posts
}

fn render_post_list(ctx: &mut FrameCtx<'_>, frame: &mut Frame<'_>, area: Rect) {
    let posts = filtered_posts(ctx);
    let inner_width = area.width.saturating_sub(2);
    let labels: Vec<String> = if posts.is_empty() {
        vec![if ctx.filter.is_empty() {
            status_placeholder()
        } else {
            "(no matches)".to_string()
        }]
    } else {
        posts
            .iter()
            .map(|post| format_post_label(&post.title, inner_width))
            .collect()
    };

    let title = if ctx.filter_open {
        format!("{{ {} }}  /{}_", ctx.router.title(), ctx.filter)
    } else if !ctx.filter.is_empty() {
        format!("{{ {} }}  /{}", ctx.router.title(), ctx.filter)
    } else {
        format!("{{ {} }}", ctx.router.title())
    };

    let body_block = Block::bordered()
        .title(title)
        .title_alignment(Alignment::Center)
        .border_type(BorderType::Plain);

    let post_list = List::new(labels)
        .block(body_block)
        .bg(theme::BG)
        .highlight_style(Style::new().fg(theme::HOVER))
        .highlight_spacing(HighlightSpacing::Never);

    let mut posts_state = ListState::default();
    ctx.nav_items.clear();
    if !posts.is_empty() {
        for (index, post) in posts.iter().enumerate() {
            let (y0, y1) = list_row_y(area, index, 1);
            let span = CellSpan::new(area.x, area.x.saturating_add(area.width), y0, y1);
            ctx.hits.add(span, post.href.clone());
            ctx.nav_items.push(post.href.clone());
            if ctx.mouse.hits_rect(area) && ctx.mouse.hits(span) {
                posts_state.select(Some(index));
            }
        }
        if posts_state.selected().is_none() {
            posts_state.select(ctx.list_selected);
        }
    }

    frame.render_stateful_widget(post_list, area, &mut posts_state);
}

fn render_category_posts(ctx: &mut FrameCtx<'_>, frame: &mut Frame<'_>, area: Rect) {
    let posts = content::posts(ctx.router.slug());
    let inner_width = area.width.saturating_sub(2);
    let labels: Vec<String> = if posts.is_empty() {
        vec![status_placeholder()]
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
        .highlight_spacing(HighlightSpacing::Never);

    let selected_post = ctx.router.post();
    let mut hovered = None;
    let mut selected = None;
    ctx.nav_items.clear();
    for (index, post) in posts.iter().enumerate() {
        let (y0, y1) = list_row_y(area, index, 1);
        let span = CellSpan::new(area.x, area.x.saturating_add(area.width), y0, y1);
        ctx.hits.add(span, post.href.clone());
        ctx.nav_items.push(post.href.clone());
        if ctx.mouse.hits_rect(area) && ctx.mouse.hits(span) {
            hovered = Some(index);
        }
        if selected_post.is_some_and(|id| same_page_id(id, &post.id)) {
            selected = Some(index);
        }
    }
    let mut state = ListState::default();
    state.select(hovered.or(ctx.list_selected).or(selected));
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_post_body(ctx: &mut FrameCtx<'_>, frame: &mut Frame<'_>, area: Rect, compact: bool) {
    let post_id = ctx.router.post().unwrap_or("");
    let title = content::posts(ctx.router.slug())
        .into_iter()
        .find(|post| same_page_id(&post.id, post_id))
        .map(|post| post.title)
        .unwrap_or_else(|| "post".to_string());

    let mut outer = Block::bordered()
        .title(format!("{{ {title} }}"))
        .title_alignment(Alignment::Center)
        .border_type(BorderType::Plain)
        .fg(theme::FG)
        .bg(theme::BG);
    if compact {
        if let Some(back) = ctx.router.parent_href() {
            let back_label = " back ";
            let span = CellSpan::new(
                area.x.saturating_add(1),
                area.x.saturating_add(1 + back_label.len() as u16),
                area.y,
                area.y.saturating_add(1),
            );
            ctx.hits.add(span, back);
            outer = outer.title(Line::from(Span::styled(
                back_label,
                if ctx.mouse.hits(span) {
                    Style::new().fg(theme::HOVER)
                } else {
                    Style::new().fg(theme::DIM)
                },
            )));
        }
    }
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    match content::post_state(post_id) {
        None => {
            ctx.report_scroll(0, inner.height);
            frame.render_widget(
                Paragraph::new("(loading…)").fg(theme::FG).bg(theme::BG),
                inner,
            );
        }
        Some(Err(err)) => {
            ctx.report_scroll(0, inner.height);
            frame.render_widget(
                Paragraph::new(format!("(failed to load)\n{err}\nretrying…"))
                    .fg(theme::FG)
                    .bg(theme::BG),
                inner,
            );
        }
        Some(Ok(segments)) if segments.is_empty() => {
            ctx.report_scroll(0, inner.height);
            frame.render_widget(
                Paragraph::new("(no content)").fg(theme::FG).bg(theme::BG),
                inner,
            );
        }
        Some(Ok(segments)) => render_segments(ctx, frame, inner, &segments),
    }
}

fn render_segments(
    ctx: &mut FrameCtx<'_>,
    frame: &mut Frame<'_>,
    inner: Rect,
    segments: &[PostSegment],
) {
    let body_area = Rect {
        width: inner.width.saturating_sub(1),
        ..inner
    };
    let text_width = body_area.width;
    let content_height = content_height(segments, text_width);
    let viewport = inner.height;
    ctx.report_scroll(content_height, viewport);
    let offset = ctx.scroll.min(content_height.saturating_sub(viewport));

    let mut cursor = 0_u16;
    for (index, segment) in segments.iter().enumerate() {
        if index > 0 {
            cursor = cursor.saturating_add(1);
        }
        let content_y = cursor;
        let height = segment_height(segment, text_width);
        cursor = cursor.saturating_add(height);
        let Some(dest) = visible_rect(body_area, content_y, height, offset, viewport) else {
            continue;
        };
        let skip = offset.saturating_sub(content_y);
        match segment {
            PostSegment::Text(text) => {
                frame.render_widget(
                    Paragraph::new(text.as_str())
                        .wrap(Wrap { trim: false })
                        .scroll((skip, 0))
                        .fg(theme::FG)
                        .bg(theme::BG),
                    dest,
                );
            }
            PostSegment::Code(code) => {
                render_code_block(ctx, frame, dest, code, skip);
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

fn content_height(segments: &[PostSegment], width: u16) -> u16 {
    let mut height = 0_u16;
    for (index, segment) in segments.iter().enumerate() {
        if index > 0 {
            height = height.saturating_add(1);
        }
        height = height.saturating_add(segment_height(segment, width));
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

fn segment_height(segment: &PostSegment, width: u16) -> u16 {
    match segment {
        PostSegment::Text(text) => wrapped_rows(text, width.max(1)),
        PostSegment::Code(code) => {
            wrapped_rows(code, width.saturating_sub(2).max(1)).saturating_add(2)
        }
    }
}

fn render_code_block(
    ctx: &mut FrameCtx<'_>,
    frame: &mut Frame<'_>,
    area: Rect,
    code: &str,
    skip: u16,
) {
    let style = theme::code_block_style();
    let copy_label = if ctx.copied { COPIED_LABEL } else { COPY_LABEL };
    let copy_hovered = copy_hit_span(area)
        .map(|span| ctx.mouse.hits(span))
        .unwrap_or(false);
    let copy_style = if copy_hovered || ctx.copied {
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
        .title(Line::from(Span::styled(copy_label, copy_style)).right_aligned())
        .style(style);

    if skip == 0 {
        if let Some(span) = copy_hit_span(area) {
            ctx.hits.add_copy(span, code.to_string());
        }
    }

    let inner_skip = skip.saturating_sub(1);
    frame.render_widget(
        Paragraph::new(code)
            .wrap(Wrap { trim: false })
            .scroll((inner_skip, 0))
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
    let Some((name, date)) = content::split_trailing_date(title) else {
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

#[cfg(test)]
mod tests {
    use super::{format_post_label, segment_height};
    use crate::content::split_trailing_date;
    use crate::module::notion::PostSegment;
    use ratatui::layout::Rect;

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

    #[test]
    fn text_height_includes_wrap() {
        let segment = PostSegment::Text("abcdefghij".into());
        assert_eq!(segment_height(&segment, 5), 2);
    }
}
