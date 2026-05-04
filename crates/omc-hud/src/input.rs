use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
pub struct Input {
    #[serde(default)]
    pub transcript_path: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub context_window_tokens: Option<u64>,
    #[serde(default)]
    pub context_window_max: Option<u64>,
    #[serde(default)]
    pub cost_usd: Option<f64>,
    #[serde(default)]
    pub prompt_start_ms: Option<u64>,
    #[serde(default)]
    pub rate_limit_reset_5h_ms: Option<u64>,
    #[serde(default)]
    pub rate_limit_reset_weekly_ms: Option<u64>,
    #[serde(default)]
    pub turns: Option<u64>,
    #[serde(default)]
    pub hooks_state: Option<serde_json::Value>,
}

pub fn parse_stdin_json(input: &str) -> Input {
    if input.trim().is_empty() {
        return Input::default();
    }

    match serde_json::from_str(input) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("omc-hud: failed to parse stdin JSON: {err}");
            Input::default()
        }
    }
}
