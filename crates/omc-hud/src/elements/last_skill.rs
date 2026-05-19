use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let skill = ctx.transcript?.last_activated_skill.as_deref()?;
    // Display name: last segment after ':'
    let display = skill.rsplit(':').next().unwrap_or(skill);
    let label = format!("skill:{display}");
    if matches!(ctx.color_level, ColorLevel::Mono) {
        Some(label)
    } else {
        Some(format!("\x1b[36m{label}\x1b[0m"))
    }
}
