use crate::elements::RenderContext;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let transcript = ctx.transcript?;

    let tool = transcript.tool_call_count;
    let agent = transcript.agent_call_count;
    let skill = transcript.skill_call_count;

    if tool == 0 && agent == 0 && skill == 0 {
        return None;
    }

    let mut parts: Vec<String> = Vec::new();
    if tool > 0 {
        parts.push(format!("T:{tool}"));
    }
    if agent > 0 {
        parts.push(format!("A:{agent}"));
    }
    if skill > 0 {
        parts.push(format!("S:{skill}"));
    }

    Some(parts.join(" "))
}
