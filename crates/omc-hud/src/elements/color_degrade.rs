use std::env;

use crate::terminal::ColorLevel;

pub fn detect_color_level() -> ColorLevel {
    if env::var_os("NO_COLOR").is_some() {
        return ColorLevel::Mono;
    }

    let force_color = env::var("FORCE_COLOR").unwrap_or_default();
    if !force_color.is_empty() && force_color != "0" {
        return match force_color.as_str() {
            "1" => ColorLevel::Color16,
            "2" => ColorLevel::Color256,
            "3" => ColorLevel::TrueColor,
            _ => ColorLevel::Color256,
        };
    }

    let colorterm = env::var("COLORTERM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if colorterm.contains("truecolor") || colorterm.contains("24bit") {
        return ColorLevel::TrueColor;
    }

    let term = env::var("TERM").unwrap_or_default().to_ascii_lowercase();
    if term.contains("256color") {
        ColorLevel::Color256
    } else if term.is_empty() || term == "dumb" {
        ColorLevel::Mono
    } else {
        ColorLevel::Color16
    }
}

pub fn render() -> Option<String> {
    None
}
