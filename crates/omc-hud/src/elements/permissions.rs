use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let transcript = ctx.transcript?;
    let tool = transcript.pending_permission.as_deref()?;

    let label = format!("⚠ APPROVE? {tool}");

    if matches!(ctx.color_level, ColorLevel::Mono) {
        Some(label)
    } else {
        // yellow
        Some(format!("\x1b[33m{label}\x1b[0m"))
    }
}
