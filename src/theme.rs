use ratatui::style::{Color, Style};

/// Terminal background used by every panel.
pub const BG: Color = Color::Rgb(28, 25, 22);
pub const FG: Color = Color::White;
pub const HOVER: Color = Color::Green;
pub const DIM: Color = Color::Rgb(140, 130, 120);

pub fn code_block_style() -> Style {
    Style::new()
        .fg(Color::Rgb(171, 178, 191))
        .bg(Color::Rgb(40, 44, 52))
}
