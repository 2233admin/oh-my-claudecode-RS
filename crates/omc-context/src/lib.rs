//! omc-context: Context injection, rules injection, and AGENTS.md management
//!
//! Ported from oh-my-claudecode's context-injector, rules-injector, and agents modules.

pub mod agents_md;
pub mod context_injector;
pub mod rules_injector;

pub use agents_md::{AgentsMdError, AgentsMdManager};
pub use context_injector::{
    ContextCollector, ContextEntry, ContextPriority, ContextSourceType, InjectionResult,
    InjectionStrategy, PendingContext, RegisterContextOptions,
};
pub use rules_injector::{RuleToInject, RulesInjector, RulesInjectorError};
