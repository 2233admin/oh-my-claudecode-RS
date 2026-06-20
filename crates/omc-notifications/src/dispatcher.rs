//! Notification dispatcher.
//!
//! Sends notifications to configured platforms in parallel with timeouts.
//! Individual failures do not block other platforms.

use std::time::Duration;

use crate::slack;
use crate::template;
use crate::types::{
    DispatchResult, NotificationConfig, NotificationEvent, NotificationPayload,
    NotificationPlatform, NotificationResult, WebhookConfig,
};

/// Per-request timeout for individual platform sends.
const SEND_TIMEOUT: Duration = Duration::from_secs(10);

/// Overall dispatch timeout for all platforms combined.
const DISPATCH_TIMEOUT: Duration = Duration::from_secs(15);

/// Dispatch notifications to all enabled platforms for an event.
///
/// Runs all sends in parallel with an overall timeout.
/// Individual failures don't block other platforms.
pub async fn dispatch(
    config: &NotificationConfig,
    event: &NotificationEvent,
    mut payload: NotificationPayload,
) -> DispatchResult {
    if !config.enabled {
        return DispatchResult {
            event: event.clone(),
            results: Vec::new(),
            any_success: false,
        };
    }

    // Format message if not already set
    if payload.message.is_empty() {
        let tmpl = template::default_template(event);
        payload.message = template::interpolate(&tmpl, &payload);
    }

    let mut handles = Vec::new();

    // Slack
    if let Some(slack_config) = &config.slack
        && slack_config.enabled
    {
        let sc = slack_config.clone();
        let p = payload.clone();
        handles.push(tokio::spawn(async move { slack::send(&sc, &p).await }));
    }

    // Webhook
    if let Some(webhook_config) = &config.webhook
        && webhook_config.enabled
    {
        let wc = webhook_config.clone();
        let p = payload.clone();
        handles.push(tokio::spawn(async move { send_webhook(&wc, &p).await }));
    }

    // tmux (local notification - always available if in tmux)
    // Tmux notifications are passive (session/pane info captured in payload),
    // no active send needed. The payload already includes tmux context.

    if handles.is_empty() {
        return DispatchResult {
            event: event.clone(),
            results: Vec::new(),
            any_success: false,
        };
    }

    // Race all sends against a timeout
    let results = match tokio::time::timeout(DISPATCH_TIMEOUT, futures_all(handles)).await {
        Ok(r) => r,
        Err(_) => {
            vec![NotificationResult {
                platform: NotificationPlatform::Webhook,
                success: false,
                error: Some("Dispatch timeout".to_string()),
                message_id: None,
            }]
        }
    };

    let any_success = results.iter().any(|r| r.success);
    DispatchResult {
        event: event.clone(),
        results,
        any_success,
    }
}

/// Collect results from all spawned tasks, swallowing join errors.
async fn futures_all(
    handles: Vec<tokio::task::JoinHandle<NotificationResult>>,
) -> Vec<NotificationResult> {
    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        match handle.await {
            Ok(r) => results.push(r),
            Err(e) => results.push(NotificationResult {
                platform: NotificationPlatform::Webhook,
                success: false,
                error: Some(format!("Task join error: {e}")),
                message_id: None,
            }),
        }
    }
    results
}

/// Send notification via generic webhook (POST JSON).
async fn send_webhook(config: &WebhookConfig, payload: &NotificationPayload) -> NotificationResult {
    if !config.enabled || config.url.is_empty() {
        return NotificationResult {
            platform: NotificationPlatform::Webhook,
            success: false,
            error: Some("Not configured".to_string()),
            message_id: None,
        };
    }

    // Validate URL is HTTPS
    if let Ok(parsed) = url::Url::parse(&config.url) {
        if parsed.scheme() != "https" {
            return NotificationResult {
                platform: NotificationPlatform::Webhook,
                success: false,
                error: Some("Invalid URL (HTTPS required)".to_string()),
                message_id: None,
            };
        }
    } else {
        return NotificationResult {
            platform: NotificationPlatform::Webhook,
            success: false,
            error: Some("Invalid URL".to_string()),
            message_id: None,
        };
    }

    let body = serde_json::json!({
        "event": payload.event,
        "session_id": payload.session_id,
        "message": payload.message,
        "timestamp": payload.timestamp,
        "tmux_session": payload.tmux_session,
        "project_name": payload.project_name,
        "project_path": payload.project_path,
        "modes_used": payload.modes_used,
        "duration_ms": payload.duration_ms,
        "reason": payload.reason,
        "active_mode": payload.active_mode,
        "question": payload.question,
    });

    let client = reqwest::Client::builder()
        .timeout(SEND_TIMEOUT)
        .build()
        .unwrap_or_default();

    let method = config.method.to_uppercase();
    let mut req = match method.as_str() {
        "PUT" => client.put(&config.url),
        _ => client.post(&config.url),
    };

    for (key, value) in &config.headers {
        req = req.header(key, value);
    }

    match req.json(&body).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                NotificationResult {
                    platform: NotificationPlatform::Webhook,
                    success: true,
                    error: None,
                    message_id: None,
                }
            } else {
                NotificationResult {
                    platform: NotificationPlatform::Webhook,
                    success: false,
                    error: Some(format!("HTTP {}", resp.status())),
                    message_id: None,
                }
            }
        }
        Err(e) => NotificationResult {
            platform: NotificationPlatform::Webhook,
            success: false,
            error: Some(e.to_string()),
            message_id: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_disabled_config() {
        let config = NotificationConfig {
            enabled: false,
            slack: None,
            webhook: None,
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(dispatch(
            &config,
            &NotificationEvent::SessionStart,
            NotificationPayload {
                event: NotificationEvent::SessionStart,
                session_id: "test".into(),
                timestamp: "2026-01-15T09:30:00Z".into(),
                ..Default::default()
            },
        ));
        assert!(!result.any_success);
        assert!(result.results.is_empty());
    }
}
