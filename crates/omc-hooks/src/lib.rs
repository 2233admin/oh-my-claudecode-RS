pub mod config;
pub mod events;
pub mod executor;
pub mod registry;

pub use config::{EventHooks, HookCommand, HookEntry, HooksConfig, HooksConfigError};
pub use events::{HookEvent, ToolName};
pub use executor::{HookExecutor, HookResult, HookSpecificOutput};
pub use registry::{
    HookExecutionResult, HookMatch, HookRegistry, HookRegistryError, HookRegistryStats, HookSource,
    InternalHook,
};
