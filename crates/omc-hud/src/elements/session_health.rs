use crate::elements::RenderContext;
use crate::terminal::{ColorLevel, SemanticColor, paint};
use chrono::Utc;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let transcript = ctx.transcript?;
    let session_start = transcript.session_start?;

    let now = Utc::now();
    let elapsed = now.signed_duration_since(session_start);
    let total_mins = elapsed.num_minutes().max(0) as u64;

    let show_duration  = ctx.config.element_enabled("showSessionDuration",  true);
    let show_indicator = ctx.config.element_enabled("showHealthIndicator",   true);

    let label = if show_duration {
        format!("session:{}m", total_mins)
    } else {
        "session".to_string()
    };

    if !show_indicator || matches!(ctx.color_level, ColorLevel::Mono) {
        return Some(label);
    }

    let color = if total_mins < 30 {
        SemanticColor::Green
    } else if total_mins < 60 {
        SemanticColor::Yellow
    } else {
        SemanticColor::Red
    };

    Some(paint(ctx.color_level, color, &label))
}
