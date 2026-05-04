use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

/// Severity tier based on percent usage.
fn severity(percent: u8) -> (&'static str, Option<&'static str>) {
    // Returns (ansi_color_code, suffix)
    if percent >= 90 {
        ("\x1b[31m", Some(" CRITICAL"))
    } else if percent >= 80 {
        ("\x1b[31m", Some(" COMPRESS?"))
    } else if percent >= 70 {
        ("\x1b[33m", None)
    } else {
        ("\x1b[32m", None)
    }
}

fn color_enabled(level: ColorLevel) -> bool {
    !matches!(level, ColorLevel::Mono)
}

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let tokens = ctx.input.context_window_tokens?;
    let max = ctx.input.context_window_max?;
    if max == 0 {
        return None;
    }

    let raw = (tokens as f64 / max as f64 * 100.0).round();
    let percent = raw.clamp(0.0, 100.0) as u8;

    // Use the i18n label (lowercased) so the Strings.ctx field stays live
    // and the output matches the spec's "ctx:NN%" format for EN locale.
    let label = ctx.strings.ctx.to_ascii_lowercase();

    let suffix = if percent >= 90 {
        " CRITICAL"
    } else if percent >= 80 {
        " COMPRESS?"
    } else {
        ""
    };

    if color_enabled(ctx.color_level) {
        let (color_code, _) = severity(percent);
        Some(format!("{label}:{color_code}{percent}%{suffix}\x1b[0m"))
    } else {
        Some(format!("{label}:{percent}%{suffix}"))
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

    fn make_input(tokens: Option<u64>, max: Option<u64>) -> Input {
        Input {
            context_window_tokens: tokens,
            context_window_max: max,
            ..Input::default()
        }
    }

    fn empty_cache() -> HudCache {
        HudCache::new("test".to_string())
    }

    /// Strip ANSI escape sequences for readability assertions.
    fn strip_ansi(s: &str) -> String {
        let mut out = String::new();
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                // skip until 'm'
                for ch in chars.by_ref() {
                    if ch == 'm' {
                        break;
                    }
                }
            } else {
                out.push(c);
            }
        }
        out
    }

    // --- None cases ---

    #[test]
    fn none_when_max_is_none() {
        let input = make_input(Some(1000), None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        assert_eq!(render(&ctx), None);
    }

    #[test]
    fn none_when_tokens_is_none() {
        let input = make_input(None, Some(10000));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        assert_eq!(render(&ctx), None);
    }

    #[test]
    fn none_when_max_is_zero() {
        let input = make_input(Some(1000), Some(0));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        assert_eq!(render(&ctx), None);
    }

    // --- 0% boundary ---

    #[test]
    fn zero_percent_no_color() {
        let input = make_input(Some(0), Some(10000));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render(&ctx).unwrap();
        assert_eq!(result, "ctx:0%");
    }

    // --- 67% without color ---

    #[test]
    fn sixty_seven_percent_no_color() {
        let input = make_input(Some(6700), Some(10000));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render(&ctx).unwrap();
        assert_eq!(result, "ctx:67%");
    }

    // --- 67% with TrueColor: must have green ANSI + reset ---

    #[test]
    fn sixty_seven_percent_truecolor_has_green_and_reset() {
        let input = make_input(Some(6700), Some(10000));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let result = render(&ctx).unwrap();
        // green code
        assert!(
            result.contains("\x1b[32m"),
            "should contain green: {result:?}"
        );
        // reset
        assert!(
            result.contains("\x1b[0m"),
            "should contain reset: {result:?}"
        );
        // plain text content
        assert_eq!(strip_ansi(&result), "ctx:67%");
    }

    // --- 70% threshold: yellow ---

    #[test]
    fn seventy_percent_is_yellow() {
        let input = make_input(Some(7000), Some(10000));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[33m"),
            "should contain yellow: {result:?}"
        );
        assert_eq!(strip_ansi(&result), "ctx:70%");
    }

    // --- 79% also yellow ---

    #[test]
    fn seventy_nine_percent_is_yellow() {
        let input = make_input(Some(7900), Some(10000));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[33m"),
            "should contain yellow: {result:?}"
        );
        assert_eq!(strip_ansi(&result), "ctx:79%");
    }

    // --- 80% threshold: red + COMPRESS? ---

    #[test]
    fn eighty_percent_is_red_with_compress() {
        let input = make_input(Some(8000), Some(10000));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[31m"),
            "should contain red: {result:?}"
        );
        assert_eq!(strip_ansi(&result), "ctx:80% COMPRESS?");
    }

    // --- 89% also red + COMPRESS? ---

    #[test]
    fn eighty_nine_percent_is_red_with_compress() {
        let input = make_input(Some(8900), Some(10000));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[31m"),
            "should contain red: {result:?}"
        );
        assert_eq!(strip_ansi(&result), "ctx:89% COMPRESS?");
    }

    // --- 90% threshold: red + CRITICAL ---

    #[test]
    fn ninety_percent_is_red_with_critical() {
        let input = make_input(Some(9000), Some(10000));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[31m"),
            "should contain red: {result:?}"
        );
        assert_eq!(strip_ansi(&result), "ctx:90% CRITICAL");
    }

    // --- 100% red + CRITICAL ---

    #[test]
    fn one_hundred_percent_is_red_with_critical() {
        let input = make_input(Some(10000), Some(10000));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[31m"),
            "should contain red: {result:?}"
        );
        assert_eq!(strip_ansi(&result), "ctx:100% CRITICAL");
    }

    // --- Tokens > max clamps to 100% ---

    #[test]
    fn tokens_exceeding_max_clamps_to_hundred() {
        let input = make_input(Some(15000), Some(10000));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert_eq!(strip_ansi(&result), "ctx:100% CRITICAL");
    }

    // --- Color256 also emits color ---

    #[test]
    fn color256_emits_ansi() {
        let input = make_input(Some(6700), Some(10000));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color256);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[32m"),
            "Color256 should emit green: {result:?}"
        );
    }

    // --- Exact string for no-color 67% ---

    #[test]
    fn exact_string_no_color_67() {
        let input = make_input(Some(6700), Some(10000));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "ctx:67%");
    }

    // --- Exact colored string for TrueColor 67% (Color16 codes) ---

    #[test]
    fn exact_colored_string_16_67() {
        let input = make_input(Some(6700), Some(10000));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        assert_eq!(render(&ctx).unwrap(), "ctx:\x1b[32m67%\x1b[0m");
    }
}
