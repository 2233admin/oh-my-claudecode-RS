pub mod agents;
pub mod api_key_source;
pub mod autopilot;
pub mod session_stats;
pub mod pet;
pub mod background_tasks;
pub mod call_counts;
pub mod cjk_width;
pub mod color_degrade;
pub mod context;
pub mod context_eta;
pub mod cost;
pub mod cwd;
pub mod enterprise_cost;
pub mod git_status;
pub mod mission_board_el;
pub mod model_name;
pub mod permissions;
pub mod prd;
pub mod prompt_time;
pub mod rate_limits;
pub mod session_health;
pub mod skills;
pub mod effort;
pub mod thinking;
pub mod todos;
pub mod token_usage;
pub mod vim_mode;
pub mod worktree_info;

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::cache::HudCache;
use crate::i18n::Strings;
use crate::input::Input;
use crate::terminal::ColorLevel;

#[derive(Debug, Clone, Copy)]
pub enum Element {
    Context,
    TokenUsage,
    ModelName,
    GitStatus,
    Todos,
    AutopilotState,
    RateLimits,
    Cost,
    PromptTimeElapsed,
    ColorDegrade,
    CjkWidth,
    I18n,
    ContextEta,
    // New elements
    SessionHealth,
    Prd,
    Agents,
    BackgroundTasks,
    Cwd,
    ApiKeySource,
    EnterpriseCost,
    Skills,
    CallCounts,
    Thinking,
    Permissions,
    MissionBoard,
    SessionStats,
    Effort,
    VimMode,
    WorktreeInfo,
}

pub struct RenderContext<'a> {
    pub input: &'a Input,
    pub cache: &'a HudCache,
    pub color_level: ColorLevel,
    pub strings: &'static Strings,
    // New data sources
    pub omc_state: &'a crate::omc_state::OmcState,
    pub usage: Option<&'a crate::usage_api::UsageData>,
    pub transcript: Option<&'a crate::transcript::TranscriptData>,
    pub mission_board: Option<&'a crate::mission_board::MissionBoardState>,
    pub config: &'a crate::config::HudConfig,
}

impl<'a> RenderContext<'a> {
    /// Convenience constructor for unit tests — supplies empty/default new fields.
    #[cfg(test)]
    pub fn for_test(
        input: &'a Input,
        cache: &'a HudCache,
        color_level: ColorLevel,
    ) -> Self {
        use crate::i18n;
        Self {
            input,
            cache,
            color_level,
            strings: i18n::strings(i18n::Locale::En),
            omc_state: &EMPTY_OMC_STATE,
            usage: None,
            transcript: None,
            mission_board: None,
            config: &EMPTY_CONFIG,
        }
    }
}

#[cfg(test)]
static EMPTY_OMC_STATE: crate::omc_state::OmcState = crate::omc_state::OmcState {
    ralph: None,
    ultrawork: None,
    autopilot: None,
    hud: None,
    prd: None,
};

#[cfg(test)]
static EMPTY_CONFIG: crate::config::HudConfig = crate::config::HudConfig {
    preset: None,
    locale: None,
    elements: None,
    thresholds: None,
    usage_api_poll_interval_ms: None,
    element_order: None,
    max_width: None,
};

pub const DEFAULT_ELEMENTS: &[Element] = &[
    Element::SessionHealth,
    Element::SessionStats,
    Element::Context,
    Element::ContextEta,
    Element::TokenUsage,
    Element::ModelName,
    Element::GitStatus,
    Element::Todos,
    Element::AutopilotState,
    Element::Cost,
    Element::PromptTimeElapsed,
    Element::RateLimits,
    Element::EnterpriseCost,
    Element::Prd,
    Element::Agents,
    Element::BackgroundTasks,
    Element::Skills,
    Element::CallCounts,
    Element::Thinking,
    Element::Permissions,
    Element::MissionBoard,
    Element::Cwd,
    Element::ApiKeySource,
    Element::ColorDegrade,
    Element::CjkWidth,
    Element::I18n,
    Element::Effort,
    Element::VimMode,
    Element::WorktreeInfo,
];

pub fn render_element(element: Element, ctx: &RenderContext<'_>) -> Option<String> {
    match catch_unwind(AssertUnwindSafe(|| render_element_inner(element, ctx))) {
        Ok(value) => value,
        Err(_) => {
            eprintln!("omc-hud: element {element:?} panicked");
            Some("?".to_string())
        }
    }
}

fn render_element_inner(element: Element, ctx: &RenderContext<'_>) -> Option<String> {
    match element {
        Element::Context => context::render(ctx),
        Element::TokenUsage => token_usage::render(ctx),
        Element::ModelName => model_name::render(ctx),
        Element::GitStatus => git_status::render(ctx),
        Element::Todos => todos::render(ctx),
        Element::AutopilotState => autopilot::render(ctx),
        Element::RateLimits => rate_limits::render(ctx),
        Element::Cost => cost::render(ctx),
        Element::PromptTimeElapsed => prompt_time::render(ctx),
        Element::ColorDegrade => color_degrade::render(),
        Element::CjkWidth => cjk_width::render(),
        Element::I18n => crate::i18n::render_element(),
        Element::ContextEta => context_eta::render(ctx),
        // New elements
        Element::SessionHealth => session_health::render(ctx),
        Element::Prd => prd::render(ctx),
        Element::Agents => agents::render(ctx),
        Element::BackgroundTasks => background_tasks::render(ctx),
        Element::Cwd => cwd::render(ctx),
        Element::ApiKeySource => api_key_source::render(ctx),
        Element::EnterpriseCost => enterprise_cost::render(ctx),
        Element::Skills => skills::render(ctx),
        Element::CallCounts => call_counts::render(ctx),
        Element::Thinking => thinking::render(ctx),
        Element::Permissions => permissions::render(ctx),
        Element::MissionBoard => mission_board_el::render(ctx),
        Element::SessionStats => session_stats::render(ctx),
        Element::Effort => effort::render(ctx),
        Element::VimMode => vim_mode::render(ctx),
        Element::WorktreeInfo => worktree_info::render(ctx),
    }
}
