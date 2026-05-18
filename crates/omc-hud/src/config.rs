use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Default, Deserialize, Clone)]
pub struct HudThresholds {
    #[serde(rename = "contextWarning", default)]
    pub context_warning: Option<f64>,
    #[serde(rename = "contextCompactSuggestion", default)]
    pub context_compact: Option<f64>,
    #[serde(rename = "contextCritical", default)]
    pub context_critical: Option<f64>,
    #[serde(rename = "ralphWarning", default)]
    pub ralph_warning: Option<f64>,
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct HudConfig {
    pub preset: Option<String>,
    pub locale: Option<String>,
    pub elements: Option<HashMap<String, bool>>,
    pub thresholds: Option<HudThresholds>,
    #[serde(rename = "usageApiPollIntervalMs")]
    pub usage_api_poll_interval_ms: Option<u64>,
    #[serde(rename = "elementOrder")]
    pub element_order: Option<Vec<String>>,
    #[serde(rename = "maxWidth")]
    pub max_width: Option<u32>,
}

pub fn load() -> HudConfig {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return HudConfig::default(),
    };
    let settings_path = home.join(".claude").join("settings.json");
    let raw = match std::fs::read_to_string(&settings_path) {
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
    serde_json::from_value(omc_hud).unwrap_or_default()
}
