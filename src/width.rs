use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Terminal display width of `text` (CJK = 2, ASCII = 1).
pub fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

pub fn char_width(ch: char) -> usize {
    UnicodeWidthChar::width(ch).unwrap_or(0)
}

/// Truncate `text` to `width` columns, adding an ellipsis when needed.
pub fn truncate_display(text: &str, width: usize) -> String {
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
        let cw = char_width(ch);
        if used + cw > budget {
            break;
        }
        out.push(ch);
        used += cw;
    }
    out.push('…');
    out
}

/// Rows a paragraph occupies when wrapped to `width` columns (trim: false).
pub fn wrapped_rows(text: &str, width: u16) -> u16 {
    let width = usize::from(width.max(1));
    let mut rows = 0_u16;
    for line in text.split('\n') {
        rows = rows.saturating_add(wrapped_line_rows(line, width));
    }
    rows.max(1)
}

fn wrapped_line_rows(line: &str, width: usize) -> u16 {
    if line.is_empty() {
        return 1;
    }
    let mut used = 0;
    let mut rows = 1_u16;
    for ch in line.chars() {
        let cw = char_width(ch).max(1);
        if used + cw > width && used > 0 {
            rows = rows.saturating_add(1);
            used = cw.min(width);
        } else {
            used = used.saturating_add(cw);
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::{display_width, truncate_display, wrapped_rows};

    #[test]
    fn truncate_keeps_display_width() {
        assert_eq!(truncate_display("abcdef", 10), "abcdef");
        assert_eq!(truncate_display("abcdef", 4), "abc…");
        assert_eq!(display_width(&truncate_display("한글제목", 5)), 5);
    }

    #[test]
    fn wrap_counts_soft_breaks() {
        assert_eq!(wrapped_rows("abcd", 2), 2);
        assert_eq!(wrapped_rows("ab\ncd", 10), 2);
        assert_eq!(wrapped_rows("", 10), 1);
    }
}
