use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let transcript = ctx.transcript?;
    let count = transcript.agent_call_count;
    if count == 0 {
        return None;
    }

    // agentsFormat: 'count' (default) | 'codes' | 'multiline'
    // 'multiline' falls back to 'count' — full per-agent detail requires richer transcript data
    let fmt = ctx.config.element_str("agentsFormat", "count");
    // agentsMaxLines: max lines for multiline format (default 3); unused until richer data available
    let _max_lines = ctx.config.element_u64("agentsMaxLines", 3) as usize;

    let label = match fmt {
        "codes" => format!("A:{count}"),
        _ => format!("agents:{count}"),  // 'count' and 'multiline'
    };

    if matches!(ctx.color_level, ColorLevel::Mono) {
        Some(label)
    } else {
        Some(format!("\x1b[2m{label}\x1b[0m"))
    }
}
