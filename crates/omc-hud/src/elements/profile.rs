use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

/// Extracts the active Claude profile name from CLAUDE_CONFIG_DIR.
/// Claude Code sets CLAUDE_CONFIG_DIR to ~/.claude/profiles/<name> when --profile is used.
fn get_profile_name() -> Option<String> {
    let config_dir = std::env::var("CLAUDE_CONFIG_DIR").ok()?;
    let path = std::path::Path::new(&config_dir);
    let name = path.file_name()?.to_string_lossy();
    // Only show if it looks like a profile (not the default ~/.claude dir)
    if name == ".claude" || name.is_empty() {
        return None;
    }
    Some(name.to_string())
}

pub fn render(_ctx: &RenderContext<'_>) -> Option<String> {
    let name = get_profile_name()?;
    let label = format!("profile:{name}");
    if matches!(_ctx.color_level, ColorLevel::Mono) {
        return Some(label);
    }
    // Bold — matches JS `bold(`profile:${name}`)`
    Some(format!("\x1b[1m{label}\x1b[0m"))
}
