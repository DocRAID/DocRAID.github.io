use super::FrameCtx;
use crate::mouse::CellSpan;
use crate::router::Route;
use crate::theme;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Padding, Paragraph};
use ratatui::Frame;

const BANNER: &str = r"
  _              _   _     _
 | |_   _  _    | |_| |__ | | ___   __ _
 | __| | || |   | __| '_ \| |/ _ \ / _` |
 | |_  | || |   | |_| |_) | | (_) | (_| |
  \__|  \_,_|    \__|_.__/|_|\___/ \__, |
                                   |___/ ";

pub fn render(ctx: &mut FrameCtx<'_>, frame: &mut Frame<'_>, area: Rect) {
    let outer = Block::bordered()
        .title("{ Intro }")
        .title_alignment(Alignment::Center)
        .border_type(BorderType::Plain)
        .fg(theme::FG)
        .bg(theme::BG);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let [banner, rule, tagline, cards, footer] = Layout::vertical([
        Constraint::Length(8),
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Length(6),
        Constraint::Min(2),
    ])
    .flex(Flex::Center)
    .areas(inner);

    frame.render_widget(
        Paragraph::new(BANNER)
            .fg(theme::HOVER)
            .alignment(Alignment::Center),
        banner,
    );

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "─ · ─ · ─ · ─ · ─ · ─ · ─ · ─",
            Style::new().fg(theme::DIM),
        )))
        .alignment(Alignment::Center)
        .bg(theme::BG),
        rule,
    );

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "personal notes from a terminal",
                Style::new().fg(theme::FG).add_modifier(Modifier::ITALIC),
            )),
            Line::from(Span::styled(
                "ratatui  ·  rust  ·  notion",
                Style::new().fg(theme::DIM),
            )),
        ])
        .alignment(Alignment::Center)
        .bg(theme::BG),
        tagline,
    );

    render_cards(ctx, frame, cards);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("hover ", Style::new().fg(theme::DIM)),
            Span::styled("[Blog]", Style::new().fg(theme::HOVER)),
            Span::styled(" or ", Style::new().fg(theme::DIM)),
            Span::styled("[About]", Style::new().fg(theme::HOVER)),
            Span::styled("  ·  click to enter", Style::new().fg(theme::DIM)),
        ]))
        .alignment(Alignment::Center)
        .bg(theme::BG),
        footer,
    );
}

fn render_cards(ctx: &mut FrameCtx<'_>, frame: &mut Frame<'_>, area: Rect) {
    let [_, row, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(5),
        Constraint::Fill(1),
    ])
    .areas(area);

    let [blog, _, about] = Layout::horizontal([
        Constraint::Length(22),
        Constraint::Length(3),
        Constraint::Length(22),
    ])
    .flex(Flex::Center)
    .areas(row);

    render_card(ctx, frame, blog, Route::Blog, "blog", "notes, tags, posts");
    render_card(
        ctx,
        frame,
        about,
        Route::About,
        "about",
        "who & how to reach",
    );
}

fn render_card(
    ctx: &mut FrameCtx<'_>,
    frame: &mut Frame<'_>,
    area: Rect,
    route: Route,
    title: &str,
    subtitle: &str,
) {
    let span = CellSpan::new(
        area.x,
        area.x.saturating_add(area.width),
        area.y,
        area.y.saturating_add(area.height),
    );
    ctx.hits.add(span, route.path());
    let hovered = ctx.mouse.hits(span);
    let accent = if hovered { theme::HOVER } else { theme::DIM };
    let title_style = if hovered {
        Style::new().fg(theme::HOVER).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(theme::FG)
    };

    let card = Paragraph::new(vec![
        Line::from(Span::styled(format!(" {title}"), title_style)),
        Line::from(Span::styled(
            format!(" {subtitle}"),
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
    .bg(theme::BG);

    frame.render_widget(card, area);
}
