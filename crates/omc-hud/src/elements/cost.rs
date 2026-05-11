use crate::elements::RenderContext;
use crate::terminal::{ColorLevel, SemanticColor, paint};

// ---------------------------------------------------------------------------
// Pricing table (USD per token, baseline 2026-05)
// ---------------------------------------------------------------------------

struct ModelPricing {
    input: f64,
    output: f64,
    cache_write: f64,
    cache_read: f64,
}

fn pricing_for_model(model: &str) -> ModelPricing {
    if model.starts_with("claude-opus-4-7") || model.starts_with("claude-opus-4-6") {
        ModelPricing {
            input: 15e-6,
            output: 75e-6,
            cache_write: 18.75e-6,
            cache_read: 1.5e-6,
        }
    } else if model.starts_with("claude-haiku-4-5") {
        ModelPricing {
            input: 0.8e-6,
            output: 4e-6,
            cache_write: 1e-6,
            cache_read: 0.08e-6,
        }
    } else {
        // sonnet-4-6 and unknown/default
        ModelPricing {
            input: 3e-6,
            output: 15e-6,
            cache_write: 3.75e-6,
            cache_read: 0.3e-6,
        }
    }
}

// ---------------------------------------------------------------------------
// Token extraction from hooks_state
// ---------------------------------------------------------------------------

struct TokenCounts {
    input: u64,
    output: u64,
    cache_creation: u64,
    cache_read: u64,
}

fn extract_tokens(ctx: &RenderContext<'_>) -> Option<TokenCounts> {
    let state = ctx.input.hooks_state.as_ref()?;
    let get_u64 = |key: &str| -> u64 { state.get(key).and_then(serde_json::Value::as_u64).unwrap_or(0) };

    Some(TokenCounts {
        input: get_u64("input_tokens"),
        output: get_u64("output_tokens"),
        cache_creation: get_u64("cache_creation_input_tokens"),
        cache_read: get_u64("cache_read_input_tokens"),
    })
}

// ---------------------------------------------------------------------------
// Cost computation
// ---------------------------------------------------------------------------

fn compute_cost(tokens: &TokenCounts, pricing: &ModelPricing) -> f64 {
    tokens.input as f64 * pricing.input
        + tokens.output as f64 * pricing.output
        + tokens.cache_creation as f64 * pricing.cache_write
        + tokens.cache_read as f64 * pricing.cache_read
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

fn format_cost(cost: f64) -> String {
    if cost < 0.01 {
        let cents = cost * 100.0;
        format!("{:.1}\u{00a2}", cents)
    } else if cost < 100.0 {
        format!("${:.2}", cost)
    } else {
        format!("${}", cost.round() as u64)
    }
}

// ---------------------------------------------------------------------------
// Color
// ---------------------------------------------------------------------------

/// Returns the semantic color tier for a cost, or None for the mid-range
/// ($0.10–$1) where no color is emitted.
fn semantic_color(cost: f64) -> Option<SemanticColor> {
    if cost < 0.10 {
        Some(SemanticColor::Green)
    } else if cost < 1.0 {
        None
    } else if cost < 10.0 {
        Some(SemanticColor::Yellow)
    } else {
        Some(SemanticColor::Red)
    }
}

/// Emit a Color16 ANSI code directly (bypasses `paint` so the code is
/// always a standard 3/4-bit escape, which is what Color16 terminals want).
fn color16_escape(color: SemanticColor) -> &'static str {
    match color {
        SemanticColor::Green => "\x1b[32m",
        SemanticColor::Yellow => "\x1b[33m",
        SemanticColor::Red => "\x1b[31m",
    }
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    // Primary path: compute cost from hooks_state token counts.
    let cost = if let Some(tokens) = extract_tokens(ctx) {
        if tokens.input == 0
            && tokens.output == 0
            && tokens.cache_creation == 0
            && tokens.cache_read == 0
        {
            return None;
        }
        let model = ctx.input.model.as_deref().unwrap_or("");
        compute_cost(&tokens, &pricing_for_model(model))
    } else {
        // Fallback: use the pre-computed cost_usd field if available.
        ctx.input.cost_usd?
    };

    let label = format_cost(cost);

    match ctx.color_level {
        ColorLevel::Mono => Some(label),
        ColorLevel::Color16 => {
            if let Some(color) = semantic_color(cost) {
                Some(format!("{}{label}\x1b[0m", color16_escape(color)))
            } else {
                Some(label)
            }
        }
        ColorLevel::Color256 | ColorLevel::TrueColor => {
            if let Some(color) = semantic_color(cost) {
                Some(paint(ctx.color_level, color, &label))
            } else {
                Some(label)
            }
        }
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

    /// Build an Input with model + hooks_state containing chosen token counts.
    fn make_input(
        model: Option<&str>,
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        cache_creation: Option<u64>,
        cache_read_tokens: Option<u64>,
    ) -> Input {
        let mut map = serde_json::Map::new();
        if let Some(v) = input_tokens {
            map.insert("input_tokens".to_string(), serde_json::json!(v));
        }
        if let Some(v) = output_tokens {
            map.insert("output_tokens".to_string(), serde_json::json!(v));
        }
        if let Some(v) = cache_creation {
            map.insert(
                "cache_creation_input_tokens".to_string(),
                serde_json::json!(v),
            );
        }
        if let Some(v) = cache_read_tokens {
            map.insert("cache_read_input_tokens".to_string(), serde_json::json!(v));
        }
        Input {
            model: model.map(std::string::ToString::to_string),
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

    /// No hooks_state and no cost_usd -> None
    #[test]
    fn none_when_no_hooks_state() {
        let input = make_input_no_hooks();
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        assert_eq!(render(&ctx), None);
    }

    /// hooks_state present but all 4 token counts are 0 -> None
    #[test]
    fn none_when_all_counts_zero() {
        let input = make_input(
            Some("claude-sonnet-4-6"),
            Some(0),
            Some(0),
            Some(0),
            Some(0),
        );
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        assert_eq!(render(&ctx), None);
    }

    /// Fallback: no hooks_state but cost_usd present -> uses cost_usd
    #[test]
    fn fallback_to_cost_usd_when_no_hooks_state() {
        let input = Input {
            cost_usd: Some(0.42),
            ..Input::default()
        };
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "$0.42");
    }

    // --- Sonnet pricing -----------------------------------------------------

    /// Sonnet 1000 in / 0 out -> 1000 * 3e-6 = $0.003 -> "0.3¢" (green)
    #[test]
    fn sonnet_1000_in_0_out_is_cents() {
        let input = make_input(Some("claude-sonnet-4-6"), Some(1000), Some(0), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert_eq!(strip_ansi(&result), "0.3\u{00a2}");
        // green for < $0.10
        assert!(result.contains("\x1b[32m"), "should be green: {result:?}");
    }

    /// Sonnet 0 in / 1000 out -> 1000 * 15e-6 = $0.015 -> "$0.02" (green)
    #[test]
    fn sonnet_0_in_1000_out_is_dollar_format() {
        let input = make_input(Some("claude-sonnet-4-6"), Some(0), Some(1000), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert_eq!(strip_ansi(&result), "$0.02");
        // green for < $0.10
        assert!(result.contains("\x1b[32m"), "should be green: {result:?}");
    }

    // --- Opus pricing -------------------------------------------------------

    /// Opus 1000 in / 1000 out -> 0.015 + 0.075 = $0.09 (green)
    #[test]
    fn opus_1000_in_1000_out() {
        let input = make_input(Some("claude-opus-4-7"), Some(1000), Some(1000), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert_eq!(strip_ansi(&result), "$0.09");
        assert!(result.contains("\x1b[32m"), "should be green: {result:?}");
    }

    /// Opus 100K in / 50K out -> $5.25 (yellow $1–$10)
    #[test]
    fn opus_100k_in_50k_out_in_yellow_band() {
        let input = make_input(
            Some("claude-opus-4-7"),
            Some(100_000),
            Some(50_000),
            None,
            None,
        );
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert_eq!(strip_ansi(&result), "$5.25");
        assert!(result.contains("\x1b[33m"), "should be yellow: {result:?}");
    }

    // --- Cache tokens -------------------------------------------------------

    /// Cache write tokens included: sonnet 1000 cache_write * 3.75e-6 = "0.4¢"
    #[test]
    fn cache_write_tokens_included() {
        let input = make_input(
            Some("claude-sonnet-4-6"),
            Some(0),
            Some(0),
            Some(1000),
            None,
        );
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "0.4\u{00a2}");
    }

    /// Cache read tokens included: sonnet 1000 cache_read * 0.3e-6 = "0.0¢"
    #[test]
    fn cache_read_tokens_included() {
        let input = make_input(
            Some("claude-sonnet-4-6"),
            Some(0),
            Some(0),
            None,
            Some(1000),
        );
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "0.0\u{00a2}");
    }

    // --- Model prefix matching ----------------------------------------------

    #[test]
    fn opus_4_7_model_uses_opus_pricing() {
        // 1000 * 15e-6 = 0.015 -> "$0.02"
        let input = make_input(Some("claude-opus-4-7"), Some(1000), Some(0), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "$0.02");
    }

    #[test]
    fn opus_4_7_with_date_suffix_uses_opus_pricing() {
        let input = make_input(
            Some("claude-opus-4-7-20251022"),
            Some(1000),
            Some(0),
            None,
            None,
        );
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "$0.02");
    }

    #[test]
    fn opus_4_6_model_uses_opus_pricing() {
        let input = make_input(Some("claude-opus-4-6"), Some(1000), Some(0), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "$0.02");
    }

    #[test]
    fn sonnet_4_6_model_uses_sonnet_pricing() {
        // 1000 * 3e-6 = 0.003 -> "0.3¢"
        let input = make_input(Some("claude-sonnet-4-6"), Some(1000), Some(0), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "0.3\u{00a2}");
    }

    #[test]
    fn haiku_4_5_model_uses_haiku_pricing() {
        // 1000 * 0.8e-6 = 0.0008 -> "0.1¢"
        let input = make_input(Some("claude-haiku-4-5"), Some(1000), Some(0), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "0.1\u{00a2}");
    }

    #[test]
    fn unknown_model_falls_back_to_sonnet_pricing() {
        // sonnet: 1000 * 3e-6 = 0.003 -> "0.3¢"
        let input = make_input(Some("gpt-4"), Some(1000), Some(0), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "0.3\u{00a2}");
    }

    // --- Boundary formatting ------------------------------------------------

    /// cost = $0.010005 >= $0.01 -> "$0.01" (not cents)
    #[test]
    fn boundary_at_one_cent_is_dollar_format() {
        // sonnet 667 out * 15e-6 = $0.010005
        let input = make_input(Some("claude-sonnet-4-6"), Some(0), Some(667), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "$0.01");
    }

    /// cost >= $100 -> "$100" (no pennies)
    #[test]
    fn boundary_at_100_dollars_drops_pennies() {
        // sonnet 6_666_667 out * 15e-6 = $100.00005
        let input = make_input(
            Some("claude-sonnet-4-6"),
            Some(0),
            Some(6_666_667),
            None,
            None,
        );
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        assert_eq!(render(&ctx).unwrap(), "$100");
    }

    // --- Color tests --------------------------------------------------------

    /// TrueColor high cost (>$10) -> uses paint, contains TrueColor red
    #[test]
    fn high_cost_truecolor_has_red() {
        // opus 1M in + 1M out = $90 -> red
        let input = make_input(
            Some("claude-opus-4-7"),
            Some(1_000_000),
            Some(1_000_000),
            None,
            None,
        );
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let result = render(&ctx).unwrap();
        // paint(TrueColor, Red, ...) emits \x1b[38;2;248;113;113m
        assert!(
            result.contains("\x1b[38;2;"),
            "TrueColor should use RGB escape: {result:?}"
        );
        assert!(!strip_ansi(&result).is_empty(), "should have label");
    }

    /// Color16 high cost (>$10) -> raw \x1b[31m red
    #[test]
    fn high_cost_color16_has_red_ansi() {
        let input = make_input(
            Some("claude-opus-4-7"),
            Some(1_000_000),
            Some(1_000_000),
            None,
            None,
        );
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert!(result.contains("\x1b[31m"), "Color16 red: {result:?}");
    }

    /// Color16 low cost (<$0.10) -> raw \x1b[32m green
    #[test]
    fn low_cost_color16_has_green_ansi() {
        let input = make_input(Some("claude-sonnet-4-6"), Some(100), Some(0), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert!(result.contains("\x1b[32m"), "Color16 green: {result:?}");
    }

    /// TrueColor low cost (<$0.10) -> paint emits RGB green escape
    #[test]
    fn low_cost_truecolor_has_rgb_green() {
        let input = make_input(Some("claude-sonnet-4-6"), Some(100), Some(0), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[38;2;"),
            "TrueColor should use RGB escape: {result:?}"
        );
    }

    /// Mono -> no escape codes at all
    #[test]
    fn mono_produces_no_ansi() {
        let input = make_input(
            Some("claude-sonnet-4-6"),
            Some(1000),
            Some(1000),
            None,
            None,
        );
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render(&ctx).unwrap();
        assert!(!result.contains('\x1b'), "Mono: {result:?}");
    }

    /// Mid-range ($0.10–$1) with Color16 -> no ANSI codes
    #[test]
    fn mid_range_cost_no_color() {
        // sonnet 10000 out * 15e-6 = $0.15
        let input = make_input(Some("claude-sonnet-4-6"), Some(0), Some(10_000), None, None);
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert!(!result.contains('\x1b'), "mid-range no color: {result:?}");
        assert_eq!(result, "$0.15");
    }

    /// Yellow band ($1–$10) with Color16
    #[test]
    fn yellow_band_cost_has_yellow_ansi() {
        // sonnet 100K out * 15e-6 = $1.50
        let input = make_input(
            Some("claude-sonnet-4-6"),
            Some(0),
            Some(100_000),
            None,
            None,
        );
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert!(result.contains("\x1b[33m"), "yellow: {result:?}");
        assert_eq!(strip_ansi(&result), "$1.50");
    }
}
