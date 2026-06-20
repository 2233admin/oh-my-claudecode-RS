//! Shared rate computation and time-window helpers.

use chrono::{DateTime, Duration, Utc};

use crate::model::UsageEvent;

/// Compute tokens per minute from events that fall inside the trailing
/// `window_seconds` ending at `now`. Events outside the window are ignored.
pub fn tokens_per_min(events: &[UsageEvent], now: DateTime<Utc>, window_seconds: i64) -> f64 {
    if window_seconds <= 0 || events.is_empty() {
        return 0.0;
    }
    let cutoff = now - Duration::seconds(window_seconds);
    let total: u64 = events
        .iter()
        .filter(|e| e.timestamp >= cutoff && e.timestamp <= now)
        .map(|e| e.delta_tokens)
        .sum();
    if total == 0 {
        return 0.0;
    }
    let per_second = total as f64 / window_seconds as f64;
    per_second * 60.0
}

/// True if the most recent activity is within `max_age_minutes` of `now`.
pub fn is_active(last_activity: DateTime<Utc>, now: DateTime<Utc>, max_age_minutes: i64) -> bool {
    let age = now - last_activity;
    age <= Duration::minutes(max_age_minutes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ts(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs, 0).unwrap()
    }

    #[test]
    fn rate_zero_when_empty() {
        assert_eq!(tokens_per_min(&[], ts(1000), 60), 0.0);
    }

    #[test]
    fn rate_sums_events_in_window() {
        let now = ts(1000);
        let events = vec![
            UsageEvent {
                timestamp: ts(970),
                delta_tokens: 1200,
            },
            UsageEvent {
                timestamp: ts(990),
                delta_tokens: 3600,
            },
        ];
        // 4800 tokens / 60 s * 60 = 4800 tokens/min
        assert_eq!(tokens_per_min(&events, now, 60), 4800.0);
    }

    #[test]
    fn rate_ignores_events_outside_window() {
        let now = ts(1000);
        let events = vec![
            UsageEvent {
                timestamp: ts(100), // way old
                delta_tokens: 9_999_999,
            },
            UsageEvent {
                timestamp: ts(990),
                delta_tokens: 600,
            },
        ];
        // 600 / 60 * 60 = 600
        assert_eq!(tokens_per_min(&events, now, 60), 600.0);
    }

    #[test]
    fn rate_zero_when_no_tokens_in_window() {
        let now = ts(1000);
        let events = vec![UsageEvent {
            timestamp: ts(100),
            delta_tokens: 50,
        }];
        assert_eq!(tokens_per_min(&events, now, 60), 0.0);
    }

    #[test]
    fn active_within_max_age() {
        let now = ts(10_000);
        assert!(is_active(ts(9_900), now, 5)); // 100s ago -> within 5 min
        assert!(!is_active(ts(9_000), now, 5)); // 1000s ago -> out
    }
}
