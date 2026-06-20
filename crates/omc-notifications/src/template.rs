//! Template interpolation engine.
//!
//! Lightweight `{{variable}}` interpolation with `{{#if var}}...{{/if}}` conditionals.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::types::NotificationPayload;

/// Known template variables for validation.
static KNOWN_VARIABLES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        "event",
        "sessionId",
        "message",
        "timestamp",
        "tmuxSession",
        "projectPath",
        "projectName",
        "modesUsed",
        "contextSummary",
        "durationMs",
        "agentsSpawned",
        "agentsCompleted",
        "reason",
        "activeMode",
        "iteration",
        "maxIterations",
        "question",
        "incompleteTasks",
        "agentName",
        "agentType",
        "tmuxTail",
        "tmuxPaneId",
        // Computed
        "duration",
        "time",
        "modesDisplay",
        "iterationDisplay",
        "agentDisplay",
        "projectDisplay",
        "footer",
        "tmuxTailBlock",
        "reasonDisplay",
    ])
});

/// Regex for `{{#if var}}...{{/if}}` conditionals.
static CONDITIONAL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{#if\s+(\w+)\}\}([\s\S]*?)\{\{/if\}\}").unwrap());

/// Regex for `{{variable}}` placeholders. Matches `{{word}}` —
/// `{{#if ...}}` and `{{/if}}` are excluded by the filter in the callback.
static VARIABLE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{\{(\w+)\}\}").unwrap());

/// Format duration from milliseconds to human-readable string.
fn format_duration(ms: Option<u64>) -> String {
    let Some(ms) = ms else {
        return "unknown".into();
    };
    let seconds = ms / 1000;
    let minutes = seconds / 60;
    let hours = minutes / 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes % 60, seconds % 60)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds % 60)
    } else {
        format!("{seconds}s")
    }
}

/// Get project display name from payload.
fn project_display(payload: &NotificationPayload) -> String {
    if let Some(name) = &payload.project_name {
        return name.clone();
    }
    if let Some(path) = &payload.project_path {
        return std::path::Path::new(path)
            .file_name()
            .map_or_else(|| "unknown".into(), |n| n.to_string_lossy().into_owned());
    }
    "unknown".into()
}

/// Build common footer with tmux and project info.
fn build_footer(payload: &NotificationPayload) -> String {
    let mut parts = Vec::new();
    if let Some(session) = &payload.tmux_session {
        parts.push(format!("**tmux:** `{session}`"));
    }
    parts.push(format!("**project:** `{}`", project_display(payload)));
    parts.join(" | ")
}

/// Build tmux tail block with code fence, or empty string.
fn build_tmux_tail_block(payload: &NotificationPayload) -> String {
    let Some(tail) = &payload.tmux_tail else {
        return String::new();
    };
    let trimmed = tail.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    format!("\n\n**Recent output:**\n```\n{trimmed}\n```")
}

/// Build the full variable map from a notification payload.
pub fn compute_variables(
    payload: &NotificationPayload,
) -> std::collections::HashMap<String, String> {
    let mut vars = std::collections::HashMap::new();

    // Raw payload fields
    vars.insert("event".into(), payload.event.to_string());
    vars.insert("sessionId".into(), payload.session_id.clone());
    vars.insert("message".into(), payload.message.clone());
    vars.insert("timestamp".into(), payload.timestamp.clone());
    vars.insert(
        "tmuxSession".into(),
        payload.tmux_session.clone().unwrap_or_default(),
    );
    vars.insert(
        "projectPath".into(),
        payload.project_path.clone().unwrap_or_default(),
    );
    vars.insert(
        "projectName".into(),
        payload.project_name.clone().unwrap_or_default(),
    );
    vars.insert(
        "modesUsed".into(),
        payload
            .modes_used
            .as_ref()
            .map(|v| v.join(", "))
            .unwrap_or_default(),
    );
    vars.insert(
        "contextSummary".into(),
        payload.context_summary.clone().unwrap_or_default(),
    );
    vars.insert(
        "durationMs".into(),
        payload
            .duration_ms
            .map(|d| d.to_string())
            .unwrap_or_default(),
    );
    vars.insert(
        "agentsSpawned".into(),
        payload
            .agents_spawned
            .map(|d| d.to_string())
            .unwrap_or_default(),
    );
    vars.insert(
        "agentsCompleted".into(),
        payload
            .agents_completed
            .map(|d| d.to_string())
            .unwrap_or_default(),
    );
    vars.insert("reason".into(), payload.reason.clone().unwrap_or_default());
    vars.insert(
        "activeMode".into(),
        payload.active_mode.clone().unwrap_or_default(),
    );
    vars.insert(
        "iteration".into(),
        payload.iteration.map(|d| d.to_string()).unwrap_or_default(),
    );
    vars.insert(
        "maxIterations".into(),
        payload
            .max_iterations
            .map(|d| d.to_string())
            .unwrap_or_default(),
    );
    vars.insert(
        "question".into(),
        payload.question.clone().unwrap_or_default(),
    );
    vars.insert(
        "incompleteTasks".into(),
        payload
            .incomplete_tasks
            .map(|d| d.to_string())
            .unwrap_or_default(),
    );
    vars.insert(
        "agentName".into(),
        payload.agent_name.clone().unwrap_or_default(),
    );
    vars.insert(
        "agentType".into(),
        payload.agent_type.clone().unwrap_or_default(),
    );
    vars.insert(
        "tmuxTail".into(),
        payload.tmux_tail.clone().unwrap_or_default(),
    );
    vars.insert(
        "tmuxPaneId".into(),
        payload.tmux_pane_id.clone().unwrap_or_default(),
    );

    // Computed variables
    vars.insert("duration".into(), format_duration(payload.duration_ms));
    vars.insert(
        "time".into(),
        chrono::DateTime::parse_from_rfc3339(&payload.timestamp)
            .ok()
            .map(|dt| dt.format("%H:%M:%S").to_string())
            .unwrap_or_default(),
    );
    vars.insert(
        "modesDisplay".into(),
        payload
            .modes_used
            .as_ref()
            .filter(|v| !v.is_empty())
            .map(|v| v.join(", "))
            .unwrap_or_default(),
    );
    vars.insert(
        "iterationDisplay".into(),
        match (payload.iteration, payload.max_iterations) {
            (Some(i), Some(m)) => format!("{i}/{m}"),
            _ => String::default(),
        },
    );
    vars.insert(
        "agentDisplay".into(),
        payload
            .agents_spawned
            .map(|spawned| {
                format!(
                    "{}/{} completed",
                    payload.agents_completed.unwrap_or(0),
                    spawned
                )
            })
            .unwrap_or_default(),
    );
    vars.insert("projectDisplay".into(), project_display(payload));
    vars.insert("footer".into(), build_footer(payload));
    vars.insert("tmuxTailBlock".into(), build_tmux_tail_block(payload));
    vars.insert(
        "reasonDisplay".into(),
        payload.reason.clone().unwrap_or_else(|| "unknown".into()),
    );

    vars
}

/// Process `{{#if var}}...{{/if}}` conditionals.
/// Only simple truthy checks (non-empty string). No nesting, no else.
fn process_conditionals(
    template: &str,
    vars: &std::collections::HashMap<String, String>,
) -> String {
    CONDITIONAL_RE
        .replace_all(template, |caps: &regex::Captures| {
            let var_name = &caps[1];
            let content = &caps[2];
            match vars.get(var_name) {
                Some(val) if !val.is_empty() => content.to_string(),
                _ => String::default(),
            }
        })
        .into_owned()
}

/// Replace `{{variable}}` placeholders with values.
/// Unknown/missing variables become empty string. Skips `{{/if}}` remnants.
fn replace_variables(template: &str, vars: &std::collections::HashMap<String, String>) -> String {
    VARIABLE_RE
        .replace_all(template, |caps: &regex::Captures| {
            let var_name = &caps[1];
            if var_name == "if" {
                return caps[0].to_string(); // skip {{/if}} remnants
            }
            vars.get(var_name).cloned().unwrap_or_default()
        })
        .into_owned()
}

/// Interpolate a template string with payload values.
///
/// 1. Process `{{#if var}}...{{/if}}` conditionals
/// 2. Replace `{{variable}}` placeholders
/// 3. Trim trailing whitespace
pub fn interpolate(template: &str, payload: &NotificationPayload) -> String {
    let vars = compute_variables(payload);
    let result = process_conditionals(template, &vars);
    let result = replace_variables(&result, &vars);
    result.trim_end().to_string()
}

/// Validate a template string for unknown variables.
pub fn validate(template: &str) -> (bool, Vec<String>) {
    let mut unknown = Vec::new();

    for caps in CONDITIONAL_RE.captures_iter(template) {
        let var_name = caps[1].to_string();
        if !KNOWN_VARIABLES.contains(var_name.as_str()) && !unknown.contains(&var_name) {
            unknown.push(var_name);
        }
    }

    for caps in VARIABLE_RE.captures_iter(template) {
        let var_name = caps[1].to_string();
        if var_name == "if" {
            continue; // skip {{/if}} remnants
        }
        if !KNOWN_VARIABLES.contains(var_name.as_str()) && !unknown.contains(&var_name) {
            unknown.push(var_name);
        }
    }

    let valid = unknown.is_empty();
    (valid, unknown)
}

/// Default templates that match the TypeScript formatter output.
pub fn default_template(event: &crate::types::NotificationEvent) -> String {
    use crate::types::NotificationEvent as E;
    match event {
        E::SessionStart => concat!(
            "# Session Started\n\n",
            "**Session:** `{{sessionId}}`\n",
            "**Project:** `{{projectDisplay}}`\n",
            "**Time:** {{time}}",
            "{{#if tmuxSession}}\n**tmux:** `{{tmuxSession}}`{{/if}}",
        )
        .into(),
        E::SessionStop => concat!(
            "# Session Continuing\n",
            "{{#if activeMode}}\n**Mode:** {{activeMode}}{{/if}}",
            "{{#if iterationDisplay}}\n**Iteration:** {{iterationDisplay}}{{/if}}",
            "{{#if incompleteTasks}}\n**Incomplete tasks:** {{incompleteTasks}}{{/if}}",
            "\n\n{{footer}}",
        )
        .into(),
        E::SessionEnd => concat!(
            "# Session Ended\n\n",
            "**Session:** `{{sessionId}}`\n",
            "**Duration:** {{duration}}\n",
            "**Reason:** {{reasonDisplay}}",
            "{{#if agentDisplay}}\n**Agents:** {{agentDisplay}}{{/if}}",
            "{{#if modesDisplay}}\n**Modes:** {{modesDisplay}}{{/if}}",
            "{{#if contextSummary}}\n\n**Summary:** {{contextSummary}}{{/if}}",
            "{{tmuxTailBlock}}",
            "\n\n{{footer}}",
        )
        .into(),
        E::SessionIdle => concat!(
            "# Session Idle\n\n",
            "Claude has finished and is waiting for input.\n",
            "{{#if reason}}\n**Reason:** {{reason}}{{/if}}",
            "{{#if modesDisplay}}\n**Modes:** {{modesDisplay}}{{/if}}",
            "{{tmuxTailBlock}}",
            "\n\n{{footer}}",
        )
        .into(),
        E::AskUserQuestion => concat!(
            "# Input Needed\n",
            "{{#if question}}\n**Question:** {{question}}\n{{/if}}",
            "\nClaude is waiting for your response.\n\n{{footer}}",
        )
        .into(),
        E::AgentCall => concat!(
            "# Agent Spawned\n",
            "{{#if agentName}}\n**Agent:** `{{agentName}}`{{/if}}",
            "{{#if agentType}}\n**Type:** `{{agentType}}`{{/if}}",
            "\n\n{{footer}}",
        )
        .into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{NotificationEvent, NotificationPayload};

    fn test_payload() -> NotificationPayload {
        NotificationPayload {
            event: NotificationEvent::SessionStart,
            session_id: "test-123".into(),
            message: String::default(),
            timestamp: "2026-01-15T09:30:00Z".into(),
            tmux_session: Some("my-session".into()),
            project_path: Some("/home/user/my-project".into()),
            project_name: Some("my-project".into()),
            ..Default::default()
        }
    }

    #[test]
    fn interpolate_default_start_template() {
        let payload = test_payload();
        let tmpl = default_template(&NotificationEvent::SessionStart);
        let result = interpolate(&tmpl, &payload);
        assert!(result.contains("Session Started"));
        assert!(result.contains("test-123"));
        assert!(result.contains("my-project"));
        assert!(result.contains("**tmux:** `my-session`"));
    }

    #[test]
    fn conditional_hidden_when_empty() {
        let mut payload = test_payload();
        payload.tmux_session = None;
        let tmpl = "{{#if tmuxSession}}tmux: {{tmuxSession}}{{/if}}";
        let result = interpolate(tmpl, &payload);
        assert_eq!(result, "");
    }

    #[test]
    fn validate_known_vars() {
        let (valid, unknown) = validate("{{sessionId}} {{event}}");
        assert!(valid);
        assert!(unknown.is_empty());
    }

    #[test]
    fn validate_unknown_var() {
        let (valid, unknown) = validate("{{sessionId}} {{bogus}}");
        assert!(!valid);
        assert!(unknown.contains(&"bogus".to_string()));
    }

    #[test]
    fn format_duration_values() {
        assert_eq!(format_duration(None), "unknown");
        assert_eq!(format_duration(Some(500)), "0s");
        assert_eq!(format_duration(Some(1500)), "1s");
        assert_eq!(format_duration(Some(65000)), "1m 5s");
        assert_eq!(format_duration(Some(3665000)), "1h 1m 5s");
    }

    #[test]
    fn project_display_fallback() {
        let mut payload = test_payload();
        payload.project_name = None;
        payload.project_path = None;
        assert_eq!(project_display(&payload), "unknown");
    }

    #[test]
    fn tmux_tail_block_empty() {
        let mut payload = test_payload();
        payload.tmux_tail = None;
        assert_eq!(build_tmux_tail_block(&payload), "");
    }

    #[test]
    fn tmux_tail_block_present() {
        let mut payload = test_payload();
        payload.tmux_tail = Some("line 1\nline 2".into());
        let block = build_tmux_tail_block(&payload);
        assert!(block.contains("```"));
        assert!(block.contains("line 1"));
    }
}
