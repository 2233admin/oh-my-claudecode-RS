use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let mode = ctx.input.vim_mode()?;
    let label = format!("vim:{mode}");
    if matches!(ctx.color_level, ColorLevel::Mono) {
        Some(label)
    } else {
        let color = match mode {
            "NORMAL" => "\x1b[32m",
            "INSERT" => "\x1b[33m",
            "VISUAL" | "VISUAL LINE" => "\x1b[35m",
            _ => "\x1b[36m",
        };
        Some(format!("{color}{label}\x1b[0m"))
    }
}
