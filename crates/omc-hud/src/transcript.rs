use chrono::{DateTime, Utc};
use std::io::{Read, Seek, SeekFrom};

const MAX_TAIL_BYTES: u64 = 4 * 1024 * 1024;
const THINKING_RECENCY_MS: u64 = 30_000;
const PERMISSION_THRESHOLD_MS: u64 = 3_000;

#[derive(Debug, Default, Clone)]
pub struct TranscriptData {
    pub session_start: Option<DateTime<Utc>>,
    pub tool_call_count: u32,
    pub agent_call_count: u32,
    pub skill_call_count: u32,
    pub last_tool_name: Option<String>,
    pub last_activated_skill: Option<String>,
    pub pending_permission: Option<String>,
    pub thinking_active: bool,
    pub last_request_input_tokens: Option<u64>,
    pub last_request_output_tokens: Option<u64>,
    pub session_total_tokens: Option<u64>,
}

pub fn parse(transcript_path: &str) -> Option<TranscriptData> {
    let mut file = std::fs::File::open(transcript_path).ok()?;
    let file_len = file.metadata().ok()?.len();

    // Seek to tail
    let seek_to = file_len.saturating_sub(MAX_TAIL_BYTES);
    if seek_to > 0 {
        file.seek(SeekFrom::Start(seek_to)).ok()?;
    }

    let mut raw = String::new();
    file.read_to_string(&mut raw).ok()?;

    let mut lines = raw.split('\n');

    // If we seeked into the middle of the file, skip the first (partial) line
    if seek_to > 0 {
        lines.next();
    }

    let now_ms = crate::cache::now_ms();
    let mut data = TranscriptData::default();
    let mut session_total_tokens: u64 = 0;

    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Session start: look for system/init message
        if value.get("type").and_then(|v| v.as_str()) == Some("system") {
            if value.get("subtype").and_then(|v| v.as_str()) == Some("init") {
                if let Some(ts) = extract_timestamp(&value) {
                    // Always overwrite — we want the LAST (most recent) session init
                    data.session_start = Some(ts);
                }
            }
        }

        // Assistant messages — extract tool uses and token usage
        if value.get("type").and_then(|v| v.as_str()) == Some("assistant") {
            // Token usage
            if let Some(usage) = value.get("usage") {
                if let Some(input_tokens) = usage.get("input_tokens").and_then(|v| v.as_u64()) {
                    data.last_request_input_tokens = Some(input_tokens);
                    session_total_tokens = session_total_tokens.saturating_add(input_tokens);
                }
                if let Some(output_tokens) = usage.get("output_tokens").and_then(|v| v.as_u64()) {
                    data.last_request_output_tokens = Some(output_tokens);
                    session_total_tokens = session_total_tokens.saturating_add(output_tokens);
                }
            }

            // Content blocks
            if let Some(content) = value.get("content").and_then(|v| v.as_array()) {
                let msg_ts_ms = extract_timestamp(&value)
                    .map(|dt| dt.timestamp_millis() as u64)
                    .unwrap_or(0);

                for block in content {
                    let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");

                    match block_type {
                        "tool_use" => {
                            let tool_name = block
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();

                            data.tool_call_count += 1;

                            // Classify tool type
                            if is_agent_tool(&tool_name) {
                                data.agent_call_count += 1;
                            } else if is_skill_tool(&tool_name) {
                                data.skill_call_count += 1;
                                if !tool_name.is_empty() {
                                    data.last_activated_skill = Some(tool_name.clone());
                                }
                            }

                            if !tool_name.is_empty() {
                                data.last_tool_name = Some(tool_name.clone());
                            }

                            // Pending permission: tool use within last 3s that hasn't been responded to
                            if msg_ts_ms > 0 && now_ms.saturating_sub(msg_ts_ms) < PERMISSION_THRESHOLD_MS {
                                // We'll mark it as pending — a heuristic
                                data.pending_permission = Some(tool_name);
                            }
                        }
                        "thinking" => {
                            // Thinking block active within last 30s
                            if msg_ts_ms > 0
                                && now_ms.saturating_sub(msg_ts_ms) < THINKING_RECENCY_MS
                            {
                                data.thinking_active = true;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    if session_total_tokens > 0 {
        data.session_total_tokens = Some(session_total_tokens);
    }

    Some(data)
}

fn is_agent_tool(name: &str) -> bool {
    matches!(
        name,
        "Task" | "Agent" | "dispatch_agent" | "mcp__agent__dispatch"
    ) || name.to_ascii_lowercase().contains("agent")
        || name.to_ascii_lowercase().contains("task")
}

fn is_skill_tool(name: &str) -> bool {
    name.starts_with("Skill") || name.starts_with("skill") || name.contains("skill")
}

fn extract_timestamp(value: &serde_json::Value) -> Option<DateTime<Utc>> {
    // Try common timestamp field names
    for key in &["timestamp", "ts", "created_at", "time"] {
        if let Some(ts_val) = value.get(key) {
            if let Some(ts_str) = ts_val.as_str() {
                if let Ok(dt) = ts_str.parse::<DateTime<Utc>>() {
                    return Some(dt);
                }
            }
            // Unix timestamp in seconds
            if let Some(ts_secs) = ts_val.as_i64() {
                use chrono::TimeZone;
                return Utc.timestamp_opt(ts_secs, 0).single();
            }
            // Unix timestamp in ms
            if let Some(ts_ms) = ts_val.as_u64() {
                use chrono::TimeZone;
                let secs = (ts_ms / 1000) as i64;
                let nanos = ((ts_ms % 1000) * 1_000_000) as u32;
                return Utc.timestamp_opt(secs, nanos).single();
            }
        }
    }
    None
}
