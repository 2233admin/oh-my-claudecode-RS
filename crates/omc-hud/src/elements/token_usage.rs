use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

// ---------------------------------------------------------------------------
// Token count formatter
// ---------------------------------------------------------------------------

/// Format a raw token count as a compact string.
///   < 1_000           -> "42"
///   1_000..999_999    -> "4.2K" (one decimal if < 10K) or "12K" (integer K)
///   >= 1_000_000      -> "1.5M" (one decimal)
fn compact(n: u64) -> String {
    if n < 1_000 {
        return n.to_string();
    }
    if n < 1_000_000 {
        let k = n as f64 / 1_000.0;
        if k < 10.0 {
            return format!("{:.1}K", k);
        }
        return format!("{}K", k.round() as u64);
    }
    let m = n as f64 / 1_000_000.0;
    format!("{:.1}M", m)
}

// ---------------------------------------------------------------------------
// Data extraction from hooks_state
// ---------------------------------------------------------------------------

struct TokenData {
    input: u64,
    output: u64,
    reasoning: u64,
    session_total: u64,
}

/// Extract token fields from `hooks_state` JSON.
///
/// Expected keys (all optional, default to 0 if absent):
///   "input_tokens"         -- last-request input tokens
///   "output_tokens"        -- last-request output tokens
///   "reasoning_tokens"     -- last-request reasoning tokens
///   "session_total_tokens" -- cumulative session total
///
/// Returns `None` only if hooks_state is absent.
fn extract(ctx: &RenderContext<'_>) -> Option<TokenData> {
    let state = ctx.input.hooks_state.as_ref()?;

    let get_u64 = |key: &str| -> u64 { state.get(key).and_then(|v| v.as_u64()).unwrap_or(0) };

    Some(TokenData {
        input: get_u64("input_tokens"),
        output: get_u64("output_tokens"),
        reasoning: get_u64("reasoning_tokens"),
        session_total: get_u64("session_total_tokens"),
    })
}

// ---------------------------------------------------------------------------
// Color helper
// ---------------------------------------------------------------------------

fn color_enabled(level: ColorLevel) -> bool {
    !matches!(level, ColorLevel::Mono)
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let data = extract(ctx)?;

    // If both input and output are 0, nothing to show.
    if data.input == 0 && data.output == 0 {
        return None;
    }

    let label = ctx.strings.tok;
    let input_str = compact(data.input);
    let output_str = compact(data.output);

    // Optional reasoning suffix
    let reasoning_part = if data.reasoning > 0 {
        format!(" r{}", compact(data.reasoning))
    } else {
        String::new()
    };

    // Optional session suffix
    let session_part = if data.session_total > 0 {
        format!(" s{}", compact(data.session_total))
    } else {
        String::new()
    };

    if color_enabled(ctx.color_level) {
        if data.output > 10_000 {
            // Dim "tok:" prefix + bright-cyan for output count
            Some(format!(
                "\x1b[2m{label}:\x1b[0mi{input_str}/\x1b[96m{output_str}\x1b[0m{reasoning_part}{session_part}"
            ))
        } else {
            // Dim "tok:" prefix, numbers plain
            Some(format!(
                "\x1b[2m{label}:\x1b[0mi{input_str}/o{output_str}{reasoning_part}{session_part}"
            ))
        }
    } else {
        Some(format!(
            "{label}:i{input_str}/o{output_str}{reasoning_part}{session_part}"
        ))
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
            strings: i18n::strings(i18n::detect_locale()),
        }
    }

    fn empty_cache() -> HudCache {
        HudCache::new("test".to_string())
    }

    fn make_input(
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        reasoning_tokens: Option<u64>,
        session_total: Option<u64>,
    ) -> Input {
        let mut map = serde_json::Map::new();
        if let Some(v) = input_tokens {
            map.insert("input_tokens".to_string(), serde_json::json!(v));
        }
        if let Some(v) = output_tokens {
            map.insert("output_tokens".to_string(), serde_json::json!(v));
        }
        if let Some(v) = reasoning_tokens {
            map.insert("reasoning_tokens".to_string(), serde_json::json!(v));
        }
        if let Some(v) = session_total {
            map.insert("session_total_tokens".to_string(), serde_json::json!(v));
        }
        Input {
            hooks_state: Some(serde_json::Value::Object(map)),
            ..Input::default()
        }
    }

    fn make_input_no_hooks() -> Input {
        Input::default()
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

    /// No hooks_state at all -> None
    #[test]
    fn none_when_no_hooks_state() {
        let input = make_input_no_hooks();
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        assert_eq!(render(&ctx), None);
    }

    /// hooks_state present but both input and output are 0 -> None
    #[test]
    fn none_when_both_zero() {
        let input = make_input(Some(0), Some(0), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        assert_eq!(render(&ctx), None);
    }

    /// hooks_state present but neither token key exists -> both default to 0 -> None
    #[test]
    fn none_when_token_keys_absent() {
        let state = serde_json::json!({ "some_other_key": 42 });
        let input = Input {
            hooks_state: Some(state),
            ..Input::default()
        };
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        assert_eq!(render(&ctx), None);
    }

    // --- No-suffix range (< 1000) -------------------------------------------

    /// 500 in, 200 out -> "tok:i500/o200"
    #[test]
    fn small_counts_no_suffix() {
        let input = make_input(Some(500), Some(200), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render(&ctx).unwrap();
        assert_eq!(result, "tok:i500/o200");
    }

    // --- K suffix -----------------------------------------------------------

    /// 12000 in, 4000 out -> "tok:i12K/o4.0K"
    #[test]
    fn k_suffix_basic() {
        let input = make_input(Some(12_000), Some(4_000), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render(&ctx).unwrap();
        // 12000 -> "12K" (>=10K => integer K)
        // 4000  -> "4.0K" (< 10K => one decimal)
        assert_eq!(strip_ansi(&result), "tok:i12K/o4.0K");
    }

    // --- M suffix -----------------------------------------------------------

    /// 1_500_000 in, 500_000 out -> "tok:i1.5M/o500K"
    #[test]
    fn m_suffix_input_k_suffix_output() {
        let input = make_input(Some(1_500_000), Some(500_000), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render(&ctx).unwrap();
        assert_eq!(strip_ansi(&result), "tok:i1.5M/o500K");
    }

    // --- Reasoning token ---------------------------------------------------

    /// reasoning 1500 -> contains " r1.5K"
    #[test]
    fn reasoning_included_when_nonzero() {
        let input = make_input(Some(12_000), Some(4_000), Some(1_500), None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains(" r1.5K"),
            "should contain ' r1.5K': {result:?}"
        );
    }

    /// reasoning 0 -> omitted
    #[test]
    fn reasoning_omitted_when_zero() {
        let input = make_input(Some(12_000), Some(4_000), Some(0), None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render(&ctx).unwrap();
        assert!(
            !result.contains(" r"),
            "reasoning should be omitted: {result:?}"
        );
    }

    // --- Session total ------------------------------------------------------

    /// session_total 120000 -> contains " s120K"
    #[test]
    fn session_total_included_when_nonzero() {
        let input = make_input(Some(12_000), Some(4_000), None, Some(120_000));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains(" s120K"),
            "should contain ' s120K': {result:?}"
        );
    }

    /// session_total 0 -> omitted
    #[test]
    fn session_total_omitted_when_zero() {
        let input = make_input(Some(12_000), Some(4_000), None, Some(0));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render(&ctx).unwrap();
        assert!(
            !result.contains(" s"),
            "session total should be omitted: {result:?}"
        );
    }

    // --- Both reasoning + session -------------------------------------------

    #[test]
    fn both_reasoning_and_session() {
        let input = make_input(Some(12_000), Some(4_000), Some(1_500), Some(120_000));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render(&ctx).unwrap();
        assert!(result.contains(" r1.5K"), "should contain r: {result:?}");
        assert!(result.contains(" s120K"), "should contain s: {result:?}");
    }

    // --- Color gating -------------------------------------------------------

    /// TrueColor: result must contain some ANSI escape
    #[test]
    fn truecolor_contains_ansi() {
        let input = make_input(Some(12_000), Some(4_000), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains('\x1b'),
            "TrueColor should emit ANSI: {result:?}"
        );
    }

    /// Mono: no ANSI escape codes at all
    #[test]
    fn mono_no_ansi() {
        let input = make_input(Some(12_000), Some(4_000), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render(&ctx).unwrap();
        assert!(
            !result.contains('\x1b'),
            "Mono should have no ANSI: {result:?}"
        );
    }

    /// High output (> 10K): contains bright-cyan \x1b[96m
    #[test]
    fn high_output_bright_cyan() {
        let input = make_input(Some(12_000), Some(15_000), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[96m"),
            "output >10K should use bright-cyan: {result:?}"
        );
    }

    /// Normal output (<= 10K): dim prefix present, no bright-cyan
    #[test]
    fn normal_output_no_bright_cyan() {
        let input = make_input(Some(12_000), Some(4_000), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert!(
            !result.contains("\x1b[96m"),
            "output <=10K should not use bright-cyan: {result:?}"
        );
        assert!(
            result.contains("\x1b[2m"),
            "should have dim prefix: {result:?}"
        );
    }

    // --- Boundary: 999 vs 1000 ----------------------------------------------

    /// 999 -> "999" (no suffix)
    #[test]
    fn boundary_999_no_suffix() {
        let input = make_input(Some(999), Some(999), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render(&ctx).unwrap();
        assert_eq!(result, "tok:i999/o999");
    }

    /// 1000 -> "1.0K"
    #[test]
    fn boundary_1000_is_1_0k() {
        let input = make_input(Some(1_000), Some(1_000), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render(&ctx).unwrap();
        assert_eq!(result, "tok:i1.0K/o1.0K");
    }

    // --- compact() unit tests -----------------------------------------------

    /// 9999 / 1000.0 = 9.999 -> one decimal -> "10.0K"
    #[test]
    fn boundary_9999_rounds_to_10_0k() {
        assert_eq!(compact(9_999), "10.0K");
    }

    /// 10000 >= 10K threshold -> integer K -> "10K"
    #[test]
    fn boundary_10000_is_10k() {
        assert_eq!(compact(10_000), "10K");
    }

    #[test]
    fn compact_42() {
        assert_eq!(compact(42), "42");
    }

    #[test]
    fn compact_0() {
        assert_eq!(compact(0), "0");
    }

    #[test]
    fn compact_999() {
        assert_eq!(compact(999), "999");
    }

    #[test]
    fn compact_4200() {
        assert_eq!(compact(4_200), "4.2K");
    }

    #[test]
    fn compact_12000() {
        assert_eq!(compact(12_000), "12K");
    }

    #[test]
    fn compact_500000() {
        assert_eq!(compact(500_000), "500K");
    }

    #[test]
    fn compact_1_500_000() {
        assert_eq!(compact(1_500_000), "1.5M");
    }

    #[test]
    fn compact_120000() {
        assert_eq!(compact(120_000), "120K");
    }
}
