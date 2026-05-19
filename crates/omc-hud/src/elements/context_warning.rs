use crate::elements::RenderContext;
use crate::terminal::{ColorLevel, SemanticColor, paint};

const WARN_THRESHOLD: f64 = 80.0;
const CRITICAL_THRESHOLD: f64 = 90.0;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let pct = ctx.input.context_used_pct()?;

    if pct < WARN_THRESHOLD {
        return None;
    }

    let threshold = if pct >= CRITICAL_THRESHOLD {
        CRITICAL_THRESHOLD
    } else {
        WARN_THRESHOLD
    };

    let label = format!("[!!] ctx {:.0}% >= {:.0}% - run /compact", pct, threshold);

    if matches!(ctx.color_level, ColorLevel::Mono) {
        return Some(label);
    }

    let color = if pct >= CRITICAL_THRESHOLD {
        SemanticColor::Red
    } else {
        SemanticColor::Yellow
    };

    Some(paint(ctx.color_level, color, &label))
}
