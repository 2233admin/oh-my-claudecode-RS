#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorLevel {
    TrueColor,
    Color256,
    Color16,
    Mono,
}

#[derive(Debug, Clone, Copy)]
pub enum SemanticColor {
    Green,
    Yellow,
    Red,
}

pub fn paint(level: ColorLevel, color: SemanticColor, text: impl AsRef<str>) -> String {
    let text = text.as_ref();
    match level {
        ColorLevel::Mono => text.to_string(),
        ColorLevel::TrueColor => {
            let (r, g, b) = match color {
                SemanticColor::Green => (74, 222, 128),
                SemanticColor::Yellow => (250, 204, 21),
                SemanticColor::Red => (248, 113, 113),
            };
            format!("\x1b[38;2;{r};{g};{b}m{text}\x1b[0m")
        }
        ColorLevel::Color256 => {
            let code = match color {
                SemanticColor::Green => 120,
                SemanticColor::Yellow => 220,
                SemanticColor::Red => 203,
            };
            format!("\x1b[38;5;{code}m{text}\x1b[0m")
        }
        ColorLevel::Color16 => {
            let code = match color {
                SemanticColor::Green => 32,
                SemanticColor::Yellow => 33,
                SemanticColor::Red => 31,
            };
            format!("\x1b[{code}m{text}\x1b[0m")
        }
    }
}
