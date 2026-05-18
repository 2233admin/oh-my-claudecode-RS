use crate::elements::RenderContext;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let transcript = ctx.transcript?;
    if transcript.agent_call_count == 0 {
        return None;
    }
    Some(format!("agents:{}", transcript.agent_call_count))
}
