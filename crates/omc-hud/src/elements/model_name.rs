use crate::elements::RenderContext;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let model = ctx.input.model.as_deref()?.trim();
    if model.is_empty() {
        None
    } else {
        Some(shorten(model))
    }
}

fn shorten(model: &str) -> String {
    model
        .trim_start_matches("claude-")
        .trim_end_matches("-latest")
        .to_string()
}
