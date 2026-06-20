use crate::buddy_state::BuddyState;
use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

pub fn render(ctx: &RenderContext<'_>, buddy: Option<&BuddyState>) -> Option<String> {
    if !ctx.config.element_enabled("buddyReaction", true) {
        return None;
    }

    let buddy = buddy?;
    let text = buddy.active_reaction()?;

    // Truncate to 60 chars so it doesn't swallow the whole line
    let text = if text.chars().count() > 60 {
        let truncated: String = text.chars().take(57).collect();
        format!("{truncated}…")
    } else {
        text.to_string()
    };

    let name = buddy.name.as_deref().or(buddy.species.as_deref()).unwrap_or("buddy");

    if matches!(ctx.color_level, ColorLevel::Mono) {
        Some(format!("{name}: \"{text}\""))
    } else {
        Some(format!("\x1b[35m{name}\x1b[0m \x1b[2m\"\x1b[0m{text}\x1b[2m\"\x1b[0m"))
    }
}
