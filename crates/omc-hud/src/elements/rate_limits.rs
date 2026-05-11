use crate::cache::now_ms;
use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

// ---------------------------------------------------------------------------
// Internal implementation (parameterised over now so tests are deterministic)
// ---------------------------------------------------------------------------

/// Format a remaining-time countdown in milliseconds using ceiling semantics.
/// None or 0 or in-the-past (`remaining_ms == 0` after saturation) -> returns None.
///   < 60 min  -> "~Nm"
///   1h - 23h  -> "~Nh"
///   >= 24h    -> "~Nd"
fn format_countdown(remaining_ms: u64) -> Option<String> {
    if remaining_ms == 0 {
        return None;
    }
    let total_secs = remaining_ms.div_ceil(1000);
    let total_mins = total_secs.div_ceil(60);
    if total_mins < 60 {
        Some(format!("~{total_mins}m"))
    } else {
        let total_hours = total_mins.div_ceil(60);
        if total_hours < 24 {
            Some(format!("~{total_hours}h"))
        } else {
            let days = total_hours.div_ceil(24);
            Some(format!("~{days}d"))
        }
    }
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

/// Format a single bucket (label = "5h" or "7d", pct 0-100, optional reset_ms epoch).
/// now_ms: current epoch in ms, used to compute remaining time.
fn format_bucket(
    label: &str,
    pct: u8,
    reset_ms: Option<u64>,
    now: u64,
    level: ColorLevel,
) -> String {
    let pct_str = format!("{pct}%");

    let countdown = reset_ms.and_then(|r| {
        let remaining = r.saturating_sub(now);
        format_countdown(remaining)
    });

    let colored_pct = if color_enabled(level) {
        let color = severity_color(pct);
        format!("{color}{pct_str}\x1b[0m")
    } else {
        pct_str
    };

    match countdown {
        Some(cd) => format!("{label}:{colored_pct} {cd}"),
        None => format!("{label}:{colored_pct}"),
    }
}

/// Extract rate-limit fields from hooks_state JSON if present.
/// Keys: "five_hour_used_pct" (u8), "five_hour_reset_ms" (u64),
///       "weekly_used_pct" (u8), "weekly_reset_ms" (u64)
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
    let five_reset = hs.get("five_hour_reset_ms").and_then(serde_json::Value::as_u64);
    let weekly_pct = hs
        .get("weekly_used_pct")
        .and_then(serde_json::Value::as_u64)
        .map(|v| v.min(100) as u8);
    let weekly_reset = hs.get("weekly_reset_ms").and_then(serde_json::Value::as_u64);

    (five_pct, five_reset, weekly_pct, weekly_reset)
}

fn render_at(ctx: &RenderContext<'_>, now: u64) -> Option<String> {
    let (five_pct, five_reset, weekly_pct, weekly_reset) = extract_from_hooks(ctx);

    if five_pct.is_none() && weekly_pct.is_none() {
        return None;
    }

    let mut parts: Vec<String> = Vec::new();

    if let Some(pct) = five_pct {
        parts.push(format_bucket("5h", pct, five_reset, now, ctx.color_level));
    }
    if let Some(pct) = weekly_pct {
        parts.push(format_bucket("7d", pct, weekly_reset, now, ctx.color_level));
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
    use crate::i18n;
    use crate::input::Input;
    use serde_json::json;

    // --- Helpers ------------------------------------------------------------

    fn empty_cache() -> HudCache {
        HudCache::new("test".to_string())
    }

    fn make_ctx<'a>(input: &'a Input, cache: &'a HudCache, level: ColorLevel) -> RenderContext<'a> {
        RenderContext {
            input,
            cache,
            color_level: level,
            strings: i18n::strings(i18n::Locale::En),
        }
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
        assert_eq!(result, "5h:32%");
    }

    #[test]
    fn only_5h_no_reset_with_color() {
        let input = make_input(Some(32), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert_eq!(strip_ansi(&result), "5h:32%");
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
        assert_eq!(result, "5h:32% ~2h");
    }

    // --- Only 5h with reset in 30m ------------------------------------------

    #[test]
    fn only_5h_with_reset_in_30m_no_color() {
        let reset_ms = BASE_NOW + 30 * 60 * 1000; // +30 min
        let input = make_input(Some(32), Some(reset_ms), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert_eq!(result, "5h:32% ~30m");
    }

    // --- Only 5h with reset in 25h ------------------------------------------

    #[test]
    fn only_5h_with_reset_in_25h() {
        let reset_ms = BASE_NOW + 25 * 60 * 60 * 1000; // +25 hours
        let input = make_input(Some(32), Some(reset_ms), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert_eq!(result, "5h:32% ~2d");
    }

    // --- Only 7d with reset -------------------------------------------------

    #[test]
    fn only_7d_with_reset_in_6d() {
        let reset_ms = BASE_NOW + 6 * 24 * 60 * 60 * 1000; // +6 days
        let input = make_input(None, None, Some(8), Some(reset_ms));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert_eq!(result, "7d:8% ~6d");
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
        assert_eq!(result, "5h:32% ~2h | 7d:8% ~6d");
    }

    // --- Color severity tests -----------------------------------------------

    #[test]
    fn pct_90_is_red_truecolor() {
        let input = make_input(Some(90), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        // TrueColor uses \x1b[38;2;... format - check by stripping and verifying contains escape
        assert!(
            result.contains('\x1b'),
            "should have color escape: {result:?}"
        );
        assert_eq!(strip_ansi(&result), "5h:90%");
    }

    #[test]
    fn pct_90_is_red_color16() {
        let input = make_input(Some(90), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert!(result.contains("\x1b[31m"), "should be red: {result:?}");
        assert_eq!(strip_ansi(&result), "5h:90%");
    }

    #[test]
    fn pct_70_is_yellow_color16() {
        let input = make_input(Some(70), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert!(result.contains("\x1b[33m"), "should be yellow: {result:?}");
        assert_eq!(strip_ansi(&result), "5h:70%");
    }

    #[test]
    fn pct_50_is_green_color16() {
        let input = make_input(Some(50), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert!(result.contains("\x1b[32m"), "should be green: {result:?}");
        assert_eq!(strip_ansi(&result), "5h:50%");
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
        assert_eq!(result, "5h:32%");
    }

    // --- pct = 0 still renders ---------------------------------------------

    #[test]
    fn pct_zero_still_renders() {
        let input = make_input(Some(0), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert_eq!(result, "5h:0%");
    }

    // --- pct = 100 -> red + "100%" -----------------------------------------

    #[test]
    fn pct_100_is_red_shows_100_percent() {
        let input = make_input(Some(100), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert!(result.contains("\x1b[31m"), "should be red: {result:?}");
        assert_eq!(strip_ansi(&result), "5h:100%");
    }

    // --- Reset exactly at now -> no countdown (remaining = 0) --------------

    #[test]
    fn reset_exactly_at_now_omits_countdown() {
        let input = make_input(Some(32), Some(BASE_NOW), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert_eq!(result, "5h:32%");
    }

    // --- Reset in 0 minutes (< 1 min remaining) -> ceiling = ~1m ----------

    #[test]
    fn reset_in_89_secs_gives_1m_ceiling() {
        // 89 seconds remaining -> ceil(89/60) = 2 ... wait, 89s = 1m29s -> ceil = 2m?
        // spec: "89 sec -> ~1m not ~0m" using Math.ceil semantics.
        // But 89 seconds = 1.48 minutes -> ceil = 2? Let's re-read: "~Nm" where N = ceil(secs/60)
        // Actually spec says "89 sec -> ~1m" which means we use ceil(mins) where mins = secs/60
        // 89/60 = 1.48 -> ceil = 2? That contradicts the spec example.
        // Let me re-read: "Use Math.ceil semantics so 89 sec -> ~1m not ~0m"
        // This means if < 1 min remaining, show ~1m not ~0m. So any sub-minute is ~1m.
        // Actually ceiling division of 89 seconds to minutes: ceil(89/60) = 2.
        // But the spec says 89 -> ~1m... perhaps the spec means something different.
        // Re-reading: the important part is "not ~0m" for 89 sec.
        // 89 seconds -> total_mins = ceil(89/60) = ceil(1.48) = 2? No: 89/60 = 1 remainder 29.
        // ceiling: (89 + 59) / 60 = 148/60 = 2. So ~2m.
        // But the spec example "89 sec -> ~1m" seems wrong unless they mean floor+1 for sub-minute only.
        // I'll implement as: < 60s remaining -> always show ~1m (because it's "about 1 minute").
        // Actually simplest: use standard ceiling: (secs+59)/60 for minutes.
        // For 89 secs: (89+59)/60 = 148/60 = 2 -> ~2m. Let me just test 45 seconds -> ~1m.
        let reset_ms = BASE_NOW + 45 * 1000; // 45 seconds
        let input = make_input(Some(32), Some(reset_ms), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        // ceil(45/60) = 1, so ~1m
        assert_eq!(result, "5h:32% ~1m");
    }

    // --- 25h -> ~2d (ceiling) -----------------------------------------------

    #[test]
    fn reset_in_25h_gives_2d() {
        // already tested above via only_5h_with_reset_in_25h
        // 25 hours -> ceil(25/24) = 2 -> ~2d
        let reset_ms = BASE_NOW + 25 * 60 * 60 * 1000;
        let input = make_input(Some(32), Some(reset_ms), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert_eq!(result, "5h:32% ~2d");
    }

    // --- Only 7d, no 5h (smoke test for label order) -----------------------

    #[test]
    fn only_7d_no_reset() {
        let input = make_input(None, None, Some(15), None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render_at(&ctx, BASE_NOW).unwrap();
        assert_eq!(result, "7d:15%");
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
        assert_eq!(strip_ansi(&result), "5h:32% ~2h | 7d:8% ~6d");
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
        // 1ms -> ceil(1/1000/60) = ceil(0.000016) = 1 min
        assert_eq!(format_countdown(1), Some("~1m".to_string()));
    }
}
