use super::FrameCtx;
use crate::content::{self, RecentPost};
use crate::mouse::CellSpan;
use crate::router::Route;
use crate::theme;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Widget,
};
use ratatui::Frame;

const RECENT_LIMIT: usize = 5;
const BANNER_HEIGHT: u16 = 8;
const RULE_HEIGHT: u16 = 2;
const TAGLINE_HEIGHT: u16 = 2;
const NULL_HEIGHT: u16 = 1;
const HEADING_HEIGHT: u16 = 2;
const FOOTER_HEIGHT: u16 = 2;
const SECTION_GAP: u16 = 1;
const POST_BOX_HEIGHT: u16 = 6;
const POST_BOX_MAX_WIDTH: u16 = 48;
const POST_BOX_GAP: u16 = 1;

//big money, big, slant, ascii 12, Rebel
const BANNER: &str = "\n    __    ____      ___          __           \n   / /   / __ \\    / ( )_____   / /___  ____ _\n  / /   / / / /_  / /|// ___/  / / __ \\/ __ `/\n / /___/ /_/ / /_/ /  (__  )  / / /_/ / /_/ / \n/_____/_____/\\____/  /____/  /_/\\____/\\__, /  \n                                  /____/";

pub fn render(ctx: &mut FrameCtx<'_>, frame: &mut Frame<'_>, area: Rect) {
    let outer = Block::bordered()
        .title("{ Intro }")
        .title_alignment(Alignment::Center)
        .border_type(BorderType::Plain)
        .fg(theme::FG)
        .bg(theme::BG);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let posts = content::recent_posts(RECENT_LIMIT);
    let content_h = content_height(posts.len());
    let viewport = inner.height;
    ctx.report_scroll(content_h, viewport);

    if inner.width == 0 || viewport == 0 {
        return;
    }

    let scrollable = content_h > viewport;
    let offset = if scrollable {
        ctx.scroll.min(content_h.saturating_sub(viewport))
    } else {
        0
    };
    let pad_top = viewport.saturating_sub(content_h) / 2;
    let body_width = if scrollable {
        inner.width.saturating_sub(1)
    } else {
        inner.width
    };
    if body_width == 0 {
        return;
    }

    let mut buf = Buffer::empty(Rect::new(0, 0, body_width, content_h.max(1)));
    Block::new().bg(theme::BG).render(*buf.area(), &mut buf);
    paint_content(ctx, &mut buf, &posts, inner, offset, pad_top, viewport);
    blit_visible(
        &buf, frame, inner, offset, pad_top, body_width, content_h, viewport,
    );

    if scrollable {
        let mut state = ScrollbarState::new(content_h.saturating_sub(viewport) as usize)
            .position(offset as usize);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight).style(Style::new().fg(theme::DIM)),
            inner,
            &mut state,
        );
    }
}

fn content_height(post_count: usize) -> u16 {
    let recent = if post_count == 0 {
        1
    } else {
        let n = post_count as u16;
        n.saturating_mul(POST_BOX_HEIGHT)
            .saturating_add(n.saturating_sub(1).saturating_mul(POST_BOX_GAP))
    };
    BANNER_HEIGHT
        .saturating_add(RULE_HEIGHT)
        .saturating_add(TAGLINE_HEIGHT)
        .saturating_add(NULL_HEIGHT)
        .saturating_add(HEADING_HEIGHT)
        .saturating_add(recent)
        .saturating_add(SECTION_GAP)
        .saturating_add(FOOTER_HEIGHT)
}

fn paint_content(
    ctx: &mut FrameCtx<'_>,
    buf: &mut Buffer,
    posts: &[RecentPost],
    inner: Rect,
    offset: u16,
    pad_top: u16,
    viewport: u16,
) {
    let width = buf.area().width;
    let mut y = 0_u16;

    Paragraph::new(BANNER)
        .fg(theme::HOVER)
        .alignment(Alignment::Center)
        .bg(theme::BG)
        .render(Rect::new(0, y, width, BANNER_HEIGHT), buf);
    y = y.saturating_add(BANNER_HEIGHT);

    Paragraph::new(Line::from(Span::styled(
        "─ · ─ · ─ · ─ · ─ · ─ · ─ · ─",
        Style::new().fg(theme::DIM),
    )))
    .alignment(Alignment::Center)
    .bg(theme::BG)
    .render(Rect::new(0, y, width, RULE_HEIGHT), buf);
    y = y.saturating_add(RULE_HEIGHT);

    Paragraph::new(vec![
        Line::from(Span::styled(
            "Dongju Lim's personal blog",
            Style::new().fg(theme::FG).add_modifier(Modifier::ITALIC),
        )),
        Line::from(Span::styled(
            "Study  ·  Think  ·  Logging",
            Style::new().fg(theme::DIM),
        )),
    ])
    .alignment(Alignment::Center)
    .bg(theme::BG)
    .render(Rect::new(0, y, width, TAGLINE_HEIGHT), buf);
    y = y.saturating_add(TAGLINE_HEIGHT);


    y = y.saturating_add(NULL_HEIGHT);

    Paragraph::new("{ recent posts }")
        .alignment(Alignment::Center)
        .fg(theme::DIM)
        .bg(theme::BG)
        .render(Rect::new(0, y, width, HEADING_HEIGHT), buf);
    y = y.saturating_add(HEADING_HEIGHT);

    if posts.is_empty() {
        Paragraph::new("no posts yet")
            .alignment(Alignment::Center)
            .fg(theme::DIM)
            .bg(theme::BG)
            .render(Rect::new(0, y, width, 1), buf);
        y = y.saturating_add(1);
    } else {
        y = paint_recent_posts(ctx, buf, posts, y, width, inner, offset, pad_top, viewport);
    }

    y = y.saturating_add(SECTION_GAP);
    Paragraph::new(Line::from(vec![
        Span::styled("hover ", Style::new().fg(theme::DIM)),
        Span::styled("[Blog]", Style::new().fg(theme::HOVER)),
        Span::styled(" or ", Style::new().fg(theme::DIM)),
        Span::styled("[About]", Style::new().fg(theme::HOVER)),
        Span::styled("  ·  click to enter", Style::new().fg(theme::DIM)),
    ]))
    .alignment(Alignment::Center)
    .bg(theme::BG)
    .render(Rect::new(0, y, width, FOOTER_HEIGHT), buf);
}

fn paint_recent_posts(
    ctx: &mut FrameCtx<'_>,
    buf: &mut Buffer,
    posts: &[RecentPost],
    mut y: u16,
    width: u16,
    inner: Rect,
    offset: u16,
    pad_top: u16,
    viewport: u16,
) -> u16 {
    let box_width = POST_BOX_MAX_WIDTH
        .min(width.saturating_sub(2))
        .max(width.min(10));
    let x = (width.saturating_sub(box_width)) / 2;

    for (index, post) in posts.iter().enumerate() {
        if index > 0 {
            y = y.saturating_add(POST_BOX_GAP);
        }
        let rect = Rect::new(x, y, box_width, POST_BOX_HEIGHT);
        let text_width = rect.width.saturating_sub(3) as usize;
        paint_hit_card(
            ctx,
            buf,
            rect,
            &post.href,
            &post.title,
            &post_subtitle(post, text_width),
            inner,
            offset,
            pad_top,
            viewport,
        );
        y = y.saturating_add(POST_BOX_HEIGHT);
    }
    y
}

fn paint_hit_card(
    ctx: &mut FrameCtx<'_>,
    buf: &mut Buffer,
    area: Rect,
    href: &str,
    title: &str,
    subtitle: &str,
    inner: Rect,
    offset: u16,
    pad_top: u16,
    viewport: u16,
) {
    let hovered = if let Some(span) = screen_span(area, inner, offset, pad_top, viewport) {
        ctx.hits.add(span, href);
        ctx.mouse.hits(span)
    } else {
        false
    };
    paint_card(buf, area, hovered, title, subtitle);
}

fn screen_span(
    content: Rect,
    inner: Rect,
    offset: u16,
    pad_top: u16,
    viewport: u16,
) -> Option<CellSpan> {
    let start = content.y.max(offset);
    let end = content
        .y
        .saturating_add(content.height)
        .min(offset.saturating_add(viewport));
    if end <= start || content.width == 0 {
        return None;
    }
    Some(CellSpan::new(
        inner.x.saturating_add(content.x),
        inner
            .x
            .saturating_add(content.x)
            .saturating_add(content.width),
        inner
            .y
            .saturating_add(pad_top)
            .saturating_add(start.saturating_sub(offset)),
        inner
            .y
            .saturating_add(pad_top)
            .saturating_add(end.saturating_sub(offset)),
    ))
}

fn blit_visible(
    src: &Buffer,
    frame: &mut Frame<'_>,
    inner: Rect,
    offset: u16,
    pad_top: u16,
    width: u16,
    content_h: u16,
    viewport: u16,
) {
    let dest = frame.buffer_mut();
    let rows = content_h.saturating_sub(offset).min(viewport);
    for row in 0..rows {
        let dest_y = inner.y.saturating_add(pad_top).saturating_add(row);
        if dest_y >= inner.y.saturating_add(inner.height) {
            break;
        }
        let src_y = offset.saturating_add(row);
        for x in 0..width {
            let dest_x = inner.x.saturating_add(x);
            if dest_x >= inner.x.saturating_add(inner.width) {
                break;
            }
            dest[(dest_x, dest_y)] = src[(x, src_y)].clone();
        }
    }
}

fn post_subtitle(post: &RecentPost, width: usize) -> String {
    match post.date.as_deref() {
        Some(date) if !post.tag.is_empty() => {
            let combined = format!("{} · {}", post.tag, date);
            if display_width(&combined) <= width {
                combined
            } else {
                date.to_string()
            }
        }
        Some(date) => date.to_string(),
        None => post.tag.clone(),
    }
}

fn paint_card(buf: &mut Buffer, area: Rect, hovered: bool, title: &str, subtitle: &str) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let accent = if hovered { theme::HOVER } else { theme::DIM };
    let title_style = if hovered {
        Style::new().fg(theme::HOVER).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(theme::FG)
    };
    let text_width = (area.width as usize).saturating_sub(3);

    Paragraph::new(vec![
        Line::from(Span::styled(
            format!(" {}", truncate_display(title, text_width)),
            title_style,
        )),
        Line::from(Span::styled(
            format!(" {}", truncate_display(subtitle, text_width)),
            Style::new().fg(theme::DIM),
        )),
    ])
    .block(
        Block::bordered()
            .border_type(if hovered {
                BorderType::Double
            } else {
                BorderType::Plain
            })
            .border_style(Style::new().fg(accent))
            .padding(Padding::vertical(1)),
    )
    .bg(theme::BG)
    .render(area, buf);
}

fn truncate_display(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if display_width(text) <= width {
        return text.to_string();
    }
    if width == 1 {
        return "…".to_string();
    }
    let budget = width.saturating_sub(1);
    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let ch_width = char_width(ch);
        if used + ch_width > budget {
            break;
        }
        out.push(ch);
        used += ch_width;
    }
    out.push('…');
    out
}

fn display_width(text: &str) -> usize {
    text.chars().map(char_width).sum()
}

fn char_width(ch: char) -> usize {
    if ch.is_ascii() || ch == '…' {
        1
    } else {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::{display_width, render, truncate_display};
    use crate::module::scraper::{self, ContentPage, TagSection};
    use crate::mouse::{HitMap, MouseState};
    use crate::router::Router;
    use crate::ui::FrameCtx;
    use ratatui::{backend::TestBackend, Terminal};

    fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
        let buffer = terminal.backend().buffer();
        let area = buffer.area();
        let mut out = String::new();
        for y in area.y..area.y.saturating_add(area.height) {
            for x in area.x..area.x.saturating_add(area.width) {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    fn draw_intro(width: u16, height: u16, scroll: u16) -> (String, HitMap, (u16, u16)) {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        let router = Router::parse("/");
        let mouse = MouseState::default();
        let mut hits = HitMap::default();
        let mut metrics = (0_u16, 0_u16);
        terminal
            .draw(|frame| {
                let mut ctx = FrameCtx {
                    router: &router,
                    mouse: &mouse,
                    hits: &mut hits,
                    scroll,
                    scroll_metrics: Some(&mut metrics),
                };
                render(&mut ctx, frame, frame.area());
            })
            .unwrap();
        (buffer_text(&terminal), hits, metrics)
    }

    fn first_line(text: &str, needle: &str) -> Option<usize> {
        text.lines().position(|line| line.contains(needle))
    }

    fn seed_posts() {
        scraper::set_catalog_for_tests(vec![TagSection {
            tag: "linux".into(),
            pages: (1..=6)
                .map(|n| ContentPage {
                    title: format!("post{n} - 2025.01.{n:02}"),
                    id: format!("{n}"),
                    href: format!("/blog/linux/{n}"),
                })
                .collect(),
        }]);
    }

    #[test]
    fn empty_catalog_shows_recent_heading() {
        scraper::set_catalog_for_tests(Vec::new());
        let (text, _, _) = draw_intro(100, 40, 0);
        assert!(text.contains("recent posts"));
        assert!(text.contains("no posts yet"));
    }

    #[test]
    fn shows_five_latest_posts_in_a_vertical_stack() {
        seed_posts();
        let (text, hits, _) = draw_intro(100, 70, 0);
        scraper::set_catalog_for_tests(Vec::new());

        assert!(text.contains("recent posts"));
        assert!(text.contains("post6"));
        assert!(text.contains("post5"));
        assert!(text.contains("post4"));
        assert!(text.contains("post3"));
        assert!(text.contains("post2"));
        assert!(!text.contains("post1"));
        assert!(text.contains("2025.01.06"));

        let post6 = first_line(&text, "post6").unwrap();
        let post5 = first_line(&text, "post5").unwrap();
        let post2 = first_line(&text, "post2").unwrap();
        assert!(post6 < post5);
        assert!(post5 < post2);
        let post6_line = text.lines().nth(post6).unwrap();
        assert!(!post6_line.contains("post5"));

        let hrefs = hits.hrefs();
        assert!(hrefs.contains(&"/blog"));
        assert!(hrefs.contains(&"/about"));
        assert!(hrefs.contains(&"/blog/linux/6"));
        assert!(hrefs.contains(&"/blog/linux/2"));
        assert!(!hrefs.contains(&"/blog/linux/1"));
    }

    #[test]
    fn short_viewport_scrolls_instead_of_clipping() {
        seed_posts();
        let (top, top_hits, metrics) = draw_intro(100, 28, 0);
        let max_scroll = metrics.0.saturating_sub(metrics.1);
        let (bottom, bottom_hits, _) = draw_intro(100, 28, max_scroll);
        scraper::set_catalog_for_tests(Vec::new());

        assert!(metrics.0 > metrics.1);
        assert!(top.contains("Dongju"));
        assert!(top.contains("post6"));
        assert!(!top.contains("click to enter"));
        assert!(bottom.contains("click to enter"));
        assert!(bottom.contains("post2"));
        assert!(!bottom.contains("Dongju"));
        assert!(top_hits.hrefs().contains(&"/blog"));
        assert!(bottom_hits.hrefs().contains(&"/blog/linux/2"));
        assert!(!bottom_hits.hrefs().contains(&"/blog"));
    }

    #[test]
    fn truncate_keeps_display_width() {
        assert_eq!(truncate_display("abcdef", 10), "abcdef");
        assert_eq!(truncate_display("abcdef", 4), "abc…");
        assert_eq!(display_width(&truncate_display("한글제목", 5)), 5);
    }
}
