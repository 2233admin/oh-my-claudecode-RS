use serde::Deserialize;

// ---------------------------------------------------------------------------
// Nested types (JS HUD schema)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize, Clone)]
pub struct ModelInfo {
    pub id: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct CurrentUsage {
    pub input_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct ContextWindow {
    pub context_window_size: Option<u64>,
    pub total_input_tokens: Option<u64>,
    pub used_percentage: Option<f64>,
    pub current_usage: Option<CurrentUsage>,
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct RateLimitBucket {
    pub used_percentage: Option<f64>,
    /// Either a Unix timestamp in ms (number) or an ISO-8601 string.
    pub resets_at: Option<serde_json::Value>,
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct RateLimitsStdin {
    pub five_hour: Option<RateLimitBucket>,
    pub seven_day: Option<RateLimitBucket>,
}

// ---------------------------------------------------------------------------
// Top-level input
// ---------------------------------------------------------------------------

/// Stdin JSON sent by Claude Code to the statusLine command.
///
/// Supports both the nested JS-HUD schema and legacy flat fields so the binary
/// works regardless of which Claude Code version is running.
#[derive(Debug, Default, Deserialize)]
pub struct Input {
    // --- session metadata ---
    pub transcript_path: Option<String>,
    pub cwd: Option<String>,
    pub session_id: Option<String>,
    pub turns: Option<u64>,

    // --- model (nested, JS schema) ---
    pub model: Option<ModelInfo>,

    // --- context window (nested, JS schema) ---
    pub context_window: Option<ContextWindow>,

    // --- rate limits (nested, JS schema) ---
    pub rate_limits: Option<RateLimitsStdin>,

    // --- cost ---
    pub cost_usd: Option<f64>,

    // --- prompt timing ---
    pub prompt_start_ms: Option<u64>,

    // --- OMC orchestration state injected via hooks ---
    pub hooks_state: Option<serde_json::Value>,

    // --- Legacy flat fields (older Claude Code versions) ---
    /// Flat token count — used when `context_window` is absent.
    pub context_window_tokens: Option<u64>,
    /// Flat max tokens — used when `context_window` is absent.
    pub context_window_max: Option<u64>,
}

// ---------------------------------------------------------------------------
// Convenience accessors — prefer nested, fall back to flat
// ---------------------------------------------------------------------------

impl Input {
    /// Tokens used this session (input side).
    pub fn tokens_used(&self) -> Option<u64> {
        self.context_window
            .as_ref()
            .and_then(|cw| cw.total_input_tokens)
            .or(self.context_window_tokens)
    }

    /// Total context window capacity.
    pub fn tokens_max(&self) -> Option<u64> {
        self.context_window
            .as_ref()
            .and_then(|cw| cw.context_window_size)
            .or(self.context_window_max)
    }

    /// Used percentage 0–100 (prefer native field, compute as fallback).
    pub fn context_used_pct(&self) -> Option<f64> {
        if let Some(pct) = self.context_window.as_ref().and_then(|cw| cw.used_percentage) {
            return Some(pct);
        }
        let used = self.tokens_used()? as f64;
        let max = self.tokens_max()? as f64;
        if max == 0.0 { return None; }
        Some((used / max * 100.0).clamp(0.0, 100.0).round())
    }

    /// Model display name (prefer display_name, fall back to id).
    pub fn model_name(&self) -> Option<&str> {
        self.model.as_ref().and_then(|m| {
            m.display_name.as_deref().or(m.id.as_deref())
        })
    }

    /// Model id string.
    pub fn model_id(&self) -> Option<&str> {
        self.model.as_ref().and_then(|m| m.id.as_deref())
    }

    /// Current-turn token counts (for cost calculation).
    pub fn current_input_tokens(&self) -> u64 {
        self.context_window
            .as_ref()
            .and_then(|cw| cw.current_usage.as_ref())
            .and_then(|u| u.input_tokens)
            .unwrap_or(0)
    }

    pub fn current_cache_creation_tokens(&self) -> u64 {
        self.context_window
            .as_ref()
            .and_then(|cw| cw.current_usage.as_ref())
            .and_then(|u| u.cache_creation_input_tokens)
            .unwrap_or(0)
    }

    pub fn current_cache_read_tokens(&self) -> u64 {
        self.context_window
            .as_ref()
            .and_then(|cw| cw.current_usage.as_ref())
            .and_then(|u| u.cache_read_input_tokens)
            .unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

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
