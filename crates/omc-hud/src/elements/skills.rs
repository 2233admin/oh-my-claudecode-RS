use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

const MAGENTA: &str = "\x1b[35m";
const BRIGHT_MAGENTA: &str = "\x1b[95m";
const CYAN: &str = "\x1b[36m";
const RESET: &str = "\x1b[0m";

/// Renders active skill mode badges (ultrawork/ralph) plus last activated skill.
/// Matches JS renderSkills() — combines active modes with last skill.
pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let ralph_active = ctx
        .omc_state
        .ralph
        .as_ref()
        .map(|r| r.active)
        .unwrap_or(false);
    let ultrawork_active = ctx
        .omc_state
        .ultrawork
        .as_ref()
        .map(|u| u.active)
        .unwrap_or(false);
    let last_skill = ctx
        .transcript
        .and_then(|t| t.last_activated_skill.as_deref());

    let mut parts: Vec<String> = Vec::new();

    let color = if matches!(ctx.color_level, ColorLevel::Mono) {
        ""
    } else {
        MAGENTA
    };
    let bright = if matches!(ctx.color_level, ColorLevel::Mono) {
        ""
    } else {
        BRIGHT_MAGENTA
    };
    let reset = if matches!(ctx.color_level, ColorLevel::Mono) {
        ""
    } else {
        RESET
    };

    if ralph_active && ultrawork_active {
        parts.push(format!("{bright}ultrawork+ralph{reset}"));
    } else if ultrawork_active {
        parts.push(format!("{color}ultrawork{reset}"));
    } else if ralph_active {
        parts.push(format!("{color}ralph{reset}"));
    }

    // Last skill, if not the active mode itself
    if let Some(skill) = last_skill {
        let is_active_mode = (skill == "ralph" && ralph_active)
            || (skill == "ultrawork" && ultrawork_active)
            || (skill == "ultrawork+ralph" && ralph_active && ultrawork_active);
        if !is_active_mode {
            let display = skill.rsplit(':').next().unwrap_or(skill);
            let label = format!("skill:{display}");
            if matches!(ctx.color_level, ColorLevel::Mono) {
                parts.push(label);
            } else {
                parts.push(format!("{CYAN}{label}{RESET}"));
            }
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}
