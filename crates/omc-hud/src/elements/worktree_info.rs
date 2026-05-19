use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let label = ctx.input.worktree_label()?;
    let text = format!("wt:{label}");
    if matches!(ctx.color_level, ColorLevel::Mono) {
        Some(text)
    } else {
        Some(format!("\x1b[36m{text}\x1b[0m"))
    }
}
