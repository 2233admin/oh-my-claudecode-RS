use crate::elements::RenderContext;
use crate::terminal::{ColorLevel, SemanticColor, paint};

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let pct = ctx.input.context_used_pct()?;
    let warn = ctx.config.thresholds.context_compact;
    let critical = ctx.config.thresholds.context_critical;

    if pct < warn {
        return None;
    }

    let threshold = if pct >= critical { critical } else { warn };
    let label = format!("[!!] ctx {:.0}% >= {:.0}% - run /compact", pct, threshold);

    if matches!(ctx.color_level, ColorLevel::Mono) {
        return Some(label);
    }

    let color = if pct >= critical {
        SemanticColor::Red
    } else {
        SemanticColor::Yellow
    };
    Some(paint(ctx.color_level, color, &label))
}
