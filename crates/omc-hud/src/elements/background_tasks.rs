use crate::elements::RenderContext;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let hud = ctx.omc_state.hud.as_ref()?;
    let running = hud
        .background_tasks
        .iter()
        .filter(|t| t.status.as_deref() == Some("running"))
        .count();
    if running == 0 {
        return None;
    }
    Some(format!("bg:{running}"))
}
