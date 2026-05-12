//! `i18n` — locale detection and per-locale string tables.
//!
//! [`detect_locale`] inspects `LC_ALL` and `LANG` once per HUD invocation
//! (in `main`) and returns a [`Locale`]. [`strings`] maps that locale to a
//! `&'static Strings` table. Other elements consume the table via
//! [`crate::elements::RenderContext::strings`] when they need a localized
//! label (e.g. `todos.rs`, `autopilot.rs`).
//!
//! As an [`crate::elements::Element`] slot this surface produces no
//! visible output — locale work has already happened by the time elements
//! render.

use std::env;

#[derive(Debug, Clone, Copy)]
pub enum Locale {
    En,
    ZhCn,
}

#[derive(Debug, Clone, Copy)]
pub struct Strings {
    pub ctx: &'static str,
    pub tok: &'static str,
    pub todo: &'static str,
    pub autopilot: &'static str,
}

const EN: Strings = Strings {
    ctx: "CTX",
    tok: "tok",
    todo: "TODO",
    autopilot: "autopilot",
};

const ZH_CN: Strings = Strings {
    ctx: "上下文",
    tok: "词元",
    todo: "待办",
    autopilot: "自动",
};

pub fn detect_locale() -> Locale {
    // skipcq: RS-W1015
    let locale = env::var("LC_ALL")
        // skipcq: RS-W1015
        .or_else(|_| env::var("LANG"))
        .unwrap_or_default();
    if locale.to_ascii_lowercase().contains("zh") {
        Locale::ZhCn
    } else {
        Locale::En
    }
}

pub fn strings(locale: Locale) -> &'static Strings {
    match locale {
        Locale::En => &EN,
        Locale::ZhCn => &ZH_CN,
    }
}

/// Utility element — never produces visible output.
/// i18n string tables are consumed via `RenderContext::strings`; the
/// `Element::I18n` enum arm exists so layout configuration can name it
/// without special-casing.
pub fn render_element() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_element_always_returns_none() {
        assert_eq!(render_element(), None);
    }

    #[test]
    fn en_strings_are_ascii() {
        assert!(EN.ctx.is_ascii());
        assert!(EN.tok.is_ascii());
    }

    #[test]
    fn zh_cn_strings_are_non_empty() {
        assert!(!ZH_CN.ctx.is_empty());
        assert!(!ZH_CN.tok.is_empty());
        assert!(!ZH_CN.todo.is_empty());
        assert!(!ZH_CN.autopilot.is_empty());
    }
}
