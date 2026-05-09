//! omc-shared: Shared types, config, and state for oh-my-claudecode-RS

pub mod config;
pub mod context_strategy;
pub mod events;
pub mod memory;
pub mod prelude;
pub mod resilience;
pub mod routing;
pub mod shared_memory;
pub mod state;
pub mod tools;
pub mod types;

pub use config::{Config, ConfigError, OmcPaths};
pub use shared_memory::{MemoryEntry, SharedMemory, SharedMemoryError};
pub use state::{
    AppState, ContextSample, HudState, SessionInfo, SessionState, StateError, StateReader,
    StateWriter, TeamRunRecord,
};
