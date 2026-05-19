use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let level = ctx.input.effort.as_ref()?.level.as_deref()?;
    // Only show non-default levels (skip "medium" as it's the baseline)
    let label = match level {
        "low" => "effort:low",
        "high" => "effort:high",
        "xhigh" => "effort:xhigh",
        "max" => "effort:max",
        _ => return None,
    };
    if matches!(ctx.color_level, ColorLevel::Mono) {
        Some(label.to_string())
    } else {
        let color = match level {
            "low" => "\x1b[2m",
            "high" => "\x1b[33m",
            "xhigh" | "max" => "\x1b[31m",
            _ => "",
        };
        Some(format!("{color}{label}\x1b[0m"))
    }
}
