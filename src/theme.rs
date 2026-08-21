use ratatui::style::{Color, Modifier, Style};

/// Terminal background used by every panel.
pub const BG: Color = Color::Rgb(28, 25, 22);
pub const FG: Color = Color::White;
pub const HOVER: Color = Color::Green;
pub const DIM: Color = Color::Rgb(140, 130, 120);

/// Set to `Some(1..=4)` to lock a code style. `None` shows all four samples.
pub const CODE_STYLE_SAMPLE: Option<u8> = Some(1);

/// Four numbered code-block looks. Change [`CODE_STYLE_SAMPLE`] to pick one.
pub fn code_block_style(sample: u8) -> Style {
    match sample {
        1 => Style::new()
            .fg(Color::Rgb(171, 178, 191))
            .bg(Color::Rgb(40, 44, 52)),
        2 => Style::new()
            .fg(Color::Rgb(163, 190, 140))
            .bg(Color::Rgb(46, 52, 64))
            .add_modifier(Modifier::BOLD),
        3 => Style::new()
            .fg(Color::Rgb(88, 86, 82))
            .bg(Color::Rgb(253, 246, 227)),
        _ => Style::new()
            .fg(Color::Rgb(230, 219, 116))
            .bg(Color::Rgb(39, 40, 34)),
    }
}

#[allow(dead_code)]
pub fn code_sample_label(sample: u8) -> &'static str {
    match sample {
        1 => "1  one-dark",
        2 => "2  nord",
        3 => "3  solarized-light",
        _ => "4  monokai",
    }
}
