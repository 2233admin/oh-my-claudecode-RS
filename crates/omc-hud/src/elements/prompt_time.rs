use crate::cache::now_ms;
use crate::elements::RenderContext;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let start = ctx.input.prompt_start_ms?;
    let elapsed = now_ms().saturating_sub(start);
    Some(format!("⏱ {}", format_duration(elapsed)))
}

fn format_duration(ms: u64) -> String {
    let secs = ms / 1000;
    let mins = secs / 60;
    let rem = secs % 60;
    if mins == 0 {
        format!("{rem}s")
    } else {
        format!("{mins}m{rem:02}s")
    }
}
