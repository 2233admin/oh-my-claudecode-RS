use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

// ---------------------------------------------------------------------------
// Data extraction
// ---------------------------------------------------------------------------

struct AutopilotState {
    mode: String,
    iteration: Option<u32>,
    max_iterations: Option<u32>,
    worker_count: Option<u32>,
}

fn extract_state(ctx: &RenderContext<'_>) -> Option<AutopilotState> {
    let state = ctx.input.hooks_state.as_ref()?;

    let mode = state
        .get("mode")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())?;

    let iteration = state
        .get("iteration")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    let max_iterations = state
        .get("max_iterations")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    let worker_count = state
        .get("worker_count")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    Some(AutopilotState {
        mode,
        iteration,
        max_iterations,
        worker_count,
    })
}

// ---------------------------------------------------------------------------
// Label formatting
// ---------------------------------------------------------------------------

fn format_label(state: &AutopilotState, autopilot_str: &str) -> Option<String> {
    match state.mode.as_str() {
        "ralph" => Some(format_iter_label("ralph", state)),
        "ultrawork" => Some(format_iter_label("ulw", state)),
        "autopilot" => Some(autopilot_str.to_string()),
        "team" => Some(format_team_label(state)),
        _ => None,
    }
}

fn format_iter_label(prefix: &str, state: &AutopilotState) -> String {
    match (state.iteration, state.max_iterations) {
        (Some(iter), Some(max)) => format!("{prefix}:{iter}/{max}"),
        (Some(iter), None) => format!("{prefix}:{iter}"),
        _ => prefix.to_string(),
    }
}

fn format_team_label(state: &AutopilotState) -> String {
    match state.worker_count {
        Some(n) if n > 0 => format!("team:{n} workers"),
        _ => "team".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Color selection
// ---------------------------------------------------------------------------

fn color_enabled(level: ColorLevel) -> bool {
    !matches!(level, ColorLevel::Mono)
}

/// ANSI escape for iteration progress severity.
/// - >=90% of max -> red
/// - 70-89%       -> yellow
/// - else         -> cyan
fn iter_color(iteration: Option<u32>, max_iterations: Option<u32>) -> &'static str {
    match (iteration, max_iterations) {
        (Some(iter), Some(max)) if max > 0 => {
            let pct = iter * 100 / max;
            if pct >= 90 {
                "\x1b[31m" // red
            } else if pct >= 70 {
                "\x1b[33m" // yellow
            } else {
                "\x1b[36m" // cyan
            }
        }
        _ => "\x1b[36m", // cyan (no progress info)
    }
}

fn mode_color(state: &AutopilotState) -> &'static str {
    match state.mode.as_str() {
        "ralph" | "ultrawork" => iter_color(state.iteration, state.max_iterations),
        "autopilot" => "\x1b[35m", // magenta
        "team" => "\x1b[96m",      // bright cyan
        _ => "\x1b[36m",
    }
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let state = extract_state(ctx)?;
    let label = format_label(&state, ctx.strings.autopilot)?;

    if color_enabled(ctx.color_level) {
        let color = mode_color(&state);
        Some(format!("{color}{label}\x1b[0m"))
    } else {
        Some(label)
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

    // --- Helpers ------------------------------------------------------------

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

    /// Build an Input with hooks_state containing the given orchestration fields.
    fn make_input(
        mode: Option<&str>,
        iteration: Option<u32>,
        max_iterations: Option<u32>,
        worker_count: Option<u32>,
    ) -> Input {
        let mut map = serde_json::Map::new();
        if let Some(m) = mode {
            map.insert("mode".to_string(), serde_json::json!(m));
        }
        if let Some(i) = iteration {
            map.insert("iteration".to_string(), serde_json::json!(i));
        }
        if let Some(m) = max_iterations {
            map.insert("max_iterations".to_string(), serde_json::json!(m));
        }
        if let Some(w) = worker_count {
            map.insert("worker_count".to_string(), serde_json::json!(w));
        }
        Input {
            hooks_state: Some(serde_json::Value::Object(map)),
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
    fn none_when_hooks_state_is_none() {
        let input = Input::default();
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        assert_eq!(render(&ctx), None);
    }

    #[test]
    fn none_when_mode_key_absent() {
        let input = make_input(None, Some(3), Some(10), None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        assert_eq!(render(&ctx), None);
    }

    #[test]
    fn none_when_mode_is_empty_string() {
        let input = make_input(Some(""), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        assert_eq!(render(&ctx), None);
    }

    #[test]
    fn none_when_mode_is_unknown() {
        let input = make_input(Some("chaos"), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        assert_eq!(render(&ctx), None);
    }

    // --- Ralph mode ---------------------------------------------------------

    #[test]
    fn ralph_iter_3_max_10_is_cyan_30_pct() {
        let input = make_input(Some("ralph"), Some(3), Some(10), None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert_eq!(strip_ansi(&result), "ralph:3/10");
        assert!(
            result.contains("\x1b[36m"),
            "30%: should be cyan: {result:?}"
        );
    }

    #[test]
    fn ralph_iter_8_max_10_is_yellow_80_pct() {
        let input = make_input(Some("ralph"), Some(8), Some(10), None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert_eq!(strip_ansi(&result), "ralph:8/10");
        assert!(
            result.contains("\x1b[33m"),
            "80%: should be yellow: {result:?}"
        );
    }

    #[test]
    fn ralph_iter_10_max_10_is_red_100_pct() {
        let input = make_input(Some("ralph"), Some(10), Some(10), None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert_eq!(strip_ansi(&result), "ralph:10/10");
        assert!(
            result.contains("\x1b[31m"),
            "100%: should be red: {result:?}"
        );
    }

    #[test]
    fn ralph_iter_only_no_max() {
        let input = make_input(Some("ralph"), Some(5), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "ralph:5");
    }

    #[test]
    fn ralph_no_iter_no_max_bare_label() {
        let input = make_input(Some("ralph"), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "ralph");
    }

    // --- Ultrawork mode -----------------------------------------------------

    #[test]
    fn ultrawork_iter_2_max_5_starts_with_ulw() {
        let input = make_input(Some("ultrawork"), Some(2), Some(5), None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render(&ctx).unwrap();
        assert_eq!(result, "ulw:2/5");
    }

    #[test]
    fn ultrawork_no_iter_bare_label() {
        let input = make_input(Some("ultrawork"), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "ulw");
    }

    // --- Autopilot mode -----------------------------------------------------

    #[test]
    fn autopilot_is_exact_string() {
        let input = make_input(Some("autopilot"), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "autopilot");
    }

    #[test]
    fn autopilot_colored_is_magenta() {
        let input = make_input(Some("autopilot"), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert_eq!(strip_ansi(&result), "autopilot");
        assert!(
            result.contains("\x1b[35m"),
            "autopilot: should be magenta: {result:?}"
        );
    }

    // --- Team mode ----------------------------------------------------------

    #[test]
    fn team_with_worker_count_3() {
        let input = make_input(Some("team"), None, None, Some(3));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "team:3 workers");
    }

    #[test]
    fn team_with_worker_count_0_bare_label() {
        // worker_count=0 means no active workers; render bare "team"
        let input = make_input(Some("team"), None, None, Some(0));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "team");
    }

    #[test]
    fn team_without_worker_count_bare_label() {
        let input = make_input(Some("team"), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "team");
    }

    #[test]
    fn team_colored_is_bright_cyan() {
        let input = make_input(Some("team"), None, None, Some(3));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert_eq!(strip_ansi(&result), "team:3 workers");
        assert!(
            result.contains("\x1b[96m"),
            "team: should be bright cyan: {result:?}"
        );
    }

    // --- Color level gating -------------------------------------------------

    #[test]
    fn truecolor_includes_ansi_escapes_ralph() {
        let input = make_input(Some("ralph"), Some(3), Some(10), None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains('\x1b'),
            "TrueColor should have ANSI: {result:?}"
        );
        assert!(result.contains("\x1b[0m"), "should have reset: {result:?}");
    }

    #[test]
    fn truecolor_includes_ansi_escapes_autopilot() {
        let input = make_input(Some("autopilot"), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains('\x1b'),
            "TrueColor should have ANSI: {result:?}"
        );
    }

    #[test]
    fn mono_produces_no_ansi_ralph() {
        let input = make_input(Some("ralph"), Some(3), Some(10), None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render(&ctx).unwrap();
        assert!(!result.contains('\x1b'), "Mono: no ANSI: {result:?}");
        assert_eq!(result, "ralph:3/10");
    }

    #[test]
    fn mono_produces_no_ansi_autopilot() {
        let input = make_input(Some("autopilot"), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render(&ctx).unwrap();
        assert!(!result.contains('\x1b'), "Mono: no ANSI: {result:?}");
    }

    #[test]
    fn mono_produces_no_ansi_team() {
        let input = make_input(Some("team"), None, None, Some(3));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render(&ctx).unwrap();
        assert!(!result.contains('\x1b'), "Mono: no ANSI: {result:?}");
    }

    // --- Exact colored strings ----------------------------------------------

    #[test]
    fn exact_colored_ralph_3_10_cyan() {
        let input = make_input(Some("ralph"), Some(3), Some(10), None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        assert_eq!(render(&ctx).unwrap(), "\x1b[36mralph:3/10\x1b[0m");
    }

    #[test]
    fn exact_colored_autopilot_magenta() {
        let input = make_input(Some("autopilot"), None, None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        assert_eq!(render(&ctx).unwrap(), "\x1b[35mautopilot\x1b[0m");
    }

    #[test]
    fn exact_colored_team_3_bright_cyan() {
        let input = make_input(Some("team"), None, None, Some(3));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        assert_eq!(render(&ctx).unwrap(), "\x1b[96mteam:3 workers\x1b[0m");
    }

    // --- Boundary: 70% threshold is yellow, 69% is cyan --------------------

    #[test]
    fn ralph_70_pct_is_yellow() {
        // 7/10 = 70%
        let input = make_input(Some("ralph"), Some(7), Some(10), None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[33m"),
            "70%: should be yellow: {result:?}"
        );
    }

    #[test]
    fn ralph_69_pct_is_cyan() {
        // 69/100 = 69%
        let input = make_input(Some("ralph"), Some(69), Some(100), None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[36m"),
            "69%: should be cyan: {result:?}"
        );
    }

    #[test]
    fn ralph_90_pct_is_red() {
        // 9/10 = 90%
        let input = make_input(Some("ralph"), Some(9), Some(10), None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[31m"),
            "90%: should be red: {result:?}"
        );
    }
}
