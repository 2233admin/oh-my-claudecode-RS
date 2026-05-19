use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

fn format_duration(ms: u64) -> String {
    let secs = ms / 1_000;
    let mins = secs / 60;
    let hours = mins / 60;
    if hours > 0 {
        let rem_mins = mins % 60;
        if rem_mins == 0 {
            format!("{hours}h")
        } else {
            format!("{hours}h{rem_mins}m")
        }
    } else if mins > 0 {
        format!("{mins}m")
    } else {
        format!("{secs}s")
    }
}

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let dur_ms = ctx.input.session_duration_ms()?;
    if dur_ms == 0 {
        return None;
    }
    let dur = format_duration(dur_ms);

    // Append api duration suffix when it differs meaningfully from total duration
    let api_suffix = ctx
        .input
        .cost
        .as_ref()
        .and_then(|c| c.total_api_duration_ms)
        .filter(|&api_ms| api_ms > 0 && api_ms < dur_ms)
        .map(|api_ms| format!(" (api:{})", format_duration(api_ms)))
        .unwrap_or_default();

    let added = ctx
        .input
        .cost
        .as_ref()
        .and_then(|c| c.total_lines_added)
        .unwrap_or(0);
    let removed = ctx
        .input
        .cost
        .as_ref()
        .and_then(|c| c.total_lines_removed)
        .unwrap_or(0);

    let stats = if added > 0 || removed > 0 {
        format!("{dur}{api_suffix} +{added}/-{removed}")
    } else {
        format!("{dur}{api_suffix}")
    };

    if matches!(ctx.color_level, ColorLevel::Mono) {
        Some(format!("sess:{stats}"))
    } else {
        Some(format!("\x1b[2msess:{stats}\x1b[0m"))
    }
}
