use crate::elements::RenderContext;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let tokens = ctx.input.context_window_tokens?;
    let max = ctx.input.context_window_max?;
    Some(format!(
        "{} {}/{}",
        ctx.strings.tok,
        compact(tokens),
        compact(max)
    ))
}

fn compact(value: u64) -> String {
    if value >= 1000 {
        format!("{}k", value / 1000)
    } else {
        value.to_string()
    }
}
