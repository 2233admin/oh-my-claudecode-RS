use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let transcript = ctx.transcript?;
    let skill = transcript.last_activated_skill.as_deref()?;

    let label = format!("skill:{skill}");

    if matches!(ctx.color_level, ColorLevel::Mono) {
        Some(label)
    } else {
        // cyan
        Some(format!("\x1b[36m{label}\x1b[0m"))
    }
}
