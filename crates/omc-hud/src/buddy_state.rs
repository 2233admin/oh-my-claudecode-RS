use serde::Deserialize;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAX_REACTION_AGE_MS: u64 = 30_000;

#[derive(Debug, Default, Clone, Deserialize)]
pub struct BuddyState {
    pub name: Option<String>,
    pub species: Option<String>,
    pub mood: Option<String>,
    pub reaction: Option<BuddyReaction>,
    #[serde(default)]
    pub muted: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BuddyReaction {
    pub text: Option<String>,
    pub ts: Option<u64>,
}

impl BuddyState {
    /// Returns the reaction text if it exists, is non-empty, buddy is not muted,
    /// and it arrived within the last 30 seconds.
    pub fn active_reaction(&self) -> Option<&str> {
        if self.muted {
            return None;
        }
        let r = self.reaction.as_ref()?;
        let text = r.text.as_deref()?;
        if text.is_empty() {
            return None;
        }
        if let Some(ts) = r.ts {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_millis() as u64;
            if now.saturating_sub(ts) > MAX_REACTION_AGE_MS {
                return None;
            }
        }
        Some(text)
    }
}

pub fn load() -> Option<BuddyState> {
    let path = buddy_state_path()?;
    let raw = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn buddy_state_path() -> Option<std::path::PathBuf> {
    // Respect $CLAUDE_CONFIG_DIR if set (buddy respects it too)
    if let Ok(dir) = std::env::var("CLAUDE_CONFIG_DIR") {
        let p = std::path::PathBuf::from(dir).join("buddy-state").join("status.json");
        if p.exists() {
            return Some(p);
        }
    }
    let home = dirs::home_dir()?;
    Some(home.join(".claude-buddy").join("status.json"))
}
