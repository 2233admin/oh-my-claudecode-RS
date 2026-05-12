//! Claude Code hook format mapping.

use crate::adapter::{HookEntry, HostKind};
use crate::unified_hooks::UnifiedHookEvent;

/// Map a unified event to a Claude Code hook entry.
/// Returns `None` if the event is not supported by Claude Code.
pub fn map_claude_hook(
    event: &UnifiedHookEvent,
    command: &str,
    timeout_secs: u64,
) -> Option<HookEntry> {
    let event_name = event.to_host_event(HostKind::Claude)?;
    Some(HookEntry {
        event_name: event_name.to_string(),
        command: command.to_string(),
        timeout_secs,
    })
}

/// All events that Claude Code supports natively.
pub fn claude_supported_events() -> &'static [UnifiedHookEvent] {
    &[
        UnifiedHookEvent::PreToolUse,
        UnifiedHookEvent::PostToolUse,
        UnifiedHookEvent::Stop,
        UnifiedHookEvent::SessionStart,
        UnifiedHookEvent::UserPromptSubmit,
        UnifiedHookEvent::SessionEnd,
        UnifiedHookEvent::PostToolUseFailure,
        UnifiedHookEvent::PreCompact,
        UnifiedHookEvent::PermissionRequest,
        UnifiedHookEvent::Notification,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_events_map_correctly() {
        let entry = map_claude_hook(&UnifiedHookEvent::PreToolUse, "my-cmd", 30).unwrap();
        assert_eq!(entry.event_name, "PreToolUse");
        assert_eq!(entry.command, "my-cmd");
    }

    #[test]
    fn omc_synthetic_returns_none() {
        assert!(map_claude_hook(&UnifiedHookEvent::TaskCreated, "cmd", 10).is_none());
    }

    #[test]
    fn codex_only_returns_none() {
        assert!(map_claude_hook(&UnifiedHookEvent::PostCompact, "cmd", 10).is_none());
    }

    #[test]
    fn supported_events_count() {
        assert_eq!(claude_supported_events().len(), 10);
    }
}
