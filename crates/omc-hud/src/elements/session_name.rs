use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let name = ctx.input.session_name.as_deref()?;
    if name.is_empty() {
        return None;
    }
    if matches!(ctx.color_level, ColorLevel::Mono) {
        Some(format!("#{name}"))
    } else {
        Some(format!("\x1b[2m#{name}\x1b[0m"))
    }
}
