use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let label = format!("[OMC#{VERSION}]");
    if matches!(ctx.color_level, ColorLevel::Mono) {
        return Some(label);
    }
    Some(format!("\x1b[1m{label}\x1b[0m"))
}
