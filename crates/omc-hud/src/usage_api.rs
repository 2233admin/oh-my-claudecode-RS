use chrono::DateTime;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const API_TIMEOUT: Duration = Duration::from_secs(10);
const CACHE_TTL_SUCCESS_MS: u64 = 5_000;
const CACHE_TTL_FAILURE_MS: u64 = 15_000;
const CACHE_TTL_NETWORK_MS: u64 = 120_000;
const MAX_STALE_MS: u64 = 15 * 60 * 1000;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct UsageData {
    pub five_hour_pct: Option<f64>,
    pub five_hour_reset_ms: Option<u64>,
    pub seven_day_pct: Option<f64>,
    pub seven_day_reset_ms: Option<u64>,
    pub seven_day_sonnet_pct: Option<f64>,
    pub seven_day_opus_pct: Option<f64>,
    pub extra_used_usd: Option<f64>,
    pub extra_limit_usd: Option<f64>,
    pub enterprise_used_usd: Option<f64>,
    pub enterprise_limit_usd: Option<f64>,
    pub provider: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    fetched_at_ms: u64,
    ttl_ms: u64,
    data: UsageData,
}

impl CacheEntry {
    #[allow(dead_code)]
    fn is_fresh(&self) -> bool {
        now_ms().saturating_sub(self.fetched_at_ms) < self.ttl_ms
    }
    fn is_beyond_stale(&self) -> bool {
        now_ms().saturating_sub(self.fetched_at_ms) > MAX_STALE_MS
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Provider {
    Anthropic,
    Zai,
    MiniMax,
}

fn detect_provider() -> Provider {
    if std::env::var("MINIMAX_API_KEY").is_ok() {
        return Provider::MiniMax;
    }
    if let Ok(base) = std::env::var("ANTHROPIC_BASE_URL")
        && base.contains("z.ai")
    {
        return Provider::Zai;
    }
    Provider::Anthropic
}

fn provider_source_name(p: Provider) -> &'static str {
    match p {
        Provider::Anthropic => "anthropic",
        Provider::Zai => "zai",
        Provider::MiniMax => "minimax",
    }
}

pub fn fetch(poll_interval_ms: Option<u64>) -> Option<UsageData> {
    let provider = detect_provider();
    let source = provider_source_name(provider);

    let cache_file = cache_path(source);

    // Load existing cache
    let cached: Option<CacheEntry> = std::fs::read_to_string(&cache_file)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok());

    // Override TTL with poll_interval_ms if provided
    if let Some(entry) = &cached {
        let effective_ttl = poll_interval_ms.unwrap_or(entry.ttl_ms);
        let age = now_ms().saturating_sub(entry.fetched_at_ms);
        if age < effective_ttl {
            return Some(entry.data.clone());
        }
    }

    // Attempt API call
    match call_api(provider) {
        Ok(data) => {
            let entry = CacheEntry {
                fetched_at_ms: now_ms(),
                ttl_ms: poll_interval_ms.unwrap_or(CACHE_TTL_SUCCESS_MS),
                data: data.clone(),
            };
            write_cache(&cache_file, &entry);
            Some(data)
        }
        Err(is_network_error) => {
            // Return stale cache if within MAX_STALE_MS
            if let Some(entry) = cached
                && !entry.is_beyond_stale()
            {
                // Update TTL for next poll
                let ttl = if is_network_error {
                    CACHE_TTL_NETWORK_MS
                } else {
                    CACHE_TTL_FAILURE_MS
                };
                let updated = CacheEntry {
                    fetched_at_ms: entry.fetched_at_ms,
                    ttl_ms: ttl,
                    data: entry.data.clone(),
                };
                write_cache(&cache_file, &updated);
                return Some(entry.data);
            }
            None
        }
    }
}

/// Returns Ok(UsageData) on success, Err(true) on network error, Err(false) on API/parse error.
fn call_api(provider: Provider) -> Result<UsageData, bool> {
    match provider {
        Provider::Anthropic => call_anthropic(),
        Provider::Zai => call_zai(),
        Provider::MiniMax => call_minimax(),
    }
}

fn call_anthropic() -> Result<UsageData, bool> {
    let token = load_credentials().ok_or(false)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(API_TIMEOUT)
        .build()
        .map_err(|_| true)?;

    let resp = client
        .get("https://api.anthropic.com/api/oauth/usage")
        .header("Authorization", format!("Bearer {token}"))
        .header("anthropic-beta", "oauth-2025-04-20")
        .send()
        .map_err(|_| true)?;

    if !resp.status().is_success() {
        return Err(false);
    }

    let json: serde_json::Value = resp.json().map_err(|_| false)?;
    parse_anthropic_response(&json).ok_or(false)
}

/// Parse a `resets_at` field that may be:
///   - Unix seconds integer (< 1e11)
///   - Unix milliseconds integer (>= 1e11)
///   - Floating-point seconds
///   - ISO-8601 string ("2026-05-18T16:00:00Z")
fn parse_reset_ms(v: &serde_json::Value) -> Option<u64> {
    // Integer
    if let Some(n) = v.as_u64() {
        return Some(if n < 100_000_000_000 { n * 1000 } else { n });
    }
    // Float (seconds)
    if let Some(f) = v.as_f64() {
        let n = f as u64;
        return Some(if n < 100_000_000_000 { n * 1000 } else { n });
    }
    // ISO-8601 string
    if let Some(s) = v.as_str()
        && let Ok(dt) = DateTime::parse_from_rfc3339(s)
    {
        let ms = dt.timestamp_millis();
        if ms > 0 {
            return Some(ms as u64);
        }
    }
    None
}

fn parse_anthropic_response(json: &serde_json::Value) -> Option<UsageData> {
    let mut data = UsageData {
        provider: Some("anthropic".to_string()),
        ..Default::default()
    };

    if let Some(five) = json.get("five_hour") {
        // API returns utilization as 0-100 directly (not 0-1)
        data.five_hour_pct = five.get("utilization").and_then(|v| v.as_f64());
        data.five_hour_reset_ms = five.get("resets_at").and_then(parse_reset_ms);
    }

    if let Some(seven) = json.get("seven_day") {
        data.seven_day_pct = seven.get("utilization").and_then(|v| v.as_f64());
        data.seven_day_reset_ms = seven.get("resets_at").and_then(parse_reset_ms);
    }

    if let Some(sonnet) = json.get("seven_day_sonnet") {
        data.seven_day_sonnet_pct = sonnet.get("utilization").and_then(|v| v.as_f64());
    }

    if let Some(opus) = json.get("seven_day_opus") {
        data.seven_day_opus_pct = opus.get("utilization").and_then(|v| v.as_f64());
    }

    if let Some(extra) = json.get("extra_usage") {
        // used_credits / monthly_limit are in cents (minor units) — divide by 100 for USD
        data.extra_used_usd = extra
            .get("used_credits")
            .and_then(|v| v.as_f64())
            .map(|v| v / 100.0);
        data.extra_limit_usd = extra
            .get("monthly_limit")
            .and_then(|v| v.as_f64())
            .map(|v| v / 100.0);
    }

    if let Some(enterprise) = json.get("enterprise_billing") {
        data.enterprise_used_usd = enterprise.get("used_usd").and_then(|v| v.as_f64());
        data.enterprise_limit_usd = enterprise.get("limit_usd").and_then(|v| v.as_f64());
    }

    Some(data)
}

fn call_zai() -> Result<UsageData, bool> {
    let base = std::env::var("ANTHROPIC_BASE_URL").map_err(|_| false)?;
    let token = std::env::var("ANTHROPIC_AUTH_TOKEN").map_err(|_| false)?;

    // SSRF guard: validate z.ai domain
    validate_zai_url(&base).map_err(|_| false)?;

    let url = format!(
        "{}/api/monitor/usage/quota/limit",
        base.trim_end_matches('/')
    );

    let client = reqwest::blocking::Client::builder()
        .timeout(API_TIMEOUT)
        .build()
        .map_err(|_| true)?;

    let resp = client
        .get(&url)
        .header("Authorization", token)
        .send()
        .map_err(|_| true)?;

    if !resp.status().is_success() {
        return Err(false);
    }

    let json: serde_json::Value = resp.json().map_err(|_| false)?;
    let mut data = UsageData {
        provider: Some("zai".to_string()),
        ..Default::default()
    };

    // Parse z.ai response (best-effort, shape may vary)
    data.five_hour_pct = json
        .get("five_hour_utilization")
        .or_else(|| json.get("utilization"))
        .and_then(|v| v.as_f64())
        .map(|v| v * 100.0);

    Ok(data)
}

fn call_minimax() -> Result<UsageData, bool> {
    let base = std::env::var("ANTHROPIC_BASE_URL").map_err(|_| false)?;
    let api_key = std::env::var("MINIMAX_API_KEY")
        .or_else(|_| std::env::var("ANTHROPIC_AUTH_TOKEN"))
        .map_err(|_| false)?;

    // SSRF guard: validate minimax domain
    validate_minimax_url(&base).map_err(|_| false)?;

    let url = format!(
        "{}/v1/api/openplatform/coding_plan/remains",
        base.trim_end_matches('/')
    );

    let client = reqwest::blocking::Client::builder()
        .timeout(API_TIMEOUT)
        .build()
        .map_err(|_| true)?;

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .send()
        .map_err(|_| true)?;

    if !resp.status().is_success() {
        return Err(false);
    }

    let json: serde_json::Value = resp.json().map_err(|_| false)?;
    let mut data = UsageData {
        provider: Some("minimax".to_string()),
        ..Default::default()
    };

    data.extra_used_usd = json.get("used").and_then(|v| v.as_f64());
    data.extra_limit_usd = json.get("limit").and_then(|v| v.as_f64());

    Ok(data)
}

/// Validate that a URL belongs to z.ai — allowlist approach.
fn validate_zai_url(url: &str) -> Result<(), ()> {
    let parsed = url.parse::<reqwest::Url>().map_err(|_| ())?;
    if parsed.scheme() != "https" {
        return Err(());
    }
    let host = parsed.host_str().unwrap_or("");
    if host == "z.ai" || host.ends_with(".z.ai") {
        return Ok(());
    }
    Err(())
}

/// Validate that a URL belongs to minimax domains.
fn validate_minimax_url(url: &str) -> Result<(), ()> {
    let parsed = url.parse::<reqwest::Url>().map_err(|_| ())?;
    if parsed.scheme() != "https" {
        return Err(());
    }
    let host = parsed.host_str().unwrap_or("");
    // Allowlist: minimaxi.com and api.minimax.chat
    if host == "api.minimaxi.com"
        || host.ends_with(".minimaxi.com")
        || host == "api.minimax.chat"
        || host.ends_with(".minimax.chat")
    {
        return Ok(());
    }
    Err(())
}

#[derive(Debug, Default, Deserialize)]
struct Credentials {
    #[serde(rename = "accessToken")]
    pub access_token: Option<String>,
    #[serde(rename = "expiresAt")]
    pub expires_at: Option<u64>,
    #[serde(rename = "claudeAiOauth")]
    pub claude_ai_oauth: Option<Box<Credentials>>,
}

fn load_credentials() -> Option<String> {
    // Priority 1: Check ANTHROPIC_API_KEY env var
    if let Ok(token) = std::env::var("ANTHROPIC_API_KEY")
        && !token.is_empty()
    {
        return Some(token);
    }

    // Priority 2: Try reading ~/.claude/.credentials.json
    let home = dirs::home_dir()?;
    let creds_path = home.join(".claude").join(".credentials.json");
    let raw = std::fs::read_to_string(&creds_path).ok()?;
    let creds: Credentials = serde_json::from_str(&raw).ok()?;

    // Support nested claudeAiOauth format
    let effective = if let Some(nested) = &creds.claude_ai_oauth {
        nested.as_ref()
    } else {
        &creds
    };

    // Check expiry
    if let Some(expires_at) = effective.expires_at
        && expires_at < now_ms()
    {
        // File exists but expired, try macOS Keychain before returning None
        #[cfg(target_os = "macos")]
        return read_macos_keychain();
        #[cfg(not(target_os = "macos"))]
        return None;
    }

    // Return token if valid
    if let Some(token) = effective.access_token.clone() {
        return Some(token);
    }

    // Priority 3: Try macOS Keychain
    #[cfg(target_os = "macos")]
    {
        read_macos_keychain()
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// Read OAuth token from macOS Keychain (Claude Code stores tokens under specific service names)
#[cfg(target_os = "macos")]
fn read_macos_keychain() -> Option<String> {
    for service in &["claude.ai", "Claude"] {
        let output = std::process::Command::new("security")
            .args(["find-generic-password", "-s", service, "-w"])
            .output()
            .ok()?;
        if output.status.success() {
            let token = String::from_utf8(output.stdout).ok()?.trim().to_string();
            if !token.is_empty() {
                return Some(token);
            }
        }
    }
    None
}

fn cache_path(source: &str) -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".claude")
        .join("plugins")
        .join("oh-my-claudecode")
        .join(format!(".usage-cache-{source}.json"))
}

fn write_cache(path: &std::path::Path, entry: &CacheEntry) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec(entry) {
        let _ = std::fs::write(path, bytes);
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
