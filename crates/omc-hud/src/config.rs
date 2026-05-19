use serde::Deserialize;
use std::collections::HashMap;
use serde_json::Value;

// ---------------------------------------------------------------------------
// Thresholds
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct HudThresholds {
    pub context_warning: f64,
    pub context_compact: f64,
    pub context_critical: f64,
    pub ralph_warning: f64,
}

impl Default for HudThresholds {
    fn default() -> Self {
        Self {
            context_warning: 70.0,
            context_compact: 80.0,
            context_critical: 85.0,
            ralph_warning: 7.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Preset tables (mirrors JS PRESET_CONFIGS)
// ---------------------------------------------------------------------------

/// Returns the elements map for a named preset.
/// Values are serde_json::Value (bool or string) matching JS PRESET_CONFIGS.
fn preset_elements(name: &str) -> HashMap<String, Value> {
    let mut m = HashMap::new();
    macro_rules! b { ($k:expr, $v:expr) => { m.insert($k.to_string(), Value::Bool($v)); }; }
    macro_rules! s { ($k:expr, $v:expr) => { m.insert($k.to_string(), Value::String($v.to_string())); }; }
    macro_rules! n { ($k:expr, $v:expr) => { m.insert($k.to_string(), Value::Number(serde_json::Number::from($v))); }; }

    match name {
        "minimal" => {
            b!("cwd", false); s!("cwdFormat", "folder");
            b!("gitRepo", false); b!("gitBranch", false); b!("gitStatus", false); s!("gitInfoPosition", "above");
            b!("model", false); s!("modelFormat", "short");
            b!("omcLabel", true); b!("rateLimits", true); b!("ralph", true); b!("autopilot", true);
            b!("prdStory", false); b!("activeSkills", true); b!("lastSkill", true);
            b!("contextBar", false);
            b!("agents", true); s!("agentsFormat", "count"); n!("agentsMaxLines", 0);
            b!("backgroundTasks", false); b!("todos", true); b!("permissionStatus", false);
            b!("thinking", false); s!("thinkingFormat", "text");
            b!("apiKeySource", false); b!("hostname", false); b!("profile", true);
            b!("missionBoard", false); b!("promptTime", false);
            b!("sessionHealth", false); b!("showSessionDuration", true); b!("showHealthIndicator", true);
            b!("showTokens", false); b!("useBars", false); b!("showCallCounts", false);
            b!("showLastTool", false); b!("sessionSummary", false);
            n!("maxOutputLines", 2); b!("safeMode", true);
        }
        "focused" => {
            b!("cwd", false); s!("cwdFormat", "relative");
            b!("gitRepo", false); b!("gitBranch", true); b!("gitStatus", true); s!("gitInfoPosition", "above");
            b!("model", false); s!("modelFormat", "short");
            b!("omcLabel", true); b!("rateLimits", true); b!("ralph", true); b!("autopilot", true);
            b!("prdStory", true); b!("activeSkills", true); b!("lastSkill", true);
            b!("contextBar", true);
            b!("agents", true); s!("agentsFormat", "multiline"); n!("agentsMaxLines", 3);
            b!("backgroundTasks", true); b!("todos", true); b!("permissionStatus", false);
            b!("thinking", true); s!("thinkingFormat", "text");
            b!("apiKeySource", false); b!("hostname", false); b!("profile", true);
            b!("missionBoard", false); b!("promptTime", true);
            b!("sessionHealth", true); b!("showSessionDuration", true); b!("showHealthIndicator", true);
            b!("showTokens", false); b!("useBars", true); b!("showCallCounts", true);
            b!("showLastTool", false); b!("sessionSummary", false);
            n!("maxOutputLines", 4); b!("safeMode", true);
        }
        "full" => {
            b!("cwd", false); s!("cwdFormat", "relative");
            b!("gitRepo", true); b!("gitBranch", true); b!("gitStatus", true); s!("gitInfoPosition", "above");
            b!("model", false); s!("modelFormat", "short");
            b!("omcLabel", true); b!("rateLimits", true); b!("ralph", true); b!("autopilot", true);
            b!("prdStory", true); b!("activeSkills", true); b!("lastSkill", true);
            b!("contextBar", true);
            b!("agents", true); s!("agentsFormat", "multiline"); n!("agentsMaxLines", 10);
            b!("backgroundTasks", true); b!("todos", true); b!("permissionStatus", false);
            b!("thinking", true); s!("thinkingFormat", "text");
            b!("apiKeySource", true); b!("hostname", false); b!("profile", true);
            b!("missionBoard", false); b!("promptTime", true);
            b!("sessionHealth", true); b!("showSessionDuration", true); b!("showHealthIndicator", true);
            b!("showTokens", false); b!("useBars", true); b!("showCallCounts", true);
            b!("showLastTool", false); b!("sessionSummary", false);
            n!("maxOutputLines", 12); b!("safeMode", true);
        }
        "opencode" => {
            b!("cwd", false); s!("cwdFormat", "relative");
            b!("gitRepo", false); b!("gitBranch", true); b!("gitStatus", false); s!("gitInfoPosition", "above");
            b!("model", false); s!("modelFormat", "short");
            b!("omcLabel", true); b!("rateLimits", false); b!("ralph", true); b!("autopilot", true);
            b!("prdStory", false); b!("activeSkills", true); b!("lastSkill", true);
            b!("contextBar", true);
            b!("agents", true); s!("agentsFormat", "codes"); n!("agentsMaxLines", 0);
            b!("backgroundTasks", false); b!("todos", true); b!("permissionStatus", false);
            b!("thinking", true); s!("thinkingFormat", "text");
            b!("apiKeySource", false); b!("hostname", false); b!("profile", true);
            b!("missionBoard", false); b!("promptTime", true);
            b!("sessionHealth", true); b!("showSessionDuration", true); b!("showHealthIndicator", true);
            b!("showTokens", false); b!("useBars", false); b!("showCallCounts", true);
            b!("showLastTool", false); b!("sessionSummary", false);
            n!("maxOutputLines", 4); b!("safeMode", true);
        }
        "dense" => {
            b!("cwd", false); s!("cwdFormat", "relative");
            b!("gitRepo", true); b!("gitBranch", true); b!("gitStatus", true); s!("gitInfoPosition", "above");
            b!("model", false); s!("modelFormat", "short");
            b!("omcLabel", true); b!("rateLimits", true); b!("ralph", true); b!("autopilot", true);
            b!("prdStory", true); b!("activeSkills", true); b!("lastSkill", true);
            b!("contextBar", true);
            b!("agents", true); s!("agentsFormat", "multiline"); n!("agentsMaxLines", 5);
            b!("backgroundTasks", true); b!("todos", true); b!("permissionStatus", false);
            b!("thinking", true); s!("thinkingFormat", "text");
            b!("apiKeySource", true); b!("hostname", false); b!("profile", true);
            b!("missionBoard", false); b!("promptTime", true);
            b!("sessionHealth", true); b!("showSessionDuration", true); b!("showHealthIndicator", true);
            b!("showTokens", false); b!("useBars", true); b!("showCallCounts", true);
            b!("showLastTool", false); b!("sessionSummary", false);
            n!("maxOutputLines", 6); b!("safeMode", true);
        }
        // "default" (no preset)
        _ => {
            b!("cwd", false); s!("cwdFormat", "relative");
            b!("gitRepo", false); b!("gitBranch", false); b!("gitStatus", false);
            b!("model", false); s!("modelFormat", "short");
            b!("omcLabel", true); b!("rateLimits", true); b!("ralph", true); b!("autopilot", true);
            b!("prdStory", true); b!("activeSkills", true); b!("lastSkill", true);
            b!("contextBar", true);
            b!("agents", true); s!("agentsFormat", "multiline"); n!("agentsMaxLines", 5);
            b!("backgroundTasks", true); b!("todos", true); b!("permissionStatus", false);
            b!("thinking", true); s!("thinkingFormat", "text");
            b!("apiKeySource", false); b!("hostname", false); b!("profile", true);
            b!("missionBoard", false); b!("promptTime", true);
            b!("sessionHealth", true); b!("showSessionDuration", true); b!("showHealthIndicator", true);
            b!("showTokens", false); b!("useBars", true); b!("showCallCounts", true);
            b!("showLastTool", false); b!("sessionSummary", false);
            n!("maxOutputLines", 4); b!("safeMode", true);
        }
    }
    m
}

// ---------------------------------------------------------------------------
// HudConfig — resolved (preset merged with user overrides)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct HudConfig {
    pub preset: Option<String>,
    pub locale: Option<String>,
    /// Merged: preset defaults + user element overrides. Values are bool or String.
    pub elements: HashMap<String, Value>,
    pub thresholds: HudThresholds,
    pub usage_api_poll_interval_ms: Option<u64>,
    pub element_order: Option<Vec<String>>,
    pub max_width: Option<u32>,
}

impl Default for HudConfig {
    fn default() -> Self {
        Self {
            preset: None,
            locale: None,
            elements: preset_elements("default"),
            thresholds: HudThresholds::default(),
            usage_api_poll_interval_ms: None,
            element_order: None,
            max_width: None,
        }
    }
}

impl HudConfig {
    /// Whether a named element is enabled. Falls back to `default` if not configured.
    pub fn element_enabled(&self, name: &str, default: bool) -> bool {
        match self.elements.get(name) {
            Some(Value::Bool(b)) => *b,
            Some(_) => default,
            None => default,
        }
    }

    /// String format option for an element. Falls back to `default`.
    pub fn element_str<'a>(&'a self, name: &'a str, default: &'a str) -> &'a str {
        match self.elements.get(name) {
            Some(Value::String(s)) => s.as_str(),
            _ => default,
        }
    }

    /// Numeric option. Falls back to `default`.
    pub fn element_u64(&self, name: &str, default: u64) -> u64 {
        match self.elements.get(name) {
            Some(Value::Number(n)) => n.as_u64().unwrap_or(default),
            _ => default,
        }
    }

    /// Max output lines from elements config.
    pub fn max_output_lines(&self) -> usize {
        self.element_u64("maxOutputLines", 4) as usize
    }
}

// ---------------------------------------------------------------------------
// Raw deserialization shape (settings.json omcHud)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
struct RawThresholds {
    #[serde(rename = "contextWarning")]
    context_warning: Option<f64>,
    #[serde(rename = "contextCompactSuggestion")]
    context_compact: Option<f64>,
    #[serde(rename = "contextCritical")]
    context_critical: Option<f64>,
    #[serde(rename = "ralphWarning")]
    ralph_warning: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
struct RawHudConfig {
    preset: Option<String>,
    locale: Option<String>,
    /// User overrides — can contain bool or string values.
    elements: Option<HashMap<String, Value>>,
    thresholds: Option<RawThresholds>,
    #[serde(rename = "usageApiPollIntervalMs")]
    usage_api_poll_interval_ms: Option<u64>,
    #[serde(rename = "elementOrder")]
    element_order: Option<Vec<String>>,
    #[serde(rename = "maxWidth")]
    max_width: Option<u32>,
}

// ---------------------------------------------------------------------------
// Load from settings.json
// ---------------------------------------------------------------------------

pub fn load() -> HudConfig {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return HudConfig::default(),
    };
    let raw = match std::fs::read_to_string(home.join(".claude").join("settings.json")) {
        Ok(s) => s,
        Err(_) => return HudConfig::default(),
    };
    let json: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return HudConfig::default(),
    };
    let omc_hud = match json.get("omcHud") {
        Some(v) => v.clone(),
        None => return HudConfig::default(),
    };
    let raw_cfg: RawHudConfig = serde_json::from_value(omc_hud).unwrap_or_default();
    resolve(raw_cfg)
}

fn resolve(raw: RawHudConfig) -> HudConfig {
    // Start with preset defaults (or "default" if no preset specified)
    let preset_name = raw.preset.as_deref().unwrap_or("default");
    let mut elements = preset_elements(preset_name);

    // Merge user overrides on top
    if let Some(user_elements) = raw.elements {
        for (k, v) in user_elements {
            elements.insert(k, v);
        }
    }

    let defaults = HudThresholds::default();
    let thresholds = match raw.thresholds {
        Some(t) => HudThresholds {
            context_warning: t.context_warning.unwrap_or(defaults.context_warning),
            context_compact: t.context_compact.unwrap_or(defaults.context_compact),
            context_critical: t.context_critical.unwrap_or(defaults.context_critical),
            ralph_warning: t.ralph_warning.unwrap_or(defaults.ralph_warning),
        },
        None => defaults,
    };

    HudConfig {
        preset: raw.preset,
        locale: raw.locale,
        elements,
        thresholds,
        usage_api_poll_interval_ms: raw.usage_api_poll_interval_ms,
        element_order: raw.element_order,
        max_width: raw.max_width,
    }
}
