use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

fn color_enabled(level: ColorLevel) -> bool {
    !matches!(level, ColorLevel::Mono)
}

fn severity_color(percent: u8, warn: u8, compact: u8) -> &'static str {
    if percent >= compact {
        "\x1b[31m"
    } else if percent >= warn {
        "\x1b[33m"
    } else {
        "\x1b[32m"
    }
}

fn severity_suffix(percent: u8, compact: u8, critical: u8) -> &'static str {
    if percent >= critical {
        " CRITICAL"
    } else if percent >= compact {
        " COMPRESS?"
    } else {
        ""
    }
}

/// Render a 10-char filled/empty progress bar, e.g. `[███████░░░]`.
fn render_bar(pct: u8, width: usize) -> String {
    let filled = ((pct as usize * width + 50) / 100).min(width);
    let empty = width - filled;
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

/// Format a token count as a compact string: `0`, `7K`, `2M`, etc.
fn format_token_count(t: u64) -> String {
    if t >= 1_000_000 {
        format!("{:.0}M", t as f64 / 1_000_000.0)
    } else if t >= 1_000 {
        format!("{}K", (t + 500) / 1_000)
    } else {
        t.to_string()
    }
}

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let pct = ctx.input.context_used_pct()? as u8;
    let warn = ctx.config.thresholds.context_warning as u8;
    let compact = ctx.config.thresholds.context_compact as u8;
    let critical = ctx.config.thresholds.context_critical as u8;

    let label = ctx.strings.ctx.to_ascii_lowercase();
    let bar = render_bar(pct, 10);
    let suffix = severity_suffix(pct, compact, critical);

    let abs_str = match (ctx.input.tokens_used(), ctx.input.tokens_max()) {
        (Some(used), Some(max)) => Some(format!(
            "{}/{}",
            format_token_count(used),
            format_token_count(max)
        )),
        _ => None,
    };

    if color_enabled(ctx.color_level) {
        let color = severity_color(pct, warn, compact);
        let core = format!("{label}:{color}{bar}{pct}%{suffix}\x1b[0m");
        match abs_str {
            Some(abs) => Some(format!("{core} \x1b[2m{abs}\x1b[0m")),
            None => Some(core),
        }
    } else {
        let core = format!("{label}:{bar}{pct}%{suffix}");
        match abs_str {
            Some(abs) => Some(format!("{core} {abs}")),
            None => Some(core),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::HudCache;
    use crate::input::Input;

    fn make_ctx<'a>(input: &'a Input, cache: &'a HudCache, level: ColorLevel) -> RenderContext<'a> {
        RenderContext::for_test(input, cache, level)
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
        // 0% → [░░░░░░░░░░], 0/10K
        assert_eq!(result, "ctx:[░░░░░░░░░░]0% 0/10K");
    }

    // --- 67% tests ---

    #[test]
    fn sixty_seven_percent_no_color() {
        let input = make_input(Some(6700), Some(10000));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render(&ctx).unwrap();
        // 67% → (67*10+50)/100=7 filled → [███████░░░], 6700→7K, 10000→10K
        assert_eq!(result, "ctx:[███████░░░]67% 7K/10K");
    }

    #[test]
    fn sixty_seven_percent_truecolor_has_green_and_reset() {
        let input = make_input(Some(6700), Some(10000));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[32m"),
            "should contain green: {result:?}"
        );
        assert!(
            result.contains("\x1b[0m"),
            "should contain reset: {result:?}"
        );
        assert_eq!(strip_ansi(&result), "ctx:[███████░░░]67% 7K/10K");
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
        // 70% → (70*10+50)/100=7 → [███████░░░], 7K/10K
        assert_eq!(strip_ansi(&result), "ctx:[███████░░░]70% 7K/10K");
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
        // 79% → (79*10+50)/100=8 → [████████░░], 7900→8K, 10000→10K
        assert_eq!(strip_ansi(&result), "ctx:[████████░░]79% 8K/10K");
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
        // 80% → (80*10+50)/100=8 → [████████░░], 8K/10K
        assert_eq!(strip_ansi(&result), "ctx:[████████░░]80% COMPRESS? 8K/10K");
    }

    // --- 89% red + CRITICAL (default contextCritical=85) ---

    #[test]
    fn eighty_nine_percent_is_red_with_critical() {
        let input = make_input(Some(8900), Some(10000));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[31m"),
            "should contain red: {result:?}"
        );
        // 89% >= contextCritical(85) → CRITICAL
        assert_eq!(strip_ansi(&result), "ctx:[█████████░]89% CRITICAL 9K/10K");
    }

    // --- 84% red + COMPRESS? (between compact=80 and critical=85) ---

    #[test]
    fn eighty_four_percent_is_red_with_compress() {
        let input = make_input(Some(8400), Some(10000));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert!(
            result.contains("\x1b[31m"),
            "should contain red: {result:?}"
        );
        // 84% < contextCritical(85) → COMPRESS?
        assert_eq!(strip_ansi(&result), "ctx:[████████░░]84% COMPRESS? 8K/10K");
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
        // 90% → (90*10+50)/100=9 → [█████████░], 9K/10K
        assert_eq!(strip_ansi(&result), "ctx:[█████████░]90% CRITICAL 9K/10K");
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
        // 100% → [██████████], 10K/10K
        assert_eq!(strip_ansi(&result), "ctx:[██████████]100% CRITICAL 10K/10K");
    }

    // --- Tokens > max clamps to 100% ---

    #[test]
    fn tokens_exceeding_max_clamps_to_hundred() {
        let input = make_input(Some(15000), Some(10000));
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Color16);
        let result = render(&ctx).unwrap();
        assert_eq!(strip_ansi(&result), "ctx:[██████████]100% CRITICAL 15K/10K");
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
        assert_eq!(render(&ctx).unwrap(), "ctx:[███████░░░]67% 7K/10K");
    }

    // --- render_bar unit tests ---

    #[test]
    fn bar_0pct() {
        assert_eq!(render_bar(0, 10), "[░░░░░░░░░░]");
    }

    #[test]
    fn bar_100pct() {
        assert_eq!(render_bar(100, 10), "[██████████]");
    }

    #[test]
    fn bar_67pct() {
        // (67*10+50)/100 = 7
        assert_eq!(render_bar(67, 10), "[███████░░░]");
    }

    // --- format_token_count unit tests ---

    #[test]
    fn token_count_zero() {
        assert_eq!(format_token_count(0), "0");
    }

    #[test]
    fn token_count_small() {
        assert_eq!(format_token_count(999), "999");
    }

    #[test]
    fn token_count_1k() {
        assert_eq!(format_token_count(1000), "1K");
    }

    #[test]
    fn token_count_6700() {
        // (6700+500)/1000 = 7
        assert_eq!(format_token_count(6700), "7K");
    }

    #[test]
    fn token_count_1m() {
        assert_eq!(format_token_count(1_000_000), "1M");
    }
}
