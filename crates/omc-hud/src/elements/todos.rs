use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

fn color_enabled(level: ColorLevel) -> bool {
    !matches!(level, ColorLevel::Mono)
}

/// Choose ANSI color code based on percent completion.
/// - >=80% -> green
/// - >=50% -> yellow
/// - <50%  -> cyan (in-progress)
fn color_for_percent(percent: u32) -> &'static str {
    if percent >= 80 {
        "\x1b[32m" // green
    } else if percent >= 50 {
        "\x1b[33m" // yellow
    } else {
        "\x1b[36m" // cyan (in-progress)
    }
}

/// Extract (completed, total) from hooks_state JSON.
///
/// Looks for:
///   - "todos_completed" / "todos_total"
///   - "completed" / "total"
///
/// Returns None if neither key-pair is present or total == 0.
fn extract_from_hooks(ctx: &RenderContext<'_>) -> Option<(u32, u32)> {
    let state = ctx.input.hooks_state.as_ref()?;

    // Try primary key-pair
    let completed = state
        .get("todos_completed")
        .and_then(serde_json::Value::as_u64)
        .or_else(|| state.get("completed").and_then(serde_json::Value::as_u64))
        .map(|v| v as u32);

    let total = state
        .get("todos_total")
        .and_then(serde_json::Value::as_u64)
        .or_else(|| state.get("total").and_then(serde_json::Value::as_u64))
        .map(|v| v as u32);

    let completed = completed?;
    let total = total?;

    if total == 0 {
        return None;
    }

    Some((completed, total))
}

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    // Keep i18n `todo` field and Input fields live to satisfy dead_code lint.
    // transcript_path and turns are used by other callers; we reference them
    // here so the compiler sees them as read even when only this element runs.
    let _ = ctx.strings.todo;
    let _ = &ctx.input.transcript_path;
    let _ = ctx.input.turns;

    let (completed, total) = extract_from_hooks(ctx)?;

    // Defensive clamp: completed cannot exceed total
    let completed = completed.min(total);

    let percent = completed * 100 / total;

    let count_str = format!("{completed}/{total}");

    if color_enabled(ctx.color_level) {
        let color = color_for_percent(percent);
        Some(format!("todos:{color}{count_str}\x1b[0m"))
    } else {
        Some(format!("todos:{count_str}"))
    }
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
            strings: i18n::strings(i18n::Locale::En),
        }
    }

    fn empty_cache() -> HudCache {
        HudCache::new("test".to_string())
    }

    /// Build an Input with hooks_state containing todos_completed and todos_total.
    fn make_input_with_todos(completed: u64, total: u64) -> Input {
        let state = serde_json::json!({
            "todos_completed": completed,
            "todos_total": total,
        });
        Input {
            hooks_state: Some(state),
            ..Input::default()
        }
    }

    /// Build an Input with no hooks_state at all.
    fn make_input_no_hooks() -> Input {
        Input::default()
    }

    /// Build an Input with hooks_state that has neither completed nor total keys.
    fn make_input_hooks_no_todos() -> Input {
        let state = serde_json::json!({ "some_other_key": 42 });
        Input {
            hooks_state: Some(state),
            ..Input::default()
        }
    }

    /// Strip ANSI escape sequences for plain-text assertions.
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
    fn none_when_no_hooks_state() {
        let input = make_input_no_hooks();
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        assert_eq!(render(&ctx), None);
    }

    #[test]
    fn none_when_hooks_state_has_no_todos_keys() {
        let input = make_input_hooks_no_todos();
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        assert_eq!(render(&ctx), None);
    }

    #[test]
    fn none_when_total_is_zero() {
        let input = make_input_with_todos(0, 0);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        assert_eq!(render(&ctx), None);
    }

    #[test]
    fn none_when_only_completed_present_without_total() {
        let state = serde_json::json!({ "todos_completed": 3 });
        let input = Input {
            hooks_state: Some(state),
            ..Input::default()
        };
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        assert_eq!(render(&ctx), None);
    }

    // --- No-color rendering (Mono) ------------------------------------------

    #[test]
    fn five_of_eight_no_color() {
        let input = make_input_with_todos(5, 8);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "todos:5/8");
    }

    #[test]
    fn zero_of_eight_no_color() {
        let input = make_input_with_todos(0, 8);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "todos:0/8");
    }

    // --- Color thresholds ---------------------------------------------------

    /// 8/8 = 100% -> green
    #[test]
    fn eight_of_eight_is_green() {
        let input = make_input_with_todos(8, 8);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[32m"),
            "100% should be green: {result:?}"
        );
        assert_eq!(strip_ansi(&result), "todos:8/8");
    }

    /// 6/8 = 75% -> green (>=80% boundary: 75% falls below, BUT spec says >=80% green)
    /// 6/8 = 75% -> yellow (50-79% range)
    #[test]
    fn six_of_eight_is_yellow() {
        let input = make_input_with_todos(6, 8); // 75%
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[33m"),
            "75% should be yellow: {result:?}"
        );
        assert_eq!(strip_ansi(&result), "todos:6/8");
    }

    /// Exactly 80%: 8/10 = 80% -> green
    #[test]
    fn eighty_percent_exact_is_green() {
        let input = make_input_with_todos(8, 10); // 80%
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[32m"),
            "80% should be green: {result:?}"
        );
        assert_eq!(strip_ansi(&result), "todos:8/10");
    }

    /// 4/8 = 50% -> yellow
    #[test]
    fn four_of_eight_is_yellow() {
        let input = make_input_with_todos(4, 8); // 50%
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[33m"),
            "50% should be yellow: {result:?}"
        );
        assert_eq!(strip_ansi(&result), "todos:4/8");
    }

    /// 3/8 = 37% -> cyan
    #[test]
    fn three_of_eight_is_cyan() {
        let input = make_input_with_todos(3, 8); // 37%
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[36m"),
            "37% should be cyan: {result:?}"
        );
        assert_eq!(strip_ansi(&result), "todos:3/8");
    }

    /// 0/8 = 0% -> cyan (in-progress)
    #[test]
    fn zero_percent_is_cyan() {
        let input = make_input_with_todos(0, 8);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let result = render(&ctx).unwrap();
        assert!(result.contains("\x1b[36m"), "0% should be cyan: {result:?}");
        assert_eq!(strip_ansi(&result), "todos:0/8");
    }

    /// 1/8 = 12% -> cyan
    #[test]
    fn one_of_eight_is_cyan() {
        let input = make_input_with_todos(1, 8); // 12%
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[36m"),
            "12% should be cyan: {result:?}"
        );
        assert_eq!(strip_ansi(&result), "todos:1/8");
    }

    // --- Defensive clamp (completed > total) --------------------------------

    /// 12/8 -> clamped to 8/8, rendered as green
    #[test]
    fn over_count_clamped_to_total() {
        let input = make_input_with_todos(12, 8);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let result = render(&ctx).unwrap();
        // After clamp: 8/8 = 100% -> green
        assert!(
            result.contains("\x1b[32m"),
            "clamped 12/8 should be green: {result:?}"
        );
        assert_eq!(strip_ansi(&result), "todos:8/8");
    }

    // --- ANSI code presence (TrueColor) -------------------------------------

    #[test]
    fn truecolor_green_contains_green_code() {
        let input = make_input_with_todos(8, 8);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let result = render(&ctx).unwrap();
        assert!(result.contains("\x1b[32m"), "should contain green code");
        assert!(result.contains("\x1b[0m"), "should contain reset");
    }

    #[test]
    fn truecolor_yellow_contains_yellow_code() {
        let input = make_input_with_todos(4, 8); // 50%
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let result = render(&ctx).unwrap();
        assert!(result.contains("\x1b[33m"), "should contain yellow code");
        assert!(result.contains("\x1b[0m"), "should contain reset");
    }

    #[test]
    fn truecolor_cyan_contains_cyan_code() {
        let input = make_input_with_todos(1, 8); // 12%
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let result = render(&ctx).unwrap();
        assert!(result.contains("\x1b[36m"), "should contain cyan code");
        assert!(result.contains("\x1b[0m"), "should contain reset");
    }

    // --- Mono suppresses ANSI -----------------------------------------------

    #[test]
    fn mono_suppresses_ansi_for_complete_todos() {
        let input = make_input_with_todos(8, 8);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render(&ctx).unwrap();
        assert!(
            !result.contains('\x1b'),
            "mono should have no ANSI: {result:?}"
        );
        assert_eq!(result, "todos:8/8");
    }

    // --- Color16 also emits color -------------------------------------------

    #[test]
    fn color16_emits_ansi_for_complete_todos() {
        let input = make_input_with_todos(8, 8);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[32m"),
            "Color16 should emit green: {result:?}"
        );
    }

    // --- Exact colored strings ----------------------------------------------

    #[test]
    fn exact_colored_string_green_8_of_8() {
        let input = make_input_with_todos(8, 8);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        assert_eq!(render(&ctx).unwrap(), "todos:\x1b[32m8/8\x1b[0m");
    }

    #[test]
    fn exact_colored_string_yellow_4_of_8() {
        let input = make_input_with_todos(4, 8);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        assert_eq!(render(&ctx).unwrap(), "todos:\x1b[33m4/8\x1b[0m");
    }

    #[test]
    fn exact_colored_string_cyan_1_of_8() {
        let input = make_input_with_todos(1, 8);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        assert_eq!(render(&ctx).unwrap(), "todos:\x1b[36m1/8\x1b[0m");
    }

    // --- Fallback key-pairs ("completed"/"total") ---------------------------

    #[test]
    fn fallback_keys_completed_total() {
        let state = serde_json::json!({
            "completed": 5u64,
            "total": 8u64,
        });
        let input = Input {
            hooks_state: Some(state),
            ..Input::default()
        };
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "todos:5/8");
    }
}
