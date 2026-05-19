use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    // Try --worktree session label first, then workspace.git_worktree
    let label = ctx.input.worktree_label()
        .or_else(|| ctx.input.workspace.as_ref()?.git_worktree.as_deref())?;
    let text = format!("wt:{label}");
    if matches!(ctx.color_level, ColorLevel::Mono) {
        Some(text)
    } else {
        Some(format!("\x1b[36m{text}\x1b[0m"))
    }
}
