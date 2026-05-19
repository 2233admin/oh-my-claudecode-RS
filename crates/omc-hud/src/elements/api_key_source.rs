use crate::elements::RenderContext;

pub fn render(_ctx: &RenderContext<'_>) -> Option<String> {
    // Check env var first
    if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        return Some("key:env".to_string());
    }

    // Check local settings (~/.claude/settings.local.json)
    if let Some(home) = dirs::home_dir() {
        let local_path = home.join(".claude").join("settings.local.json");
        if has_api_key_in_file(&local_path) {
            return Some("key:local".to_string());
        }

        // Check global settings (~/.claude/settings.json)
        let global_path = home.join(".claude").join("settings.json");
        if has_api_key_in_file(&global_path) {
            return Some("key:global".to_string());
        }
    }

    None
}

fn has_api_key_in_file(path: &std::path::Path) -> bool {
    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let json: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return false,
    };
    json.get("apiKeyHelper").is_some() || json.get("apiKey").is_some()
}
