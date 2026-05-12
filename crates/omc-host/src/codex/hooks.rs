//! Codex CLI hook format mapping.

use crate::adapter::{HookEntry, HostKind};
use crate::unified_hooks::UnifiedHookEvent;

/// Map a unified event to a Codex CLI hook entry.
pub fn map_codex_hook(
    event: &UnifiedHookEvent,
    command: &str,
    timeout_secs: u64,
) -> Option<HookEntry> {
    let event_name = event.to_host_event(HostKind::Codex)?;
    Some(HookEntry {
        event_name: event_name.to_string(),
        command: command.to_string(),
        timeout_secs,
    })
}

/// All events that Codex CLI supports natively.
pub fn codex_supported_events() -> &'static [UnifiedHookEvent] {
    &[
        UnifiedHookEvent::PreToolUse,
        UnifiedHookEvent::PostToolUse,
        UnifiedHookEvent::Stop,
        UnifiedHookEvent::SessionStart,
        UnifiedHookEvent::UserPromptSubmit,
        UnifiedHookEvent::PreCompact,
        UnifiedHookEvent::PostCompact,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_events_map_to_snake_case() {
        let entry = map_codex_hook(&UnifiedHookEvent::PreToolUse, "cmd", 30).unwrap();
        assert_eq!(entry.event_name, "pre_tool_use");
    }

    #[test]
    fn claude_only_returns_none() {
        assert!(map_codex_hook(&UnifiedHookEvent::Notification, "cmd", 10).is_none());
        assert!(map_codex_hook(&UnifiedHookEvent::PermissionRequest, "cmd", 10).is_none());
    }

    #[test]
    fn codex_unique_post_compact() {
        let entry = map_codex_hook(&UnifiedHookEvent::PostCompact, "cmd", 10).unwrap();
        assert_eq!(entry.event_name, "post_compact");
    }

    #[test]
    fn supported_events_count() {
        assert_eq!(codex_supported_events().len(), 7);
    }
}
