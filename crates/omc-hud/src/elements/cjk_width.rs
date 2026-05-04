use unicode_width::UnicodeWidthStr;

pub fn display_width(value: &str) -> usize {
    UnicodeWidthStr::width(value)
}

pub fn render() -> Option<String> {
    let _ = display_width("");
    None
}
