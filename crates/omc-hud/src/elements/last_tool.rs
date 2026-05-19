use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let tool_name = ctx.transcript?.last_tool_name.as_deref()?;
    if tool_name.is_empty() {
        return None;
    }

    let label = format!("tool:{tool_name}");

    if matches!(ctx.color_level, ColorLevel::Mono) {
        return Some(label);
    }

    // Dim label, same style as JS version
    Some(format!("\x1b[2m{label}\x1b[0m"))
}
