//! Slack webhook notification sender.

use crate::types::{NotificationPayload, NotificationResult, SlackConfig};

const SEND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Validate a Slack webhook URL. Must be HTTPS from hooks.slack.com.
fn validate_webhook_url(url: &str) -> bool {
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    parsed.scheme() == "https"
        && (parsed.host_str() == Some("hooks.slack.com")
            || parsed
                .host_str()
                .is_some_and(|h| h.ends_with(".hooks.slack.com")))
}

/// Validate a Slack mention string.
/// Allowed: `<@U12345678>`, `<!channel>`, `<!here>`, `<!everyone>`, `<!subteam^S123>`.
fn validate_mention(mention: &str) -> Option<String> {
    if mention.is_empty() {
        return None;
    }
    // Must start with < and end with >
    if !(mention.starts_with('<') && mention.ends_with('>')) {
        return None;
    }
    let inner = &mention[1..mention.len() - 1];
    // User mention: <@U...>
    if inner.starts_with('@') && inner.len() > 1 {
        return Some(mention.to_string());
    }
    // Special: <!channel>, <!here>, <!everyone>
    if inner == "!channel" || inner == "!here" || inner == "!everyone" {
        return Some(mention.to_string());
    }
    // User group: <!subteam^S123>
    if inner.starts_with("!subteam^") {
        return Some(mention.to_string());
    }
    None
}

/// Validate a Slack channel name/ID.
fn validate_channel(channel: &str) -> Option<String> {
    if channel.is_empty() {
        return None;
    }
    // Channel ID (starts with C) or channel name (#channel)
    if channel.starts_with('C') || channel.starts_with('#') {
        Some(channel.to_string())
    } else {
        None
    }
}

/// Validate a Slack username.
fn validate_username(username: &str) -> Option<String> {
    if username.is_empty() {
        return None;
    }
    // No whitespace, no @ prefix
    if username.contains(char::is_whitespace) || username.starts_with('@') {
        return None;
    }
    Some(username.to_string())
}

/// Send a notification via Slack incoming webhook.
pub async fn send(config: &SlackConfig, payload: &NotificationPayload) -> NotificationResult {
    if !config.enabled || config.webhook_url.is_empty() {
        return NotificationResult {
            platform: crate::types::NotificationPlatform::Slack,
            success: false,
            error: Some("Not configured".to_string()),
            message_id: None,
        };
    }

    if !validate_webhook_url(&config.webhook_url) {
        return NotificationResult {
            platform: crate::types::NotificationPlatform::Slack,
            success: false,
            error: Some("Invalid webhook URL".to_string()),
            message_id: None,
        };
    }

    let text = compose_text(&payload.message, config.mention.as_deref());

    let mut body = serde_json::json!({ "text": text });
    if let Some(ch) = config.channel.as_deref().and_then(validate_channel) {
        body["channel"] = serde_json::Value::String(ch);
    }
    if let Some(un) = config.username.as_deref().and_then(validate_username) {
        body["username"] = serde_json::Value::String(un);
    }

    let client = reqwest::Client::builder()
        .timeout(SEND_TIMEOUT)
        .build()
        .unwrap_or_default();

    match client.post(&config.webhook_url).json(&body).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                NotificationResult {
                    platform: crate::types::NotificationPlatform::Slack,
                    success: true,
                    error: None,
                    message_id: None,
                }
            } else {
                NotificationResult {
                    platform: crate::types::NotificationPlatform::Slack,
                    success: false,
                    error: Some(format!("HTTP {}", resp.status())),
                    message_id: None,
                }
            }
        }
        Err(e) => NotificationResult {
            platform: crate::types::NotificationPlatform::Slack,
            success: false,
            error: Some(e.to_string()),
            message_id: None,
        },
    }
}

/// Compose Slack message text with optional mention prefix.
fn compose_text(message: &str, mention: Option<&str>) -> String {
    if let Some(m) = mention.and_then(validate_mention) {
        format!("{m}\n{message}")
    } else {
        message.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_webhook_url_valid() {
        assert!(validate_webhook_url(
            "https://hooks.slack.com/services/T00/B00/xxx"
        ));
    }

    #[test]
    fn validate_webhook_url_invalid_scheme() {
        assert!(!validate_webhook_url(
            "http://hooks.slack.com/services/T00/B00/xxx"
        ));
    }

    #[test]
    fn validate_webhook_url_invalid_host() {
        assert!(!validate_webhook_url(
            "https://evil.com/services/T00/B00/xxx"
        ));
    }

    #[test]
    fn validate_mention_user() {
        assert_eq!(
            validate_mention("<@U12345678>"),
            Some("<@U12345678>".to_string())
        );
    }

    #[test]
    fn validate_mention_channel() {
        assert_eq!(
            validate_mention("<!channel>"),
            Some("<!channel>".to_string())
        );
    }

    #[test]
    fn validate_mention_invalid() {
        assert_eq!(validate_mention("hello"), None);
    }

    #[test]
    fn compose_text_with_mention() {
        let result = compose_text("hello", Some("<!channel>"));
        assert_eq!(result, "<!channel>\nhello");
    }

    #[test]
    fn compose_text_without_mention() {
        let result = compose_text("hello", None);
        assert_eq!(result, "hello");
    }

    #[test]
    fn validate_channel_id() {
        assert_eq!(validate_channel("C12345"), Some("C12345".to_string()));
    }

    #[test]
    fn validate_channel_name() {
        assert_eq!(validate_channel("#general"), Some("#general".to_string()));
    }

    #[test]
    fn validate_channel_invalid() {
        assert_eq!(validate_channel(""), None);
    }
}
