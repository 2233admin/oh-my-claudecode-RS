use crate::elements::RenderContext;
use crate::terminal::{paint, SemanticColor};

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let tokens = ctx.input.context_window_tokens?;
    let max = ctx.input.context_window_max?;
    if max == 0 {
        return Some(format!("{} ?", ctx.strings.ctx));
    }

    let percent = ((tokens as f64 / max as f64) * 100.0).round().clamp(0.0, 999.0) as u64;
    let filled = ((percent.min(100) as usize * 10) + 5) / 100;
    let bar = format!("{}{}", "▓".repeat(filled), "░".repeat(10 - filled));
    let color = if percent < 50 {
        SemanticColor::Green
    } else if percent <= 80 {
        SemanticColor::Yellow
    } else {
        SemanticColor::Red
    };
    Some(paint(
        ctx.color_level,
        color,
        format!("{} {percent}% {bar}", ctx.strings.ctx),
    ))
}
