use crate::cache::now_ms;
use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

fn color_enabled(level: ColorLevel) -> bool {
    !matches!(level, ColorLevel::Mono)
}

/// Format elapsed milliseconds — JS-compatible format:
///   < 60s      → "13s"
///   1m–59m59s  → "1m23s" (seconds omitted if zero: "5m")
///   1h–23h59m  → "2h3m"  (minutes omitted if zero: "1h")
///   >= 24h     → "1d"    (floor days, no hours shown)
fn format_elapsed(ms: u64) -> String {
    let total_secs = ms / 1_000;
    if total_secs < 60 {
        return format!("{total_secs}s");
    }
    let total_mins = total_secs / 60;
    let rem_secs = total_secs % 60;
    if total_mins < 60 {
        if rem_secs == 0 {
            return format!("{total_mins}m");
        }
        return format!("{total_mins}m{rem_secs}s");
    }
    let total_hours = total_mins / 60;
    let rem_mins = total_mins % 60;
    if total_hours < 24 {
        if rem_mins == 0 {
            return format!("{total_hours}h");
        }
        return format!("{total_hours}h{rem_mins}m");
    }
    let days = total_hours / 24;
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

/// Resolve the prompt start timestamp (ms since epoch).
/// Priority: omc_state.hud.last_prompt_timestamp → stdin.prompt_start_ms
fn resolve_start_ms(ctx: &RenderContext<'_>) -> Option<u64> {
    // Primary: hud-state.json lastPromptTimestamp (ISO-8601 or Unix ms string)
    if let Some(ts_str) = ctx
        .omc_state
        .hud
        .as_ref()
        .and_then(|h| h.last_prompt_timestamp.as_deref())
    {
        // Try parsing as Unix ms integer string first
        if let Ok(ms) = ts_str.parse::<u64>() {
            return Some(ms);
        }
        // Try ISO-8601
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts_str) {
            let ms = dt.timestamp_millis();
            if ms > 0 {
                return Some(ms as u64);
            }
        }
    }
    // Fallback: stdin prompt_start_ms
    ctx.input.prompt_start_ms.filter(|&v| v > 0)
}

/// Core rendering logic, parameterised over `now` so tests are deterministic.
/// Returns just the elapsed time string without emoji or wall clock.
fn render_at(ctx: &RenderContext<'_>, now: u64) -> Option<String> {
    let start = resolve_start_ms(ctx)?;

    // Clamp negative elapsed (clock skew) to 0.
    let elapsed_ms = now.saturating_sub(start);
    let time_str = format_elapsed(elapsed_ms);

    if color_enabled(ctx.color_level) {
        if let Some(color) = color_for_elapsed(elapsed_ms) {
            return Some(format!("{color}{time_str}\x1b[0m"));
        }
        // White/default tier: no color code at all
        return Some(time_str);
    }

    Some(time_str)
}

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let now = now_ms();
    let base = render_at(ctx, now)?;
    // Wall clock time (local timezone)
    let clock = chrono::Local::now().format("%H:%M:%S").to_string();
    if color_enabled(ctx.color_level) {
        Some(format!("⏱{base} \x1b[2m{clock}\x1b[0m"))
    } else {
        Some(format!("⏱{base} {clock}"))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::HudCache;
    use crate::input::Input;

    // Helpers ----------------------------------------------------------------

    fn make_ctx<'a>(input: &'a Input, cache: &'a HudCache, level: ColorLevel) -> RenderContext<'a> {
        RenderContext::for_test(input, cache, level)
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

    fn now_after(start: u64, delta_ms: u64) -> u64 {
        start + delta_ms
    }

    fn strip_ansi(s: &str) -> String {
        let mut out = String::default();
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
    fn one_minute_twenty_three_seconds() {
        let start = 1_000_000_u64;
        let input = make_input(Some(start));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let now = now_after(start, 83_000); // 1m23s
        let result = render_at(&ctx, now).unwrap();
        assert_eq!(strip_ansi(&result), "1m23s");
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
    fn two_hours_three_minutes() {
        let start = 1_000_000_u64;
        let input = make_input(Some(start));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let now = now_after(start, (2 * 60 + 3) * 60 * 1_000); // 2h3m
        let result = render_at(&ctx, now).unwrap();
        assert_eq!(strip_ansi(&result), "2h3m");
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

    // --- format_elapsed unit tests ------------------------------------------

    #[test]
    fn elapsed_0s() {
        assert_eq!(format_elapsed(0), "0s");
    }

    #[test]
    fn elapsed_exact_1min_no_seconds() {
        assert_eq!(format_elapsed(60_000), "1m");
    }

    #[test]
    fn elapsed_1m30s() {
        assert_eq!(format_elapsed(90_000), "1m30s");
    }

    #[test]
    fn elapsed_exact_1h_no_minutes() {
        assert_eq!(format_elapsed(3_600_000), "1h");
    }

    #[test]
    fn elapsed_2h3m() {
        assert_eq!(format_elapsed((2 * 60 + 3) * 60 * 1_000), "2h3m");
    }

    #[test]
    fn elapsed_24h_becomes_1d() {
        assert_eq!(format_elapsed(24 * 3_600_000), "1d");
    }

    #[test]
    fn elapsed_25h_becomes_1d() {
        assert_eq!(format_elapsed(25 * 3_600_000), "1d");
    }

    #[test]
    fn elapsed_48h_becomes_2d() {
        assert_eq!(format_elapsed(48 * 3_600_000), "2d");
    }
}
