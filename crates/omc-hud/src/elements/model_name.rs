use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

/// Map a raw model identifier to a friendly display label.
/// Returns (label, family) where family is "opus" | "sonnet" | "haiku" | "unknown".
fn parse_model(model: &str) -> (&'static str, &'static str) {
    // Specific versioned patterns (check before generic family patterns)
    if model.contains("opus-4-7") {
        return ("Opus 4.7", "opus");
    }
    if model.contains("opus-4-6") {
        return ("Opus 4.6", "opus");
    }
    if model.contains("opus-4-5") {
        return ("Opus 4.5", "opus");
    }
    if model.contains("sonnet-4-6") {
        return ("Sonnet 4.6", "sonnet");
    }
    if model.contains("sonnet-4-5") {
        return ("Sonnet 4.5", "sonnet");
    }
    if model.contains("haiku-4-6") {
        return ("Haiku 4.6", "haiku");
    }
    if model.contains("haiku-4-5") {
        return ("Haiku 4.5", "haiku");
    }
    // Generic family fallbacks
    if model.contains("opus") {
        return ("Opus", "opus");
    }
    if model.contains("sonnet") {
        return ("Sonnet", "sonnet");
    }
    if model.contains("haiku") {
        return ("Haiku", "haiku");
    }
    // Unknown: passthrough truncated to 20 chars
    ("", "unknown")
}

fn color_for_family(family: &str) -> &'static str {
    match family {
        "opus" => "\x1b[35m",   // magenta
        "sonnet" => "\x1b[33m", // yellow
        "haiku" => "\x1b[32m",  // green
        _ => "",
    }
}

fn color_enabled(level: ColorLevel) -> bool {
    !matches!(level, ColorLevel::Mono)
}

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let raw = ctx.input.model.as_deref()?.trim();
    if raw.is_empty() {
        return None;
    }

    let (label, family) = parse_model(raw);

    let display: &str = if label.is_empty() {
        // Unknown model: truncate to 20 chars (ASCII labels only)
        &raw[..raw.len().min(20)]
    } else {
        label
    };

    if color_enabled(ctx.color_level) && !family.eq("unknown") {
        let color = color_for_family(family);
        Some(format!("{color}{display}\x1b[0m"))
    } else {
        Some(display.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::HudCache;
    use crate::i18n;
    use crate::input::Input;

    fn make_ctx<'a>(input: &'a Input, cache: &'a HudCache, level: ColorLevel) -> RenderContext<'a> {
        RenderContext {
            input,
            cache,
            color_level: level,
            strings: i18n::strings(i18n::detect_locale()),
        }
    }

    fn make_input(model: Option<&str>) -> Input {
        Input {
            model: model.map(|s| s.to_string()),
            ..Input::default()
        }
    }

    fn empty_cache() -> HudCache {
        HudCache::new("test".to_string())
    }

    // --- None cases ---

    #[test]
    fn none_when_model_is_none() {
        let input = make_input(None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        assert_eq!(render(&ctx), None);
    }

    #[test]
    fn none_when_model_is_empty_string() {
        let input = make_input(Some(""));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        assert_eq!(render(&ctx), None);
    }

    // --- Specific model parsing (no color) ---

    #[test]
    fn opus_4_7_no_color() {
        let input = make_input(Some("claude-opus-4-7"));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "Opus 4.7");
    }

    #[test]
    fn opus_4_7_with_date_suffix() {
        let input = make_input(Some("claude-opus-4-7-20251022"));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "Opus 4.7");
    }

    #[test]
    fn sonnet_4_6_no_color() {
        let input = make_input(Some("claude-sonnet-4-6"));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "Sonnet 4.6");
    }

    #[test]
    fn haiku_4_5_no_color() {
        let input = make_input(Some("claude-haiku-4-5"));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "Haiku 4.5");
    }

    // --- Color tests ---

    #[test]
    fn opus_truecolor_has_magenta_and_reset() {
        let input = make_input(Some("claude-opus-4-7"));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[35m"),
            "opus should be magenta: {result:?}"
        );
        assert!(result.contains("\x1b[0m"), "should have reset: {result:?}");
    }

    #[test]
    fn sonnet_truecolor_has_yellow() {
        let input = make_input(Some("claude-sonnet-4-6"));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[33m"),
            "sonnet should be yellow: {result:?}"
        );
    }

    #[test]
    fn haiku_truecolor_has_green() {
        let input = make_input(Some("claude-haiku-4-5"));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[32m"),
            "haiku should be green: {result:?}"
        );
    }

    // --- Generic family fallback (no version) ---

    #[test]
    fn generic_opus_variant() {
        let input = make_input(Some("some-opus-variant"));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "Opus");
    }

    // --- Unknown model passthrough truncated to 20 chars ---

    #[test]
    fn unknown_model_passthrough() {
        let input = make_input(Some("gpt-4"));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "gpt-4");
    }

    #[test]
    fn unknown_model_truncated_to_20_chars() {
        // 25 chars: "abcdefghijklmnopqrstuvwxy"
        let long_id = "abcdefghijklmnopqrstuvwxy";
        let input = make_input(Some(long_id));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render(&ctx).unwrap();
        assert_eq!(result.len(), 20);
        assert_eq!(&result, "abcdefghijklmnopqrst");
    }

    // --- Color16 also emits color ---

    #[test]
    fn color16_emits_ansi_for_opus() {
        let input = make_input(Some("claude-opus-4-7"));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[35m"),
            "Color16 should emit magenta: {result:?}"
        );
    }

    // --- Exact colored string (Color16) ---

    #[test]
    fn exact_colored_opus_4_7() {
        let input = make_input(Some("claude-opus-4-7"));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        assert_eq!(render(&ctx).unwrap(), "\x1b[35mOpus 4.7\x1b[0m");
    }
}
