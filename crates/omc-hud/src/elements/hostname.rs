use crate::elements::RenderContext;
use crate::terminal::ColorLevel;

pub fn render(ctx: &RenderContext<'_>) -> Option<String> {
    let name = get_hostname()?;
    if name.is_empty() {
        return None;
    }

    let label = format!("host:{name}");

    if matches!(ctx.color_level, ColorLevel::Mono) {
        return Some(label);
    }

    // Cyan — same as JS version
    Some(format!("\x1b[36m{label}\x1b[0m"))
}

fn get_hostname() -> Option<String> {
    // Try COMPUTERNAME (Windows) then HOSTNAME (Unix)
    if let Ok(h) = std::env::var("COMPUTERNAME") {
        if !h.is_empty() {
            return Some(h.to_lowercase());
        }
    }
    if let Ok(h) = std::env::var("HOSTNAME") {
        if !h.is_empty() {
            // Short name only
            return Some(h.split('.').next().unwrap_or(&h).to_string());
        }
    }
    // Fallback: read /etc/hostname on Unix
    if let Ok(h) = std::fs::read_to_string("/etc/hostname") {
        let h = h.trim().to_string();
        if !h.is_empty() {
            return Some(h.split('.').next().unwrap_or(&h).to_string());
        }
    }
    None
}
