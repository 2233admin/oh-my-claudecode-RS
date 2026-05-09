//! tmux session detection for notifications.

use std::process::Command;

/// Get the current tmux session name.
/// Returns `None` if not running inside tmux.
static TMUX: &str = "TMUX";
static TMUX_PANE: &str = "TMUX_PANE";

pub fn current_session() -> Option<String> {
    std::env::var(TMUX).ok()?;

    // Try $TMUX_PANE to find the session this process belongs to.
    if let Ok(pane_id) = std::env::var(TMUX_PANE)
        && !pane_id.is_empty()
        && let Ok(output) = run_tmux_cmd(&["list-panes", "-a", "-F", "#{pane_id} #{session_name}"])
    {
        for line in output.lines() {
            if let Some(rest) = line.strip_prefix(&format!("{pane_id} ")) {
                let name = rest.trim();
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
    }

    // Fallback: ask the attached session.
    if let Ok(output) = run_tmux_cmd(&["display-message", "-p", "#S"]) {
        let name = output.trim().to_string();
        if !name.is_empty() {
            return Some(name);
        }
    }

    None
}

/// Get the current tmux pane ID (e.g. "%0").
/// Returns `None` if not running inside tmux.
static TMUX: &str = "TMUX";
static TMUX_PANE: &str = "TMUX_PANE";

pub fn current_pane_id() -> Option<String> {
    std::env::var(TMUX).ok()?;

    // Prefer $TMUX_PANE env var.
    if let Ok(pane) = std::env::var(TMUX_PANE)
        && pane.starts_with('%')
        && pane.len() > 1
        && pane[1..].chars().all(|c| c.is_ascii_digit())
    {
        return Some(pane);
    }

    // Fallback: ask tmux.
    if let Ok(output) = run_tmux_cmd(&["display-message", "-p", "#{pane_id}"]) {
        let pane = output.trim().to_string();
        if pane.starts_with('%') && pane.len() > 1 && pane[1..].chars().all(|c| c.is_ascii_digit())
        {
            return Some(pane);
        }
    }

    None
}

/// List active omc-team tmux sessions for a given team name.
pub fn team_sessions(team_name: &str) -> Vec<String> {
    let sanitized: Vec<&str> = team_name
        .matches(|c| c.is_ascii_alphanumeric() || c == '-')
        .collect();
    if sanitized.is_empty() {
        return Vec::new();
    }

    let prefix = format!("omc-team-{sanitized}-");
    let Ok(output) = run_tmux_cmd(&["list-sessions", "-F", "#{session_name}"]) else {
        return Vec::new();
    };

    output
        .lines()
        .filter_map(|s| {
            let s = s.trim();
            s.strip_prefix(&prefix).map(|name| name.to_string())
        })
        .collect()
}

/// Format tmux session info for display. Returns `None` if not in tmux.
pub fn format_info() -> Option<String> {
    current_session().map(|s| format!("tmux: {s}"))
}

/// Capture the last N lines of a tmux pane's content.
pub fn capture_pane(pane_id: &str, lines: u32) -> Option<String> {
    let start = format!("-{}", lines.saturating_sub(1));
    run_tmux_cmd(&["capture-pane", "-p", "-t", pane_id, "-S", &start])
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Run a tmux command and return its stdout. Returns an error if the command fails.
fn run_tmux_cmd(args: &[&str]) -> Result<String, TmuxError> {
    let output = Command::new("tmux")
        .args(args)
        .env_remove("TMUX")
        .output()
        .map_err(|e| TmuxError::Io(e.to_string()))?;

    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|e| TmuxError::Utf8(e.to_string()))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(TmuxError::Command(stderr.trim().to_string()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TmuxError {
    #[error("tmux I/O error: {0}")]
    Io(String),
    #[error("tmux command failed: {0}")]
    Command(String),
    #[error("tmux output not valid UTF-8: {0}")]
    Utf8(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    static TMUX: &str = "TMUX";
    fn format_info_without_tmux() {
        // Outside tmux, format_info should return None.
        // (This test only passes when not inside tmux.)
        if std::env::var(TMUX).is_err() {
            assert!(format_info().is_none());
        }
    }

    #[test]
    fn team_sessions_empty_name() {
        assert!(team_sessions("").is_empty());
        assert!(team_sessions("!!!").is_empty());
    }
}
