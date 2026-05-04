pub mod autopilot;
pub mod cjk_width;
pub mod color_degrade;
pub mod context;
pub mod context_eta;
pub mod cost;
pub mod git_status;
pub mod model_name;
pub mod prompt_time;
pub mod rate_limits;
pub mod todos;
pub mod token_usage;

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
}

pub struct RenderContext<'a> {
    pub input: &'a Input,
    pub cache: &'a HudCache,
    pub color_level: ColorLevel,
    pub strings: &'static Strings,
}

pub const DEFAULT_ELEMENTS: &[Element] = &[
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
    Element::ColorDegrade,
    Element::CjkWidth,
    Element::I18n,
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
    }
}
