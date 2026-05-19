use std::time::Duration;
use crate::git_util::git_output_timeout;

use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

// ---------------------------------------------------------------------------
// Git data extraction
// ---------------------------------------------------------------------------

struct GitData {
    branch: String,
    staged: u32,
    modified: u32,
    untracked: u32,
    ahead: u32,
    behind: u32,
}

fn get_branch(cwd: &str) -> Option<String> {
    let output = git_output_timeout(
        &["-C", cwd, "rev-parse", "--abbrev-ref", "HEAD"],
        Duration::from_millis(2000),
    )?;
    if !output.status.success() {
        return None;
    }
    let branch = String::from_utf8(output.stdout).ok()?;
    let branch = branch.trim().to_string();
    if branch.is_empty() { None } else { Some(branch) }
}

/// Parse `git --no-optional-locks status --porcelain -b` for all status counts.
/// Returns (staged, modified, untracked, ahead, behind).
fn get_status_counts(cwd: &str) -> Option<(u32, u32, u32, u32, u32)> {
    let output = git_output_timeout(
        &["-C", cwd, "--no-optional-locks", "status", "--porcelain", "-b"],
        Duration::from_millis(2000),
    )?;
    if !output.status.success() {
        return Some((0, 0, 0, 0, 0));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut staged = 0u32;
    let mut modified = 0u32;
    let mut untracked = 0u32;
    let mut ahead = 0u32;
    let mut behind = 0u32;

    for (i, line) in text.lines().enumerate() {
        if i == 0 {
            // Branch tracking line: ## main...origin/main [ahead 3, behind 1]
            if let Some(m) = regex_ahead(line) { ahead = m; }
            if let Some(m) = regex_behind(line) { behind = m; }
            continue;
        }
        if line.len() < 2 { continue; }
        let xy: Vec<char> = line.chars().take(2).collect();
        let x = xy[0];
        let y = xy[1];
        if x == '?' && y == '?' {
            untracked += 1;
        } else {
            if x != ' ' && x != '?' { staged += 1; }
            if matches!(y, 'M' | 'D') { modified += 1; }
        }
    }
    Some((staged, modified, untracked, ahead, behind))
}

fn regex_ahead(line: &str) -> Option<u32> {
    let idx = line.find("ahead ")?;
    let rest = &line[idx + 6..];
    rest.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}

fn regex_behind(line: &str) -> Option<u32> {
    let idx = line.find("behind ")?;
    let rest = &line[idx + 7..];
    rest.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}

fn collect_git_data(cwd: &str) -> Option<GitData> {
    let branch = get_branch(cwd)?;
    let (staged, modified, untracked, ahead, behind) = get_status_counts(cwd)?;
    Some(GitData { branch, staged, modified, untracked, ahead, behind })
}

// ---------------------------------------------------------------------------
// Pure formatting
// ---------------------------------------------------------------------------

fn color_enabled(level: ColorLevel) -> bool {
    !matches!(level, ColorLevel::Mono)
}

pub fn render_with_data(
    branch: &str,
    staged: u32,
    modified: u32,
    untracked: u32,
    ahead: u32,
    behind: u32,
    color_level: ColorLevel,
) -> String {
    let dirty = staged > 0 || modified > 0 || untracked > 0;

    if !color_enabled(color_level) {
        let branch_part = if dirty {
            format!("git:({}*)", branch)
        } else {
            format!("git:({})", branch)
        };
        let mut parts = vec![branch_part];
        if staged > 0 { parts.push(format!("+{staged}")); }
        if modified > 0 { parts.push(format!("~{modified}")); }
        if untracked > 0 { parts.push(format!("?{untracked}")); }
        if ahead > 0 { parts.push(format!("^{ahead}")); }
        if behind > 0 { parts.push(format!("v{behind}")); }
        return parts.join(" ");
    }

    let branch_color = if dirty { "\x1b[33m" } else { "\x1b[32m" };
    let reset = "\x1b[0m";

    let branch_display = if dirty {
        format!("git:({}{}*{})", branch_color, branch, reset)
    } else {
        format!("git:({}{}{})", branch_color, branch, reset)
    };

    if !dirty && ahead == 0 && behind == 0 {
        return branch_display;
    }

    let mut parts = vec![branch_display];
    if staged > 0 { parts.push(format!("\x1b[32m+{staged}{reset}")); }
    if modified > 0 { parts.push(format!("\x1b[31m~{modified}{reset}")); }
    if untracked > 0 { parts.push(format!("\x1b[36m?{untracked}{reset}")); }
    if ahead > 0 { parts.push(format!("\x1b[32m\u{21e1}{ahead}{reset}")); }   // ⇡
    if behind > 0 { parts.push(format!("\x1b[31m\u{21e3}{behind}{reset}")); } // ⇣

    parts.join(" ")
}

// ---------------------------------------------------------------------------
// Public render entry point
// ---------------------------------------------------------------------------

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let cwd = ctx.input.current_dir()?;
    let data = collect_git_data(cwd)?;
    Some(render_with_data(
        &data.branch,
        data.staged,
        data.modified,
        data.untracked,
        data.ahead,
        data.behind,
        ctx.color_level,
    ))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn strip_ansi(s: &str) -> String {
        let mut out = String::new();
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                for ch in chars.by_ref() { if ch == 'm' { break; } }
            } else {
                out.push(c);
            }
        }
        out
    }

    #[test]
    fn clean_mono() {
        assert_eq!(render_with_data("main", 0, 0, 0, 0, 0, ColorLevel::Mono), "git:(main)");
    }

    #[test]
    fn dirty_modified_mono() {
        let r = render_with_data("main", 0, 3, 0, 0, 0, ColorLevel::Mono);
        assert_eq!(r, "git:(main*) ~3");
    }

    #[test]
    fn staged_and_ahead_mono() {
        let r = render_with_data("main", 2, 0, 0, 3, 0, ColorLevel::Mono);
        assert_eq!(r, "git:(main*) +2 ^3");
    }

    #[test]
    fn clean_with_ahead_behind_mono() {
        let r = render_with_data("main", 0, 0, 0, 1, 2, ColorLevel::Mono);
        assert_eq!(r, "git:(main) ^1 v2");
    }

    #[test]
    fn clean_color16_has_green_branch() {
        let r = render_with_data("main", 0, 0, 0, 0, 0, ColorLevel::Color16);
        assert!(r.contains("\x1b[32m"), "clean should be green: {r:?}");
        assert_eq!(strip_ansi(&r), "git:(main)");
    }

    #[test]
    fn dirty_color16_has_yellow_branch() {
        let r = render_with_data("main", 0, 3, 0, 0, 0, ColorLevel::Color16);
        assert!(r.contains("\x1b[33m"), "dirty branch yellow: {r:?}");
        assert_eq!(strip_ansi(&r), "git:(main*) ~3");
    }

    #[test]
    fn ahead_has_green_arrow() {
        let r = render_with_data("main", 0, 0, 0, 2, 0, ColorLevel::Color16);
        assert!(r.contains('\u{21e1}'), "ahead arrow: {r:?}");
    }

    #[test]
    fn behind_has_red_arrow() {
        let r = render_with_data("main", 0, 0, 0, 0, 1, ColorLevel::Color16);
        assert!(r.contains('\u{21e3}'), "behind arrow: {r:?}");
    }

    #[test]
    fn integration_repo_returns_some() {
        let repo_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
        use crate::cache::HudCache;
        use crate::input::Input;
        use crate::elements::RenderContext;
        let input = Input { cwd: Some(repo_root.to_string()), ..Input::default() };
        let cache = HudCache::new("test".to_string());
        let ctx = RenderContext::for_test(&input, &cache, ColorLevel::Mono);
        let result = render(&ctx);
        assert!(result.is_some(), "expected Some from actual repo");
        assert!(result.unwrap().starts_with("git:("));
    }
}
