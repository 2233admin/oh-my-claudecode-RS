use std::process::Command;

use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

// ---------------------------------------------------------------------------
// Git data extraction via subprocess
// ---------------------------------------------------------------------------

/// Raw git data extracted by spawning `git`.
struct GitData {
    branch: String,
    modified: u32,
    untracked: u32,
}

/// Spawn `git -C <cwd> rev-parse --abbrev-ref HEAD`.
/// Returns None on any failure (not a repo, spawn error, non-zero exit).
fn get_branch(cwd: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["-C", cwd, "rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let branch = String::from_utf8(output.stdout).ok()?;
    let branch = branch.trim().to_string();
    if branch.is_empty() {
        return None;
    }
    Some(branch)
}

/// Spawn `git -C <cwd> status --porcelain`.
/// Returns (modified_count, untracked_count).
/// Returns None on spawn failure; non-zero exit returns (0, 0) treated as clean.
fn get_status_counts(cwd: &str) -> Option<(u32, u32)> {
    let output = Command::new("git")
        .args(["-C", cwd, "status", "--porcelain"])
        .output()
        .ok()?;
    // non-zero exit from `status --porcelain` is unusual but treat as clean
    if !output.status.success() {
        return Some((0, 0));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut modified: u32 = 0;
    let mut untracked: u32 = 0;
    for line in text.lines() {
        if line.starts_with("??") {
            untracked += 1;
        } else if line.len() >= 2 {
            // Any non-?? line with a meaningful XY code counts as modified
            let xy: Vec<char> = line.chars().take(2).collect();
            let x = xy[0];
            let y = xy[1];
            if matches!(x, 'M' | 'A' | 'D' | 'R' | 'C' | 'U')
                || matches!(y, 'M' | 'A' | 'D' | 'R' | 'C' | 'U')
            {
                modified += 1;
            }
        }
    }
    Some((modified, untracked))
}

fn collect_git_data(cwd: &str) -> Option<GitData> {
    let branch = get_branch(cwd)?;
    let (modified, untracked) = get_status_counts(cwd)?;
    Some(GitData {
        branch,
        modified,
        untracked,
    })
}

// ---------------------------------------------------------------------------
// Pure formatting (testable without spawning)
// ---------------------------------------------------------------------------

/// Returns true if the color level supports ANSI.
fn color_enabled(level: ColorLevel) -> bool {
    !matches!(level, ColorLevel::Mono)
}

/// Format the git status string.
/// This is the pure function tested exhaustively.
pub fn render_with_data(
    branch: &str,
    modified: u32,
    untracked: u32,
    color_level: ColorLevel,
) -> String {
    let dirty = modified > 0 || untracked > 0;

    if !color_enabled(color_level) {
        // Plain text, no ANSI
        let branch_part = if dirty {
            format!("git:({}*)", branch)
        } else {
            format!("git:({})", branch)
        };

        if !dirty {
            return branch_part;
        }

        let mut parts = vec![branch_part];
        if modified > 0 {
            parts.push(format!("~{modified}"));
        }
        if untracked > 0 {
            parts.push(format!("?{untracked}"));
        }
        return parts.join(" ");
    }

    // Colored output
    // Branch color: green if clean, yellow if dirty
    let branch_color = if dirty { "\x1b[33m" } else { "\x1b[32m" };
    let reset = "\x1b[0m";

    let branch_display = if dirty {
        format!("git:({}{}*{})", branch_color, branch, reset)
    } else {
        format!("git:({}{}{})", branch_color, branch, reset)
    };

    if !dirty {
        return branch_display;
    }

    let mut parts = vec![branch_display];

    if modified > 0 {
        parts.push(format!("\x1b[31m~{}{}", modified, reset));
    }
    if untracked > 0 {
        parts.push(format!("\x1b[36m?{}{}", untracked, reset));
    }

    parts.join(" ")
}

// ---------------------------------------------------------------------------
// Public render entry point
// ---------------------------------------------------------------------------

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let cwd = ctx.input.cwd.as_deref()?;
    let data = collect_git_data(cwd)?;
    Some(render_with_data(
        &data.branch,
        data.modified,
        data.untracked,
        ctx.color_level,
    ))
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

    // --- render_with_data: plain text (Mono) --------------------------------

    #[test]
    fn clean_mono_format() {
        let result = render_with_data("main", 0, 0, ColorLevel::Mono);
        assert_eq!(result, "git:(main)");
    }

    #[test]
    fn dirty_modified_only_mono() {
        let result = render_with_data("main", 3, 0, ColorLevel::Mono);
        assert_eq!(result, "git:(main*) ~3");
    }

    #[test]
    fn dirty_untracked_only_mono() {
        let result = render_with_data("main", 0, 2, ColorLevel::Mono);
        assert_eq!(result, "git:(main*) ?2");
    }

    #[test]
    fn dirty_both_counts_mono() {
        let result = render_with_data("main", 3, 2, ColorLevel::Mono);
        assert_eq!(result, "git:(main*) ~3 ?2");
    }

    #[test]
    fn branch_with_slash_mono() {
        let result = render_with_data("feat/some-thing", 1, 0, ColorLevel::Mono);
        assert_eq!(result, "git:(feat/some-thing*) ~1");
    }

    #[test]
    fn single_untracked_mono() {
        let result = render_with_data("main", 0, 1, ColorLevel::Mono);
        assert_eq!(result, "git:(main*) ?1");
    }

    #[test]
    fn single_modified_mono() {
        let result = render_with_data("develop", 1, 0, ColorLevel::Mono);
        assert_eq!(result, "git:(develop*) ~1");
    }

    // --- render_with_data: Color16 ------------------------------------------

    #[test]
    fn clean_color16_has_green_branch() {
        let result = render_with_data("main", 0, 0, ColorLevel::Color16);
        assert!(
            result.contains("\x1b[32m"),
            "clean should be green: {result:?}"
        );
        assert!(result.contains("\x1b[0m"), "should have reset: {result:?}");
        assert_eq!(strip_ansi(&result), "git:(main)");
    }

    #[test]
    fn dirty_color16_has_yellow_branch() {
        let result = render_with_data("main", 3, 0, ColorLevel::Color16);
        assert!(
            result.contains("\x1b[33m"),
            "dirty branch should be yellow: {result:?}"
        );
        assert_eq!(strip_ansi(&result), "git:(main*) ~3");
    }

    #[test]
    fn dirty_color16_modified_has_red() {
        let result = render_with_data("main", 3, 0, ColorLevel::Color16);
        assert!(
            result.contains("\x1b[31m"),
            "modified count should be red: {result:?}"
        );
    }

    #[test]
    fn dirty_color16_untracked_has_cyan() {
        let result = render_with_data("main", 0, 2, ColorLevel::Color16);
        assert!(
            result.contains("\x1b[36m"),
            "untracked count should be cyan: {result:?}"
        );
        assert_eq!(strip_ansi(&result), "git:(main*) ?2");
    }

    #[test]
    fn dirty_both_color16_has_all_colors() {
        let result = render_with_data("main", 3, 1, ColorLevel::Color16);
        assert!(result.contains("\x1b[33m"), "yellow branch: {result:?}");
        assert!(result.contains("\x1b[31m"), "red modified: {result:?}");
        assert!(result.contains("\x1b[36m"), "cyan untracked: {result:?}");
        assert_eq!(strip_ansi(&result), "git:(main*) ~3 ?1");
    }

    // --- render_with_data: TrueColor / Color256 (same ANSI codes as Color16 here) ---

    #[test]
    fn clean_truecolor_has_green_escape() {
        let result = render_with_data("main", 0, 0, ColorLevel::TrueColor);
        assert!(
            result.contains("\x1b[32m"),
            "TrueColor clean: green: {result:?}"
        );
    }

    #[test]
    fn dirty_truecolor_has_yellow_escape() {
        let result = render_with_data("main", 1, 0, ColorLevel::TrueColor);
        assert!(
            result.contains("\x1b[33m"),
            "TrueColor dirty: yellow: {result:?}"
        );
    }

    #[test]
    fn dirty_truecolor_modified_has_red_escape() {
        let result = render_with_data("main", 2, 0, ColorLevel::TrueColor);
        assert!(
            result.contains("\x1b[31m"),
            "TrueColor modified: red: {result:?}"
        );
    }

    #[test]
    fn dirty_truecolor_untracked_has_cyan_escape() {
        let result = render_with_data("main", 0, 3, ColorLevel::TrueColor);
        assert!(
            result.contains("\x1b[36m"),
            "TrueColor untracked: cyan: {result:?}"
        );
    }

    #[test]
    fn color_none_produces_no_ansi() {
        let result = render_with_data("main", 3, 2, ColorLevel::Mono);
        assert!(
            !result.contains('\x1b'),
            "Mono must have no ANSI: {result:?}"
        );
    }

    // --- render_with_data: exact colored strings ----------------------------

    #[test]
    fn exact_clean_color16_string() {
        let result = render_with_data("main", 0, 0, ColorLevel::Color16);
        assert_eq!(result, "git:(\x1b[32mmain\x1b[0m)");
    }

    #[test]
    fn exact_dirty_modified_only_color16_string() {
        let result = render_with_data("main", 3, 0, ColorLevel::Color16);
        assert_eq!(result, "git:(\x1b[33mmain*\x1b[0m) \x1b[31m~3\x1b[0m");
    }

    #[test]
    fn exact_dirty_both_color16_string() {
        let result = render_with_data("main", 3, 1, ColorLevel::Color16);
        assert_eq!(
            result,
            "git:(\x1b[33mmain*\x1b[0m) \x1b[31m~3\x1b[0m \x1b[36m?1\x1b[0m"
        );
    }

    #[test]
    fn exact_dirty_untracked_only_color16_string() {
        let result = render_with_data("main", 0, 2, ColorLevel::Color16);
        assert_eq!(result, "git:(\x1b[33mmain*\x1b[0m) \x1b[36m?2\x1b[0m");
    }

    // --- render(ctx): None cases -------------------------------------------

    #[test]
    fn none_when_cwd_is_none() {
        let input = Input {
            cwd: None,
            ..Input::default()
        };
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        assert_eq!(render(&ctx), None);
    }

    #[test]
    fn none_when_cwd_is_not_a_git_repo() {
        // Use a path that definitely doesn't exist / isn't a git repo
        let input = Input {
            cwd: Some("C:/__definitely_not_a_repo__".to_string()),
            ..Input::default()
        };
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::TrueColor);
        assert_eq!(render(&ctx), None);
    }

    // --- render(ctx): integration test against actual repo -----------------

    #[test]
    fn integration_actual_repo_returns_some_with_master() {
        // This test spawns real git against the workspace repo root.
        // Uses CARGO_MANIFEST_DIR (crates/omc-hud) + ../.. to reach repo root.
        // Works on both Windows and Linux CI runners.
        // Fixed in chore: fix CI -- kept as Chesterton fence breadcrumb (rule 42).
        let repo_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
        let input = Input {
            cwd: Some(repo_root.to_string()),
            ..Input::default()
        };
        let cache = empty_cache();
        let ctx = make_ctx(&input, &cache, ColorLevel::Mono);
        let result = render(&ctx);
        assert!(
            result.is_some(),
            "expected Some from actual repo at {repo_root:?}"
        );
        let s = result.unwrap();
        // Only check the format prefix; branch name varies across environments.
        assert!(
            s.starts_with("git:("),
            "expected 'git:(...' format but got: {s:?}"
        );
    }
}
