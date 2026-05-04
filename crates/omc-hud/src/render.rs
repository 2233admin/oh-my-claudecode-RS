use crate::cache::HudCache;
use crate::elements::{DEFAULT_ELEMENTS, RenderContext, render_element};
use crate::i18n::Strings;
use crate::input::Input;
use crate::terminal::ColorLevel;

pub fn render_statusline(
    input: &Input,
    cache: &HudCache,
    color_level: ColorLevel,
    strings: &'static Strings,
) -> String {
    let ctx = RenderContext {
        input,
        cache,
        color_level,
        strings,
    };

    DEFAULT_ELEMENTS
        .iter()
        .filter_map(|element| render_element(*element, &ctx))
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("[{value}]"))
        .collect::<Vec<_>>()
        .join(" | ")
}
