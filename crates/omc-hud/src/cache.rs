use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::input::Input;

const MAX_SAMPLES: usize = 36;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSample {
    pub ts_ms: u64,
    pub tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HudCache {
    pub session_id: String,
    pub context_samples: Vec<ContextSample>,
    pub last_updated_ms: u64,
}

impl HudCache {
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            context_samples: Vec::new(),
            last_updated_ms: now_ms(),
        }
    }

    pub fn record_context(&mut self, tokens: Option<u64>, ts_ms: u64) {
        let Some(tokens) = tokens else {
            return;
        };

        if self
            .context_samples
            .last()
            .is_some_and(|sample| sample.ts_ms == ts_ms && sample.tokens == tokens)
        {
            return;
        }

        self.context_samples.push(ContextSample { ts_ms, tokens });
        if self.context_samples.len() > MAX_SAMPLES {
            let overflow = self.context_samples.len() - MAX_SAMPLES;
            self.context_samples.drain(0..overflow);
        }
        self.last_updated_ms = ts_ms;
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

pub fn cache_path(input: &Input) -> Option<PathBuf> {
    let session_id = input.session_id.as_deref()?;
    let cwd = input.cwd.as_deref().unwrap_or(".");
    Some(
        Path::new(cwd)
            .join(".omc")
            .join("state")
            .join("sessions")
            .join(session_id)
            .join("hud-cache.json"),
    )
}

pub fn load(input: &Input) -> HudCache {
    let session_id = input
        .session_id
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let Some(path) = cache_path(input) else {
        return HudCache::new(session_id);
    };

    let cache = fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<HudCache>(&raw).ok());

    match cache {
        Some(mut cache) if cache.session_id == session_id => {
            cache.context_samples.truncate(MAX_SAMPLES);
            cache
        }
        Some(_) | None => HudCache::new(session_id),
    }
}

pub fn save(input: &Input, cache: &HudCache) {
    let Some(path) = cache_path(input) else {
        return;
    };

    if let Some(parent) = path.parent()
        && let Err(err) = fs::create_dir_all(parent)
    {
        eprintln!(
            "omc-hud: failed to create cache dir {}: {err}",
            parent.display()
        );
        return;
    }

    let tmp = path.with_extension("json.tmp");
    let Ok(bytes) = serde_json::to_vec(cache) else {
        eprintln!("omc-hud: failed to serialize cache");
        return;
    };

    if let Err(err) = fs::write(&tmp, bytes) {
        eprintln!(
            "omc-hud: failed to write cache temp {}: {err}",
            tmp.display()
        );
        return;
    }
    if let Err(err) = fs::rename(&tmp, &path) {
        eprintln!(
            "omc-hud: failed to rename cache temp {} to {}: {err}",
            tmp.display(),
            path.display()
        );
        let _ = fs::remove_file(tmp);
    }
}
