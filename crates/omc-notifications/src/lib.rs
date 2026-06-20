//! omc-notifications: Multi-platform lifecycle notification system for oh-my-claudecode-RS
//!
//! Sends notifications to Slack and generic webhooks on session lifecycle events.
//! Includes a template engine with `{{variable}}` interpolation and tmux session detection.

pub mod dispatcher;
pub mod slack;
pub mod template;
pub mod tmux;
pub mod types;

pub use dispatcher::dispatch;
pub use template::{default_template, interpolate, validate};
pub use tmux::{capture_pane, current_pane_id, current_session, format_info, team_sessions};
pub use types::{
    DispatchResult, NotificationConfig, NotificationEvent, NotificationPayload,
    NotificationPlatform, NotificationResult, SlackConfig, WebhookConfig,
};

use tracing::warn;

/// High-level notification function.
///
/// Reads config, formats the message, and dispatches to all configured platforms.
/// Non-blocking, swallows errors to never propagate to callers.
pub async fn notify(
    config: &NotificationConfig,
    event: NotificationEvent,
    mut payload: NotificationPayload,
) -> Option<DispatchResult> {
    if !config.enabled {
        return None;
    }

    // Auto-populate tmux context if not set
    if payload.tmux_session.is_none() {
        payload.tmux_session = current_session();
    }
    if payload.tmux_pane_id.is_none() {
        payload.tmux_pane_id = current_pane_id();
    }

    // Format message using template if not already set
    if payload.message.is_empty() {
        let tmpl = default_template(&event);
        payload.message = interpolate(&tmpl, &payload);
    }

    match dispatch(config, &event, payload).await {
        result if result.any_success => Some(result),
        result => {
            for r in &result.results {
                if let Some(err) = &r.error {
                    warn!(platform = %r.platform, error = %err, "Notification send failed");
                }
            }
            Some(result)
        }
    }
}
