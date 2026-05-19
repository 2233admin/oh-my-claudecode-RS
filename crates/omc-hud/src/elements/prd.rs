use crate::elements::RenderContext;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let prd = ctx.omc_state.prd.as_ref()?;
    if prd.total == 0 {
        return None;
    }
    let story = prd.current_story_id.as_deref().unwrap_or("?");
    Some(format!("{} ({}/{})", story, prd.completed, prd.total))
}
