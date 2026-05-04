use crate::elements::RenderContext;
use crate::terminal::{paint, SemanticColor};

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let cost = ctx.input.cost_usd?;
    let color = if cost < 0.10 {
        SemanticColor::Green
    } else if cost < 0.50 {
        SemanticColor::Yellow
    } else {
        SemanticColor::Red
    };
    Some(paint(ctx.color_level, color, format!("💰 ${cost:.3}")))
}
