use crate::elements::RenderContext;
use crate::terminal::{ColorLevel, SemanticColor, paint};

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let ralph = ctx.omc_state.ralph.as_ref()?;
    if !ralph.active {
        return None;
    }

    let label = format!("ralph:{}/{}", ralph.iteration, ralph.max_iterations);

    if matches!(ctx.color_level, ColorLevel::Mono) {
        return Some(label);
    }

    let color = if ralph.max_iterations == 0 {
        SemanticColor::Green
    } else {
        let pct = ralph.iteration as f64 / ralph.max_iterations as f64;
        if pct >= 0.9 {
            SemanticColor::Red
        } else if pct >= 0.7 {
            SemanticColor::Yellow
        } else {
            SemanticColor::Green
        }
    };

    Some(paint(ctx.color_level, color, &label))
}
