//! `color_degrade` — terminal-capability detection utility.
//!
//! [`detect_color_level`] inspects `NO_COLOR`, `FORCE_COLOR`, `COLORTERM`,
//! and `TERM` to pick the highest color level the terminal can render.
//! Called once per HUD invocation (in `main`) and threaded into every
//! [`super::RenderContext`] via [`crate::terminal::ColorLevel`]. Other
//! elements gate their ANSI escape emission on the resulting level.
//!
//! As an [`super::Element`] slot this surface produces no visible output —
//! the work has already happened by the time elements render.

use std::env;

use crate::terminal::ColorLevel;

pub fn detect_color_level() -> ColorLevel {
    if env::var_os("NO_COLOR").is_some() {
        return ColorLevel::Mono;
    }

    // skipcq: RS-W1015
    let force_color = env::var("FORCE_COLOR").unwrap_or_default();
    if !force_color.is_empty() && force_color != "0" {
        return match force_color.as_str() {
            "1" => ColorLevel::Color16,
            "2" => ColorLevel::Color256,
            "3" => ColorLevel::TrueColor,
            _ => ColorLevel::Color256,
        };
    }

    // skipcq: RS-W1015
    let colorterm = env::var("COLORTERM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if colorterm.contains("truecolor") || colorterm.contains("24bit") {
        return ColorLevel::TrueColor;
    }

    // skipcq: RS-W1015
    let term = env::var("TERM").unwrap_or_default().to_ascii_lowercase();
    if term.contains("256color") {
        return ColorLevel::Color256;
    }
    if term.contains("xterm") || term.contains("ansi") || term.contains("vt1") {
        return ColorLevel::Color16;
    }

    // Windows-specific: TERM is often unset even in capable terminals (Warp, Windows Terminal).
    // Check known Windows terminal indicators before falling back to Mono.
    if env::var_os("WT_SESSION").is_some()   // Windows Terminal
        || env::var_os("WT_PROFILE_ID").is_some()
        || env::var("TERM_PROGRAM")
            .map(|v| !v.is_empty())
            .unwrap_or(false)
    {
        return ColorLevel::TrueColor;
    }

    // On Windows with no color env vars set, default to Color256 — modern Windows
    // terminals all support color and the Unix "empty TERM = dumb terminal" heuristic
    // does not apply.
    if cfg!(target_os = "windows") {
        return ColorLevel::Color256;
    }

    if term.is_empty() || term == "dumb" {
        ColorLevel::Mono
    } else {
        ColorLevel::Color16
    }
}

/// Utility element — never produces visible output.
/// Color-level detection happens once in `main` and propagates through
/// `RenderContext::color_level`; consumers gate ANSI escapes on it.
pub fn render() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_always_returns_none() {
        assert_eq!(render(), None);
    }
}
