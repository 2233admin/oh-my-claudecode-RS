use std::fs;
use std::path::Path;

use crate::elements::RenderContext;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    if hook_enabled(ctx) || state_file_enabled(ctx) {
        Some(format!("🤖 {}", ctx.strings.autopilot))
    } else {
        None
    }
}

fn hook_enabled(ctx: &RenderContext<'_>) -> bool {
    ctx.input
        .hooks_state
        .as_ref()
        .and_then(|state| {
            state
                .get("autopilot")
                .or_else(|| state.get("autopilot_enabled"))
                .and_then(|value| value.as_bool())
        })
        .unwrap_or(false)
}

fn state_file_enabled(ctx: &RenderContext<'_>) -> bool {
    let Some(session_id) = ctx.input.session_id.as_deref() else {
        return false;
    };
    let cwd = ctx.input.cwd.as_deref().unwrap_or(".");
    let dir = Path::new(cwd)
        .join(".omc")
        .join("state")
        .join("sessions")
        .join(session_id);
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };

    for entry in entries.flatten() {
        if entry.path().extension().and_then(|v| v.to_str()) != Some("json") {
            continue;
        }
        let Ok(raw) = fs::read_to_string(entry.path()) else {
            continue;
        };
        if raw.contains("\"autopilot\":true") || raw.contains("\"autopilot_enabled\":true") {
            return true;
        }
    }
    false
}
