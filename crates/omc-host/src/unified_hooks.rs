//! Unified hook event system — the superset of Claude Code, Codex CLI, and OMC events.

use crate::adapter::HostKind;
use serde::{Deserialize, Serialize};

/// Unified hook events covering all host-specific and OMC-synthetic events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnifiedHookEvent {
    // ── Shared by both hosts ───────────────────────────────────────
    PreToolUse,
    PostToolUse,
    Stop,
    SessionStart,

    // ── Claude Code ────────────────────────────────────────────────
    UserPromptSubmit,
    SessionEnd,
    PostToolUseFailure,
    PreCompact,
    PermissionRequest,
    Notification,

    // ── Codex CLI ──────────────────────────────────────────────────
    PostCompact,

    // ── OMC-synthetic (host-agnostic) ──────────────────────────────
    SubagentStart,
    SubagentStop,
    TaskCreated,
    TaskCompleted,
    TeammateIdle,
}

impl UnifiedHookEvent {
    /// Map to the host-specific event string.
    /// Returns `None` for OMC-synthetic events or events not supported by the host.
    pub fn to_host_event(&self, host: HostKind) -> Option<&'static str> {
        match (self, host) {
            // Shared
            (Self::PreToolUse, HostKind::Claude) => Some("PreToolUse"),
            (Self::PreToolUse, HostKind::Codex) => Some("pre_tool_use"),
            (Self::PostToolUse, HostKind::Claude) => Some("PostToolUse"),
            (Self::PostToolUse, HostKind::Codex) => Some("post_tool_use"),
            (Self::Stop, HostKind::Claude) => Some("Stop"),
            (Self::Stop, HostKind::Codex) => Some("stop"),
            (Self::SessionStart, HostKind::Claude) => Some("SessionStart"),
            (Self::SessionStart, HostKind::Codex) => Some("session_start"),

            // Claude-only
            (Self::UserPromptSubmit, HostKind::Claude) => Some("UserPromptSubmit"),
            (Self::SessionEnd, HostKind::Claude) => Some("SessionEnd"),
            (Self::PostToolUseFailure, HostKind::Claude) => Some("PostToolUseFailure"),
            (Self::PreCompact, HostKind::Claude) => Some("PreCompact"),
            (Self::PermissionRequest, HostKind::Claude) => Some("PermissionRequest"),
            (Self::Notification, HostKind::Claude) => Some("Notification"),

            // Codex-only
            (Self::PostCompact, HostKind::Codex) => Some("post_compact"),

            // Also support some on Codex (they exist there too)
            (Self::UserPromptSubmit, HostKind::Codex) => Some("user_prompt_submit"),
            (Self::PreCompact, HostKind::Codex) => Some("pre_compact"),

            // OMC-synthetic or unsupported
            _ => None,
        }
    }

    /// Whether this event is OMC-specific (synthetic, not from any host CLI).
    pub fn is_omc_specific(&self) -> bool {
        matches!(
            self,
            Self::SubagentStart
                | Self::SubagentStop
                | Self::TaskCreated
                | Self::TaskCompleted
                | Self::TeammateIdle
        )
    }

    /// Which hosts support this event natively.
    pub fn supported_hosts(&self) -> &'static [HostKind] {
        match self {
            // Shared
            Self::PreToolUse | Self::PostToolUse | Self::Stop | Self::SessionStart => {
                &[HostKind::Claude, HostKind::Codex]
            }
            // Claude + Codex
            Self::UserPromptSubmit | Self::PreCompact => {
                &[HostKind::Claude, HostKind::Codex]
            }
            // Claude-only
            Self::SessionEnd
            | Self::PostToolUseFailure
            | Self::PermissionRequest
            | Self::Notification => &[HostKind::Claude],
            // Codex-only
            Self::PostCompact => &[HostKind::Codex],
            // OMC-synthetic
            _ => &[],
        }
    }

    /// All variants.
    pub fn all() -> &'static [UnifiedHookEvent] {
        &[
            Self::PreToolUse,
            Self::PostToolUse,
            Self::Stop,
            Self::SessionStart,
            Self::UserPromptSubmit,
            Self::SessionEnd,
            Self::PostToolUseFailure,
            Self::PreCompact,
            Self::PermissionRequest,
            Self::Notification,
            Self::PostCompact,
            Self::SubagentStart,
            Self::SubagentStop,
            Self::TaskCreated,
            Self::TaskCompleted,
            Self::TeammateIdle,
        ]
    }
}

impl std::fmt::Display for UnifiedHookEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::Stop => "Stop",
            Self::SessionStart => "SessionStart",
            Self::UserPromptSubmit => "UserPromptSubmit",
            Self::SessionEnd => "SessionEnd",
            Self::PostToolUseFailure => "PostToolUseFailure",
            Self::PreCompact => "PreCompact",
            Self::PermissionRequest => "PermissionRequest",
            Self::Notification => "Notification",
            Self::PostCompact => "PostCompact",
            Self::SubagentStart => "SubagentStart",
            Self::SubagentStop => "SubagentStop",
            Self::TaskCreated => "TaskCreated",
            Self::TaskCompleted => "TaskCompleted",
            Self::TeammateIdle => "TeammateIdle",
        };
        f.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_events_have_both_hosts() {
        for ev in [
            UnifiedHookEvent::PreToolUse,
            UnifiedHookEvent::PostToolUse,
            UnifiedHookEvent::Stop,
            UnifiedHookEvent::SessionStart,
        ] {
            assert!(
                ev.to_host_event(HostKind::Claude).is_some(),
                "{ev} should map to Claude"
            );
            assert!(
                ev.to_host_event(HostKind::Codex).is_some(),
                "{ev} should map to Codex"
            );
        }
    }

    #[test]
    fn claude_only_events() {
        for ev in [
            UnifiedHookEvent::SessionEnd,
            UnifiedHookEvent::PostToolUseFailure,
            UnifiedHookEvent::PermissionRequest,
            UnifiedHookEvent::Notification,
        ] {
            assert!(ev.to_host_event(HostKind::Claude).is_some());
            assert!(ev.to_host_event(HostKind::Codex).is_none());
            assert!(ev.supported_hosts().contains(&HostKind::Claude));
            assert!(!ev.supported_hosts().contains(&HostKind::Codex));
        }
    }

    #[test]
    fn codex_only_events() {
        let ev = UnifiedHookEvent::PostCompact;
        assert!(ev.to_host_event(HostKind::Codex).is_some());
        assert!(ev.to_host_event(HostKind::Claude).is_none());
    }

    #[test]
    fn omc_synthetic_events_have_no_host_mapping() {
        for ev in [
            UnifiedHookEvent::SubagentStart,
            UnifiedHookEvent::SubagentStop,
            UnifiedHookEvent::TaskCreated,
            UnifiedHookEvent::TaskCompleted,
            UnifiedHookEvent::TeammateIdle,
        ] {
            assert!(ev.is_omc_specific());
            assert!(ev.to_host_event(HostKind::Claude).is_none());
            assert!(ev.to_host_event(HostKind::Codex).is_none());
        }
    }

    #[test]
    fn event_display_roundtrip() {
        for ev in UnifiedHookEvent::all() {
            let s = format!("{ev}");
            assert!(!s.is_empty());
        }
    }

    #[test]
    fn event_serde_roundtrip() {
        for ev in UnifiedHookEvent::all() {
            let json = serde_json::to_string(ev).unwrap();
            let back: UnifiedHookEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(back, *ev);
        }
    }

    #[test]
    fn claude_event_strings_are_pascal_case() {
        for ev in UnifiedHookEvent::all() {
            if let Some(s) = ev.to_host_event(HostKind::Claude) {
                assert!(
                    s.chars().next().unwrap().is_uppercase(),
                    "Claude event string should start uppercase: {s}"
                );
            }
        }
    }

    #[test]
    fn codex_event_strings_are_snake_case() {
        for ev in UnifiedHookEvent::all() {
            if let Some(s) = ev.to_host_event(HostKind::Codex) {
                assert!(
                    s.chars().all(|c| c.is_lowercase() || c == '_'),
                    "Codex event string should be snake_case: {s}"
                );
            }
        }
    }
}
