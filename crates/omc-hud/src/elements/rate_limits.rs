use crate::cache::now_ms;
use crate::elements::RenderContext;
use crate::terminal::ColorLevel;
use chrono;

// ---------------------------------------------------------------------------
// Internal implementation (parameterised over now so tests are deterministic)
// ---------------------------------------------------------------------------

/// Format reset countdown as `~Xm`, `~Xh`, or `~Xd` — ceiling values.
fn format_countdown(remaining_ms: u64) -> Option<String> {
    if remaining_ms == 0 {
        return None;
    }
    const MIN_MS: u64 = 60_000;
    const HOUR_MS: u64 = 3_600_000;
    const DAY_MS: u64 = 86_400_000;
    if remaining_ms < HOUR_MS {
        let mins = remaining_ms.div_ceil(MIN_MS).max(1);
        Some(format!("~{mins}m"))
    } else if remaining_ms < DAY_MS {
        let hours = remaining_ms.div_ceil(HOUR_MS);
        Some(format!("~{hours}h"))
    } else {
        let days = remaining_ms.div_ceil(DAY_MS);
        Some(format!("~{days}d"))
    }
}

/// Render a 10-char filled/empty progress bar, e.g. `[███░░░░░░░]`.
fn render_bar(pct: u8, width: usize) -> String {
    let filled = ((pct as usize * width + 50) / 100).min(width);
    let empty = width - filled;
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

/// ANSI color code for a percentage (only the number, not the label).
fn severity_color(pct: u8) -> &'static str {
    if pct >= 90 {
        "\x1b[31m" // red
    } else if pct >= 70 {
        "\x1b[33m" // yellow
    } else {
        "\x1b[32m" // green
    }
}

fn color_enabled(level: ColorLevel) -> bool {
    !matches!(level, ColorLevel::Mono)
}

/// Format a single bucket.
/// useBars=true:  `5h:[███░░░░░░░]32% ~2h`
/// useBars=false: `5h:32% ~2h`
fn format_bucket(
    label: &str,
    pct: u8,
    reset_ms: Option<u64>,
    now: u64,
    level: ColorLevel,
    use_bars: bool,
) -> String {
    let pct_str = format!("{pct}%");
    let bar_str = if use_bars {
        render_bar(pct, 10)
    } else {
        String::new()
    };

    let countdown = reset_ms.and_then(|r| {
        let remaining = r.saturating_sub(now);
        format_countdown(remaining)
    });

    if color_enabled(level) {
        let color = severity_color(pct);
        let dim_label = format!("\x1b[2m{label}:\x1b[0m");
        let colored_val = format!("{color}{bar_str}{pct_str}\x1b[0m");
        match countdown {
            Some(cd) => format!("{dim_label}{colored_val} \x1b[2m{cd}\x1b[0m"),
            None => format!("{dim_label}{colored_val}"),
        }
    } else {
        let val = format!("{bar_str}{pct_str}");
        match countdown {
            Some(cd) => format!("{label}:{val} {cd}"),
            None => format!("{label}:{val}"),
        }
    }
}

/// Extract rate-limit fields from hooks_state JSON if present.
fn extract_from_hooks(
    ctx: &RenderContext<'_>,
) -> (Option<u8>, Option<u64>, Option<u8>, Option<u64>) {
    let Some(hs) = ctx.input.hooks_state.as_ref() else {
        return (None, None, None, None);
    };

    let five_pct = hs
        .get("five_hour_used_pct")
        .and_then(serde_json::Value::as_u64)
        .map(|v| v.min(100) as u8);
    let five_reset = hs
        .get("five_hour_reset_ms")
        .and_then(serde_json::Value::as_u64);
    let weekly_pct = hs
        .get("weekly_used_pct")
        .and_then(serde_json::Value::as_u64)
        .map(|v| v.min(100) as u8);
    let weekly_reset = hs
        .get("weekly_reset_ms")
        .and_then(serde_json::Value::as_u64);

    (five_pct, five_reset, weekly_pct, weekly_reset)
}

/// Parse `resets_at` which may be a Unix-seconds integer, a Unix-ms integer, or an ISO-8601 string.
fn parse_resets_at(v: &serde_json::Value) -> Option<u64> {
    if let Some(n) = v.as_u64() {
        // Claude Code sends Unix seconds (10-digit, < 1e12); ms would be 13-digit (>= 1e12)
        return Some(if n < 1_000_000_000_000 { n * 1_000 } else { n });
    }
    if let Some(s) = v.as_str()
        && let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s)
    {
        let ms = dt.timestamp_millis();
        if ms > 0 {
            return Some(ms as u64);
        }
    }
    None
}

/// Extract from stdin `rate_limits` field (highest priority — real-time from Claude Code).
fn extract_from_stdin(
    ctx: &RenderContext<'_>,
) -> (Option<u8>, Option<u64>, Option<u8>, Option<u64>) {
    let Some(rl) = ctx.input.rate_limits.as_ref() else {
        return (None, None, None, None);
    };
    let five_pct = rl
        .five_hour
        .as_ref()
        .and_then(|b| b.used_percentage)
        .map(|v| v.clamp(0.0, 100.0) as u8);
    let five_reset = rl
        .five_hour
        .as_ref()
        .and_then(|b| b.resets_at.as_ref())
        .and_then(parse_resets_at);
    let weekly_pct = rl
        .seven_day
        .as_ref()
        .and_then(|b| b.used_percentage)
        .map(|v| v.clamp(0.0, 100.0) as u8);
    let weekly_reset = rl
        .seven_day
        .as_ref()
        .and_then(|b| b.resets_at.as_ref())
        .and_then(parse_resets_at);
    (five_pct, five_reset, weekly_pct, weekly_reset)
}

fn render_at(ctx: &RenderContext<'_>, now: u64) -> Option<String> {
    // Priority 1: stdin rate_limits (real-time from Claude Code)
    let (mut five_pct, mut five_reset, mut weekly_pct, mut weekly_reset) = extract_from_stdin(ctx);

    // Priority 2: hooks_state (OMC-injected)
    if five_pct.is_none() && weekly_pct.is_none() {
        let hs = extract_from_hooks(ctx);
        (five_pct, five_reset, weekly_pct, weekly_reset) = hs;
    }

    // Priority 3: usage API cache for percentages (least fresh)
    if five_pct.is_none()
        && weekly_pct.is_none()
        && let Some(usage) = ctx.usage
    {
        five_pct = usage.five_hour_pct.map(|v| v.clamp(0.0, 100.0) as u8);
        five_reset = usage.five_hour_reset_ms;
        weekly_pct = usage.seven_day_pct.map(|v| v.clamp(0.0, 100.0) as u8);
        weekly_reset = usage.seven_day_reset_ms;
    }

    if five_pct.is_none() && weekly_pct.is_none() {
        return None;
    }

    // Supplement missing reset times from usage API even when stdin had percentages
    if let Some(usage) = ctx.usage {
        if five_reset.is_none() {
            five_reset = usage.five_hour_reset_ms;
        }
        if weekly_reset.is_none() {
            weekly_reset = usage.seven_day_reset_ms;
        }
    }

    // useBars defaults to true (JS default); set "useBars": false in omcHud.elements to disable
    let use_bars = ctx.config.element_enabled("useBars", true);

    let mut parts: Vec<String> = Vec::new();

    if let Some(pct) = five_pct {
        parts.push(format_bucket(
            "5h",
            pct,
            five_reset,
            now,
            ctx.color_level,
            use_bars,
        ));
    }
    if let Some(pct) = weekly_pct {
        parts.push(format_bucket(
            "7d",
            pct,
            weekly_reset,
            now,
            ctx.color_level,
            use_bars,
        ));
    }

    // Sonnet / Opus model-specific weekly quotas from usage API
    if let Some(usage) = ctx.usage {
        if let Some(pct) = usage
            .seven_day_sonnet_pct
            .map(|v| v.clamp(0.0, 100.0) as u8)
            && pct > 0
        {
            parts.push(format_bucket(
                "sn",
                pct,
                None,
                now,
                ctx.color_level,
                use_bars,
            ));
        }
        if let Some(pct) = usage.seven_day_opus_pct.map(|v| v.clamp(0.0, 100.0) as u8)
            && pct > 0
        {
            parts.push(format_bucket(
                "op",
                pct,
                None,
                now,
                ctx.color_level,
                use_bars,
            ));
        }
    }

    Some(parts.join(" | "))
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
    use crate::input::Input;
    use serde_json::json;

    // --- Helpers ------------------------------------------------------------

    fn empty_cache() -> HudCache {
        HudCache::new("test".to_string())
    }

    fn make_ctx<'a>(input: &'a Input, cache: &'a HudCache, level: ColorLevel) -> RenderContext<'a> {
        RenderContext::for_test(input, cache, level)
    }

    /// Build a minimal Input with hooks_state containing rate-limit fields.
    fn make_input(
        five_pct: Option<u8>,
        five_reset_ms: Option<u64>,
        weekly_pct: Option<u8>,
        weekly_reset_ms: Option<u64>,
    ) -> Input {
        let mut obj = serde_json::Map::new();
        if let Some(v) = five_pct {
            obj.insert("five_hour_used_pct".to_string(), json!(v));
        }
        if let Some(v) = five_reset_ms {
            obj.insert("five_hour_reset_ms".to_string(), json!(v));
        }
        if let Some(v) = weekly_pct {
            obj.insert("weekly_used_pct".to_string(), json!(v));
        }
        if let Some(v) = weekly_reset_ms {
            obj.insert("weekly_reset_ms".to_string(), json!(v));
        }
        let hooks_state = if obj.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(obj))
        };
        Input {
            hooks_state,
            ..Input::default()
        }
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

    const BASE_NOW: u64 = 1_700_000_000_000; // arbitrary fixed "now" for tests

    // --- None cases ---------------------------------------------------------

    #[test]
    fn none_when_both_buckets_absent() {
        let input = Input::default();
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        assert_eq!(render_at(&ctx, BASE_NOW), None);
    }

    #[test]
    fn none_when_hooks_state_is_empty_object() {
        let input = Input {
            hooks_state: Some(json!({})),
            ..Input::default()
        };
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        assert_eq!(render_at(&ctx, BASE_NOW), None);
    }

    // --- Only 5h, no reset --------------------------------------------------

    #[test]
    fn only_5h_no_reset_no_color() {
        let input = make_input(Some(32), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        // 32% → (32*10+50)/100 = 3 filled → [███░░░░░░░]
        assert_eq!(result, "5h:[███░░░░░░░]32%");
    }

    #[test]
    fn only_5h_no_reset_with_color() {
        let input = make_input(Some(32), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert_eq!(strip_ansi(&result), "5h:[███░░░░░░░]32%");
        // 32% < 70 -> green
        assert!(result.contains("\x1b[32m"), "should be green: {result:?}");
        assert!(result.contains("\x1b[0m"), "should have reset: {result:?}");
    }

    // --- Only 5h with reset in 2h -------------------------------------------

    #[test]
    fn only_5h_with_reset_in_2h_no_color() {
        let reset_ms = BASE_NOW + 2 * 60 * 60 * 1000; // +2 hours
        let input = make_input(Some(32), Some(reset_ms), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert_eq!(result, "5h:[███░░░░░░░]32% ~2h");
    }

    // --- Only 5h with reset in 30m ------------------------------------------

    #[test]
    fn only_5h_with_reset_in_30m_no_color() {
        let reset_ms = BASE_NOW + 30 * 60 * 1000; // +30 min
        let input = make_input(Some(32), Some(reset_ms), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert_eq!(result, "5h:[███░░░░░░░]32% ~30m");
    }

    // --- Only 5h with reset in 25h ------------------------------------------

    #[test]
    fn only_5h_with_reset_in_25h() {
        let reset_ms = BASE_NOW + 25 * 60 * 60 * 1000; // +25 hours → ceiling 2d
        let input = make_input(Some(32), Some(reset_ms), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert_eq!(result, "5h:[███░░░░░░░]32% ~2d");
    }

    // --- Only 7d with reset -------------------------------------------------

    #[test]
    fn only_7d_with_reset_in_6d() {
        let reset_ms = BASE_NOW + 6 * 24 * 60 * 60 * 1000; // +6 days
        let input = make_input(None, None, Some(8), Some(reset_ms));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        // 8% → (8*10+50)/100 = 1 filled → [█░░░░░░░░░]
        assert_eq!(result, "7d:[█░░░░░░░░░]8% ~6d");
    }

    // --- Both with reset ----------------------------------------------------

    #[test]
    fn both_with_reset() {
        let reset_5h = BASE_NOW + 2 * 60 * 60 * 1000;
        let reset_7d = BASE_NOW + 6 * 24 * 60 * 60 * 1000;
        let input = make_input(Some(32), Some(reset_5h), Some(8), Some(reset_7d));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert_eq!(result, "5h:[███░░░░░░░]32% ~2h | 7d:[█░░░░░░░░░]8% ~6d");
    }

    // --- Color severity tests -----------------------------------------------

    #[test]
    fn pct_90_is_red_truecolor() {
        let input = make_input(Some(90), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert!(
            result.contains('\x1b'),
            "should have color escape: {result:?}"
        );
        // 90% → (90*10+50)/100 = 9 filled → [█████████░]
        assert_eq!(strip_ansi(&result), "5h:[█████████░]90%");
    }

    #[test]
    fn pct_90_is_red_color16() {
        let input = make_input(Some(90), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert!(result.contains("\x1b[31m"), "should be red: {result:?}");
        assert_eq!(strip_ansi(&result), "5h:[█████████░]90%");
    }

    #[test]
    fn pct_70_is_yellow_color16() {
        let input = make_input(Some(70), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert!(result.contains("\x1b[33m"), "should be yellow: {result:?}");
        // 70% → (70*10+50)/100 = 7 filled → [███████░░░]
        assert_eq!(strip_ansi(&result), "5h:[███████░░░]70%");
    }

    #[test]
    fn pct_50_is_green_color16() {
        let input = make_input(Some(50), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert!(result.contains("\x1b[32m"), "should be green: {result:?}");
        // 50% → (50*10+50)/100 = 5 filled → [█████░░░░░]
        assert_eq!(strip_ansi(&result), "5h:[█████░░░░░]50%");
    }

    // --- ColorLevel::Mono suppresses all ANSI -------------------------------

    #[test]
    fn mono_suppresses_ansi() {
        let reset_ms = BASE_NOW + 60 * 60 * 1000;
        let input = make_input(Some(90), Some(reset_ms), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert!(
            !result.contains('\x1b'),
            "Mono must have no ANSI: {result:?}"
        );
    }

    // --- Reset in past -> no countdown -------------------------------------

    #[test]
    fn reset_in_past_omits_countdown() {
        let reset_ms = BASE_NOW - 1000; // 1 second ago
        let input = make_input(Some(32), Some(reset_ms), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert_eq!(result, "5h:[███░░░░░░░]32%");
    }

    // --- pct = 0 still renders ---------------------------------------------

    #[test]
    fn pct_zero_still_renders() {
        let input = make_input(Some(0), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        // 0% → 0 filled → [░░░░░░░░░░]
        assert_eq!(result, "5h:[░░░░░░░░░░]0%");
    }

    // --- pct = 100 -> red + "100%" -----------------------------------------

    #[test]
    fn pct_100_is_red_shows_100_percent() {
        let input = make_input(Some(100), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert!(result.contains("\x1b[31m"), "should be red: {result:?}");
        // 100% → 10 filled → [██████████]
        assert_eq!(strip_ansi(&result), "5h:[██████████]100%");
    }

    // --- Reset exactly at now -> no countdown (remaining = 0) --------------

    #[test]
    fn reset_exactly_at_now_omits_countdown() {
        let input = make_input(Some(32), Some(BASE_NOW), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert_eq!(result, "5h:[███░░░░░░░]32%");
    }

    // --- Reset in < 1 min remaining -> ceiling ~1m -------------------------

    #[test]
    fn reset_in_89_secs_gives_1m_ceiling() {
        let reset_ms = BASE_NOW + 45 * 1000; // 45 seconds
        let input = make_input(Some(32), Some(reset_ms), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert_eq!(result, "5h:[███░░░░░░░]32% ~1m");
    }

    // --- 25h -> ~2d (ceiling) -----------------------------------------------

    #[test]
    fn reset_in_25h_gives_2d() {
        let reset_ms = BASE_NOW + 25 * 60 * 60 * 1000;
        let input = make_input(Some(32), Some(reset_ms), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert_eq!(result, "5h:[███░░░░░░░]32% ~2d");
    }

    // --- Only 7d, no 5h (smoke test for label order) -----------------------

    #[test]
    fn only_7d_no_reset() {
        let input = make_input(None, None, Some(15), None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        // 15% → (15*10+50)/100 = 2 filled → [██░░░░░░░░]
        assert_eq!(result, "7d:[██░░░░░░░░]15%");
    }

    // --- Exact colored string for Color16, both present --------------------

    #[test]
    fn exact_colored_both_color16() {
        let reset_5h = BASE_NOW + 2 * 60 * 60 * 1000;
        let reset_7d = BASE_NOW + 6 * 24 * 60 * 60 * 1000;
        let input = make_input(Some(32), Some(reset_5h), Some(8), Some(reset_7d));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        // 32% -> green, 8% -> green
        assert!(result.contains("\x1b[32m"), "should have green: {result:?}");
        assert_eq!(
            strip_ansi(&result),
            "5h:[███░░░░░░░]32% ~2h | 7d:[█░░░░░░░░░]8% ~6d"
        );
    }

    // --- format_countdown unit tests ----------------------------------------

    #[test]
    fn countdown_30m_exact() {
        assert_eq!(format_countdown(30 * 60 * 1000), Some("~30m".to_string()));
    }

    #[test]
    fn countdown_2h_exact() {
        assert_eq!(
            format_countdown(2 * 60 * 60 * 1000),
            Some("~2h".to_string())
        );
    }

    #[test]
    fn countdown_6d_exact() {
        assert_eq!(
            format_countdown(6 * 24 * 60 * 60 * 1000),
            Some("~6d".to_string())
        );
    }

    #[test]
    fn countdown_zero_returns_none() {
        assert_eq!(format_countdown(0), None);
    }

    #[test]
    fn countdown_1ms_gives_1m() {
        assert_eq!(format_countdown(1), Some("~1m".to_string()));
    }

    // --- render_bar unit tests ----------------------------------------------

    #[test]
    fn bar_0pct() {
        assert_eq!(render_bar(0, 10), "[░░░░░░░░░░]");
    }

    #[test]
    fn bar_100pct() {
        assert_eq!(render_bar(100, 10), "[██████████]");
    }

    #[test]
    fn bar_50pct() {
        assert_eq!(render_bar(50, 10), "[█████░░░░░]");
    }

    #[test]
    fn bar_32pct() {
        // (32*10+50)/100 = 3
        assert_eq!(render_bar(32, 10), "[███░░░░░░░]");
    }
}
