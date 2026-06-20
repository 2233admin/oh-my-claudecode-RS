use crate::elements::RenderContext;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let board = ctx.mission_board?;

    // Find the first active mission
    let mission = board.missions.iter().find(|m| {
        matches!(
            m.status.as_deref(),
            Some("running") | Some("blocked") | Some("waiting")
        )
    })?;

    let status = mission.status.as_deref().unwrap_or("?");

    let progress = mission
        .task_counts
        .as_ref()
        .map(|tc| format!("{}/{}", tc.completed, tc.total));

    let label = match progress {
        Some(p) => format!("MISSION [{}] {}", status, p),
        None => format!("MISSION [{}]", status),
    };

    Some(label)
}
