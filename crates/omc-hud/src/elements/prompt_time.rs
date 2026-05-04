use crate::cache::now_ms;
use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

fn color_enabled(level: ColorLevel) -> bool {
    !matches!(level, ColorLevel::Mono)
}

/// Format elapsed milliseconds into a human-readable string.
/// <60s -> "Ns", <60m -> "Nm", <24h -> "Nh", else -> "Nd"
fn format_elapsed(ms: u64) -> String {
    let secs = ms / 1_000;
    if secs < 60 {
        return format!("{secs}s");
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{mins}m");
    }
    let hours = mins / 60;
    if hours < 24 {
        return format!("{hours}h");
    }
    let days = hours / 24;
    format!("{days}d")
}

/// Choose an ANSI color code based on elapsed milliseconds.
/// Returns `None` when no coloring is desired (30s-2m tier = default white).
fn color_for_elapsed(ms: u64) -> Option<&'static str> {
    let secs = ms / 1_000;
    if secs < 30 {
        Some("\x1b[32m") // green
    } else if secs < 120 {
        None // default / white tier
    } else if secs < 300 {
        Some("\x1b[33m") // yellow
    } else {
        Some("\x1b[31m") // red
    }
}

/// Internal implementation parameterised over the current time so tests can
/// inject synthetic timestamps without mocking the clock.
fn render_at(ctx: &RenderContext<'_>, now: u64) -> Option<String> {
    let start = ctx.input.prompt_start_ms?;
    if start == 0 {
        return None;
    }

    // Clamp negative elapsed (clock skew) to 0.
    let elapsed_ms = now.saturating_sub(start);

    let time_str = format_elapsed(elapsed_ms);

    if color_enabled(ctx.color_level) && let Some(color) = color_for_elapsed(elapsed_ms) {
        return Some(format!("{color}{time_str}\x1b[0m"));
    }

    Some(time_str)
}

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    render_at(ctx, now_ms())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::HudCache;
    use crate::i18n;
    use crate::input::Input;

    // Helpers ----------------------------------------------------------------

    fn make_ctx<'a>(input: &'a Input, cache: &'a HudCache, level: ColorLevel) -> RenderContext<'a> {
        RenderContext {
            input,
            cache,
            color_level: level,
            strings: i18n::strings(i18n::detect_locale()),
        }
    }

    fn make_input(prompt_start_ms: Option<u64>) -> Input {
        Input {
            prompt_start_ms,
            ..Input::default()
        }
    }

    fn empty_cache() -> HudCache {
        HudCache::new("test".to_string())
    }

    // Convenience: build a `now` that is `delta_ms` milliseconds after `start`.
    fn now_after(start: u64, delta_ms: u64) -> u64 {
        start + delta_ms
    }

    // Strip ANSI escapes for plain-text assertions.
    fn strip_ansi(s: &str) -> String {
        let mut out = String::new();
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
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

    // --- None cases ---------------------------------------------------------

    #[test]
    fn none_when_prompt_start_is_none() {
        let input = make_input(None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        // Any `now` is fine since start is None.
        assert_eq!(render_at(&ctx, 1_000_000), None);
    }

    #[test]
    fn none_when_prompt_start_is_zero() {
        let input = make_input(Some(0));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        assert_eq!(render_at(&ctx, 1_000_000), None);
    }

    // --- Formatting ---------------------------------------------------------

    #[test]
    fn five_seconds_elapsed() {
        let start = 1_000_000_u64;
        let input = make_input(Some(start));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let now = now_after(start, 5_000);
        let result = render_at(&ctx, now).unwrap();
        assert_eq!(strip_ansi(&result), "5s");
    }

    #[test]
    fn thirty_seconds_boundary() {
        let start = 1_000_000_u64;
        let input = make_input(Some(start));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let now = now_after(start, 30_000);
        let result = render_at(&ctx, now).unwrap();
        assert_eq!(strip_ansi(&result), "30s");
    }

    #[test]
    fn fifty_nine_seconds() {
        let start = 1_000_000_u64;
        let input = make_input(Some(start));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let now = now_after(start, 59_000);
        let result = render_at(&ctx, now).unwrap();
        assert_eq!(strip_ansi(&result), "59s");
    }

    #[test]
    fn sixty_seconds_becomes_one_minute() {
        let start = 1_000_000_u64;
        let input = make_input(Some(start));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let now = now_after(start, 60_000);
        let result = render_at(&ctx, now).unwrap();
        assert_eq!(strip_ansi(&result), "1m");
    }

    #[test]
    fn five_minutes() {
        let start = 1_000_000_u64;
        let input = make_input(Some(start));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let now = now_after(start, 5 * 60 * 1_000);
        let result = render_at(&ctx, now).unwrap();
        assert_eq!(strip_ansi(&result), "5m");
    }

    #[test]
    fn fifty_nine_minutes() {
        let start = 1_000_000_u64;
        let input = make_input(Some(start));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let now = now_after(start, 59 * 60 * 1_000);
        let result = render_at(&ctx, now).unwrap();
        assert_eq!(strip_ansi(&result), "59m");
    }

    #[test]
    fn sixty_minutes_becomes_one_hour() {
        let start = 1_000_000_u64;
        let input = make_input(Some(start));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let now = now_after(start, 60 * 60 * 1_000);
        let result = render_at(&ctx, now).unwrap();
        assert_eq!(strip_ansi(&result), "1h");
    }

    #[test]
    fn twenty_five_hours_becomes_one_day() {
        let start = 1_000_000_u64;
        let input = make_input(Some(start));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let now = now_after(start, 25 * 60 * 60 * 1_000);
        let result = render_at(&ctx, now).unwrap();
        assert_eq!(strip_ansi(&result), "1d");
    }

    // --- Color gating -------------------------------------------------------

    #[test]
    fn truecolor_five_seconds_is_green() {
        let start = 1_000_000_u64;
        let input = make_input(Some(start));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let now = now_after(start, 5_000);
        let result = render_at(&ctx, now).unwrap();
        assert!(
            result.contains("\x1b[32m"),
            "5s should be green: {result:?}"
        );
        assert!(result.contains("\x1b[0m"), "should have reset: {result:?}");
        assert_eq!(strip_ansi(&result), "5s");
    }

    #[test]
    fn truecolor_three_minutes_is_yellow() {
        let start = 1_000_000_u64;
        let input = make_input(Some(start));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let now = now_after(start, 3 * 60 * 1_000);
        let result = render_at(&ctx, now).unwrap();
        assert!(
            result.contains("\x1b[33m"),
            "3m should be yellow: {result:?}"
        );
        assert_eq!(strip_ansi(&result), "3m");
    }

    #[test]
    fn truecolor_ten_minutes_is_red() {
        let start = 1_000_000_u64;
        let input = make_input(Some(start));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let now = now_after(start, 10 * 60 * 1_000);
        let result = render_at(&ctx, now).unwrap();
        assert!(result.contains("\x1b[31m"), "10m should be red: {result:?}");
        assert_eq!(strip_ansi(&result), "10m");
    }

    // 30s-2m tier: no color even with TrueColor
    #[test]
    fn truecolor_one_minute_no_color_code() {
        let start = 1_000_000_u64;
        let input = make_input(Some(start));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let now = now_after(start, 60_000);
        let result = render_at(&ctx, now).unwrap();
        assert!(
            !result.contains('\x1b'),
            "1m should have no ANSI code: {result:?}"
        );
        assert_eq!(result, "1m");
    }

    // --- Clock skew (negative elapsed) clamped to 0 -------------------------

    #[test]
    fn negative_elapsed_clamped_to_zero_seconds() {
        // now < start simulates clock skew
        let start = 1_000_000_u64;
        let input = make_input(Some(start));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let now = start - 5_000; // 5 seconds in the past
        let result = render_at(&ctx, now).unwrap();
        assert_eq!(strip_ansi(&result), "0s");
    }

    // --- Mono suppresses all ANSI -------------------------------------------

    #[test]
    fn mono_suppresses_ansi_even_for_long_elapsed() {
        let start = 1_000_000_u64;
        let input = make_input(Some(start));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let now = now_after(start, 10 * 60 * 1_000);
        let result = render_at(&ctx, now).unwrap();
        assert!(
            !result.contains('\x1b'),
            "mono should have no ANSI: {result:?}"
        );
        assert_eq!(result, "10m");
    }
}
