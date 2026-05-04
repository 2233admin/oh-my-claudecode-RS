use crate::cache::now_ms;
use crate::elements::RenderContext;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let now = now_ms();
    let reset = [ctx.input.rate_limit_reset_5h_ms, ctx.input.rate_limit_reset_weekly_ms]
        .into_iter()
        .flatten()
        .filter(|reset| *reset > now)
        .min()?;
    Some(format!(
        "⚡ {} {}",
        ctx.strings.rl,
        format_duration(reset.saturating_sub(now))
    ))
}

fn format_duration(ms: u64) -> String {
    let mins = ms / 1000 / 60;
    if mins < 60 {
        format!("{mins}m")
    } else {
        format!("{}h", mins / 60)
    }
}
