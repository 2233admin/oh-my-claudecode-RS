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
    let transcript = ctx.transcript?;
    if !transcript.thinking_active {
        return None;
    }

    let frame = spinner_frame();
    if matches!(ctx.color_level, ColorLevel::Mono) {
        Some(format!("{frame}thinking"))
    } else {
        Some(format!("\x1b[36m{frame}thinking\x1b[0m"))
    }
}
