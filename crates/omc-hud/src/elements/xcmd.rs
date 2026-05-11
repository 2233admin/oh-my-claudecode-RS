//! x-cmd status element
//! 
//! Shows x-cmd installation status and package count.

use crate::elements::RenderContext;
use crate::terminal::ColorLevel;
use std::path::PathBuf;

/// Detect x-cmd installation path.
fn xcmd_root() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".x-cmd.root"))
}

/// Check if x-cmd is installed.
fn is_xcmd_installed() -> bool {
    xcmd_root().is_some_and(|p| p.exists())
}

/// Get x-cmd version (from VERSION file).
fn get_xcmd_version() -> Option<String> {
    xcmd_root()
        .and_then(|p| std::fs::read_to_string(p.join("VERSION")).ok())
        .map(|v| v.trim().to_string())
}

/// Count installed packages (env/lock/*.json files).
fn count_installed_packages() -> Option<usize> {
    xcmd_root().and_then(|p| {
        let env_dir = p.join("env/lock");
        std::fs::read_dir(env_dir).ok()
            .map(|entries| {
                entries.filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
                    .count()
            })
    })
}

fn color_enabled(level: ColorLevel) -> bool {
    !matches!(level, ColorLevel::Mono)
}

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    // Always show x-cmd status
    let installed = is_xcmd_installed();
    let version = get_xcmd_version();
    let pkg_count = count_installed_packages();
    
    let label = if installed {
        match (version, pkg_count) {
            (Some(v), Some(n)) => format!("x-cmd {v} ({n} pkg)"),
            (Some(v), None) => format!("x-cmd {v}"),
            (None, Some(n)) => format!("x-cmd ({n} pkg)"),
            (None, None) => "x-cmd".to_string(),
        }
    } else {
        return Some(format!(
            "{}\x1b[2m[no x-cmd]\x1b[0m",
            if color_enabled(ctx.color_level) { "\x1b[31m" } else { "" }
        ));
    };

    if color_enabled(ctx.color_level) {
        Some(format!("\x1b[36m{label}\x1b[0m")) // cyan
    } else {
        Some(label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xcmd_root_works() {
        // Just verify the function doesn't panic
        let _ = xcmd_root();
    }
}
