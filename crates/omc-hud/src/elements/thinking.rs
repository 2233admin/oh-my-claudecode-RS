use crate::cache::now_ms;
use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

/// Braille spinner — 10 frames, ~80ms per frame → ~8 Hz.
const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

fn spinner_frame() -> char {
    let idx = (now_ms() / 80) as usize % SPINNER.len();
    SPINNER[idx]
}

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let active =
        ctx.input.thinking_enabled() || ctx.transcript.map(|t| t.thinking_active).unwrap_or(false);
    if !active {
        return None;
    }

    // thinkingFormat: 'text' (default) = spinner + "thinking"; 'icon' = spinner only
    let fmt = ctx.config.element_str("thinkingFormat", "text");
    let frame = spinner_frame();
    let text = if fmt == "icon" {
        format!("{frame}")
    } else {
        format!("{frame}thinking")
    };

    if matches!(ctx.color_level, ColorLevel::Mono) {
        Some(text)
    } else {
        Some(format!("\x1b[36m{text}\x1b[0m"))
    }
}
