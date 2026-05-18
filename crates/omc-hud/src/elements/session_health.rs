use crate::elements::RenderContext;
use crate::terminal::{ColorLevel, SemanticColor, paint};
use chrono::Utc;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let transcript = ctx.transcript?;
    let session_start = transcript.session_start?;

    let now = Utc::now();
    let elapsed = now.signed_duration_since(session_start);
    let total_mins = elapsed.num_minutes().max(0) as u64;

    let label = format!("session:{}m", total_mins);

    let color = if total_mins < 30 {
        SemanticColor::Green
    } else if total_mins < 60 {
        SemanticColor::Yellow
    } else {
        SemanticColor::Red
    };

    if matches!(ctx.color_level, ColorLevel::Mono) {
        Some(label)
    } else {
        Some(paint(ctx.color_level, color, &label))
    }
}
