pub mod agents;
pub mod api_key_source;
pub mod autopilot;
pub mod background_tasks;
pub mod call_counts;
pub mod cjk_width;
pub mod color_degrade;
pub mod context;
pub mod context_eta;
pub mod context_warning;
pub mod cost;
pub mod cwd;
pub mod effort;
pub mod enterprise_cost;
pub mod git_branch;
pub mod git_repo;
pub mod git_status;
pub mod hostname;
pub mod last_skill;
pub mod last_tool;
pub mod mission_board_el;
pub mod model_name;
pub mod omc_label;
pub mod permissions;
pub mod pet;
pub mod prd;
pub mod profile;
pub mod prompt_time;
pub mod ralph;
pub mod rate_limits;
pub mod session_health;
pub mod session_name;
pub mod session_stats;
pub mod skills;
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
    SessionName,
    Effort,
    VimMode,
    WorktreeInfo,
    Ralph,
    ContextWarning,
    Hostname,
    LastTool,
    GitRepo,
    GitBranch,
    Profile,
    OmcLabel,
    LastSkill,
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
    pub fn for_test(input: &'a Input, cache: &'a HudCache, color_level: ColorLevel) -> Self {
        use crate::i18n;
        Self {
            input,
            cache,
            color_level,
            strings: i18n::strings(i18n::Locale::En),
            omc_state: test_helpers::empty_omc_state(),
            usage: None,
            transcript: None,
            mission_board: None,
            config: test_helpers::empty_config(),
        }
    }
}

pub const DEFAULT_ELEMENTS: &[Element] = &[
    Element::SessionHealth,
    Element::SessionStats,
    Element::SessionName,
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
    Element::Ralph,
    Element::ContextWarning,
    Element::Hostname,
    Element::LastTool,
    Element::GitRepo,
    Element::GitBranch,
    Element::Profile,
    Element::OmcLabel,
    Element::LastSkill,
];

/// Map Element → (config_key, default_enabled).
/// Returns None if the element has no config gate (always rendered).
fn element_config_key(element: Element) -> Option<(&'static str, bool)> {
    match element {
        Element::Hostname => Some(("hostname", false)),
        Element::GitRepo => Some(("gitRepo", false)),
        Element::GitBranch => Some(("gitBranch", false)),
        Element::GitStatus => Some(("gitStatus", false)),
        Element::ModelName => Some(("model", false)),
        Element::ApiKeySource => Some(("apiKeySource", false)),
        Element::Profile => Some(("profile", true)),
        Element::OmcLabel => Some(("omcLabel", true)),
        Element::RateLimits => Some(("rateLimits", true)),
        Element::EnterpriseCost => Some(("enterpriseCost", true)),
        Element::Permissions => Some(("permissionStatus", false)),
        Element::Thinking => Some(("thinking", true)),
        Element::PromptTimeElapsed => Some(("promptTime", true)),
        Element::SessionHealth => Some(("sessionHealth", true)),
        Element::TokenUsage => Some(("showTokens", false)),
        Element::Ralph => Some(("ralph", true)),
        Element::AutopilotState => Some(("autopilot", true)),
        Element::Prd => Some(("prdStory", true)),
        Element::Skills => Some(("activeSkills", true)),
        Element::LastSkill => Some(("lastSkill", true)),
        Element::Context => Some(("contextBar", true)),
        Element::Agents => Some(("agents", true)),
        Element::BackgroundTasks => Some(("backgroundTasks", true)),
        Element::CallCounts => Some(("showCallCounts", true)),
        Element::LastTool => Some(("showLastTool", false)),
        Element::MissionBoard => Some(("missionBoard", false)),
        Element::Todos => Some(("todos", true)),
        Element::Cwd => Some(("cwd", false)),
        _ => None,
    }
}

pub fn render_element(element: Element, ctx: &RenderContext<'_>) -> Option<String> {
    // Config gate: check per-element enable flag before rendering
    if let Some((key, default)) = element_config_key(element)
        && !ctx.config.element_enabled(key, default)
    {
        return None;
    }
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
        Element::SessionName => session_name::render(ctx),
        Element::Effort => effort::render(ctx),
        Element::VimMode => vim_mode::render(ctx),
        Element::WorktreeInfo => worktree_info::render(ctx),
        Element::Ralph => ralph::render(ctx),
        Element::ContextWarning => context_warning::render(ctx),
        Element::Hostname => hostname::render(ctx),
        Element::LastTool => last_tool::render(ctx),
        Element::GitRepo => git_repo::render(ctx),
        Element::GitBranch => git_branch::render(ctx),
        Element::Profile => profile::render(ctx),
        Element::OmcLabel => omc_label::render(ctx),
        Element::LastSkill => last_skill::render(ctx),
    }
}

#[cfg(test)]
mod test_helpers {
    use crate::config::HudConfig;
    use crate::omc_state::OmcState;
    use std::sync::OnceLock;

    pub fn empty_omc_state() -> &'static OmcState {
        static S: OnceLock<OmcState> = OnceLock::new();
        S.get_or_init(OmcState::default)
    }

    pub fn empty_config() -> &'static HudConfig {
        static C: OnceLock<HudConfig> = OnceLock::new();
        C.get_or_init(HudConfig::default)
    }
}
