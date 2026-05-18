use crate::cache::HudCache;
use crate::config::HudConfig;
use crate::elements::{Element, RenderContext, render_element};
use crate::elements::pet;
use crate::i18n::Strings;
use crate::input::Input;
use crate::mission_board::MissionBoardState;
use crate::omc_state::OmcState;
use crate::terminal::ColorLevel;
use crate::transcript::TranscriptData;
use crate::usage_api::UsageData;

// Row 0: critical metrics — context bar, timing, rate limits
const ROW0: &[Element] = &[
    Element::Context,
    Element::ContextEta,
    Element::PromptTimeElapsed,
    Element::RateLimits,
    Element::EnterpriseCost,
];

// Row 1: session identity
const ROW1: &[Element] = &[
    Element::ModelName,
    Element::Cost,
    Element::TokenUsage,
    Element::GitStatus,
    Element::SessionHealth,
];

// Row 2: orchestration & misc
const ROW2: &[Element] = &[
    Element::AutopilotState,
    Element::Todos,
    Element::Agents,
    Element::Skills,
    Element::CallCounts,
    Element::Thinking,
    Element::Permissions,
    Element::Prd,
    Element::MissionBoard,
    Element::Cwd,
    Element::ApiKeySource,
    Element::BackgroundTasks,
];

/// Compute the visual (terminal column) width of a string that may contain
/// ANSI escape sequences.  Only handles the common `ESC [ ... m` form.
fn ansi_visual_width(s: &str) -> usize {
    let mut width = 0usize;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // skip until 'm' (SGR terminator)
            for ch in chars.by_ref() {
                if ch == 'm' {
                    break;
                }
            }
        } else {
            // Basic CJK double-width heuristic
            let cw = if (c as u32) > 0x2E7F { 2 } else { 1 };
            width += cw;
        }
    }
    width
}

fn make_row(elements: &[Element], ctx: &RenderContext<'_>, sep: &str) -> String {
    elements
        .iter()
        .filter_map(|e| render_element(*e, ctx))
        .filter(|v| !v.trim().is_empty())
        .collect::<Vec<_>>()
        .join(sep)
}

pub fn render_statusline(
    input: &Input,
    cache: &HudCache,
    color_level: ColorLevel,
    strings: &'static Strings,
    omc_state: &OmcState,
    usage: Option<&UsageData>,
    transcript: Option<&TranscriptData>,
    mission_board: Option<&MissionBoardState>,
    config: &HudConfig,
) -> String {
    let ctx = RenderContext {
        input,
        cache,
        color_level,
        strings,
        omc_state,
        usage,
        transcript,
        mission_board,
        config,
    };

    let sep = match color_level {
        ColorLevel::Mono => " | ",
        _ => "\x1b[2m | \x1b[0m",
    };

    let ctx_pct: Option<u8> = input
        .context_used_pct()
        .map(|p| p.clamp(0.0, 100.0) as u8);

    // Pet is enabled by default; disable via settings.json: { "omcHud": { "elements": { "pet": false } } }
    let pet_enabled = config
        .elements
        .as_ref()
        .and_then(|m| m.get("pet").copied())
        .unwrap_or(true);

    if !pet_enabled {
        // Flat mode: all elements in a single line separated by |
        use crate::elements::{DEFAULT_ELEMENTS, render_element};
        return DEFAULT_ELEMENTS
            .iter()
            .filter_map(|e| render_element(*e, &ctx))
            .filter(|v| !v.trim().is_empty())
            .collect::<Vec<_>>()
            .join(sep);
    }

    let pet_frame = pet::render_pet(ctx_pct, color_level);
    let pet_col = pet_frame.width;

    let row0 = make_row(ROW0, &ctx, sep);
    let row1 = make_row(ROW1, &ctx, sep);
    let row2 = make_row(ROW2, &ctx, sep);
    let info_rows = [row0, row1, row2];

    let mut out = String::new();
    for (i, (pet_line, info)) in pet_frame.lines.iter().zip(info_rows.iter()).enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let raw_pw = ansi_visual_width(pet_line);
        let padding = " ".repeat(pet_col.saturating_sub(raw_pw) + 2);
        out.push_str(pet_line);
        out.push_str(&padding);
        out.push_str(info);
    }
    out
}
