use crate::cache::HudCache;
use crate::config::HudConfig;
use crate::elements::pet;
use crate::elements::{Element, RenderContext, render_element};
use crate::i18n::Strings;
use crate::input::Input;
use crate::mission_board::MissionBoardState;
use crate::omc_state::OmcState;
use crate::terminal::ColorLevel;
use crate::transcript::TranscriptData;
use crate::usage_api::UsageData;

// Row 0: identity + git info (default when gitInfoPosition = "above")
const ROW0: &[Element] = &[
    Element::OmcLabel,
    Element::Hostname,
    Element::Profile,
    Element::GitRepo,
    Element::GitBranch,
    Element::GitStatus,
    Element::ModelName,
    Element::ApiKeySource,
    Element::Cwd,
];

// Row 0 without git elements (used when gitInfoPosition = "inline")
const ROW0_NO_GIT: &[Element] = &[
    Element::OmcLabel,
    Element::Hostname,
    Element::Profile,
    Element::ModelName,
    Element::ApiKeySource,
    Element::Cwd,
];

// Git-only elements prepended to Row 1 when gitInfoPosition = "inline"
const GIT_ELEMENTS: &[Element] = &[Element::GitRepo, Element::GitBranch, Element::GitStatus];

// Row 1: critical metrics — context bar, timing, rate limits, cost
const ROW1: &[Element] = &[
    Element::ContextWarning,
    Element::Context,
    Element::ContextEta,
    Element::PromptTimeElapsed,
    Element::RateLimits,
    Element::EnterpriseCost,
    Element::Cost,
    Element::TokenUsage,
    Element::SessionStats,
    Element::SessionName,
    Element::SessionHealth,
    Element::Effort,
    Element::VimMode,
    Element::WorktreeInfo,
];

// Row 2: orchestration & misc
const ROW2: &[Element] = &[
    Element::AutopilotState,
    Element::Ralph,
    Element::Todos,
    Element::Agents,
    Element::Skills,
    Element::LastSkill,
    Element::CallCounts,
    Element::Thinking,
    Element::Permissions,
    Element::Prd,
    Element::MissionBoard,
    Element::LastTool,
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

#[allow(clippy::too_many_arguments)]
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

    let ctx_pct: Option<u8> = input.context_used_pct().map(|p| p.clamp(0.0, 100.0) as u8);

    // Pet is enabled by default; disable via settings.json: { "omcHud": { "elements": { "pet": false } } }
    let pet_enabled = config.element_enabled("pet", true);

    if !pet_enabled {
        // Flat mode: all elements in a single line separated by |
        use crate::elements::{DEFAULT_ELEMENTS, render_element};
        let line = DEFAULT_ELEMENTS
            .iter()
            .filter_map(|e| render_element(*e, &ctx))
            .filter(|v| !v.trim().is_empty())
            .collect::<Vec<_>>()
            .join(sep);
        return match config.max_width {
            Some(max_w) => truncate_ansi(&line, max_w as usize),
            None => line,
        };
    }

    let pet_frame = pet::render_pet(ctx_pct, color_level);
    let pet_col = pet_frame.width;

    // gitInfoPosition: "above" (default) keeps git on row0; "inline" moves it into row1
    let git_inline = ctx.config.element_str("gitInfoPosition", "above") == "inline";

    let row0 = if git_inline {
        make_row(ROW0_NO_GIT, &ctx, sep)
    } else {
        make_row(ROW0, &ctx, sep)
    };
    let row1 = if git_inline {
        let git_part = make_row(GIT_ELEMENTS, &ctx, sep);
        let main_part = make_row(ROW1, &ctx, sep);
        match (git_part.is_empty(), main_part.is_empty()) {
            (true, _) => main_part,
            (_, true) => git_part,
            (false, false) => format!("{git_part}{sep}{main_part}"),
        }
    } else {
        make_row(ROW1, &ctx, sep)
    };
    let row2 = make_row(ROW2, &ctx, sep);
    let info_rows = [row0, row1, row2];

    let max_width = config.max_width.map(|w| w as usize);
    let max_lines = config.max_output_lines();

    let mut lines: Vec<String> = Vec::with_capacity(3);
    for (pet_line, info) in pet_frame.lines.iter().zip(info_rows.iter()) {
        let raw_pw = ansi_visual_width(pet_line);
        let padding = " ".repeat(pet_col.saturating_sub(raw_pw) + 2);
        let line = format!("{pet_line}{padding}{info}");
        lines.push(line);
    }

    // Apply maxWidth truncation
    if let Some(max_w) = max_width {
        for line in &mut lines {
            *line = truncate_ansi(line, max_w);
        }
    }

    // Apply maxOutputLines
    if lines.len() > max_lines {
        lines.truncate(max_lines);
    }

    lines.join("\n")
}

/// Truncate a string that may contain ANSI escapes to at most `max_cols` visible columns.
fn truncate_ansi(s: &str, max_cols: usize) -> String {
    let visible = ansi_visual_width(s);
    if visible <= max_cols {
        return s.to_string();
    }
    // Walk chars, skip ANSI sequences, count visible width
    let mut out = String::new();
    let mut cols = 0usize;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Collect the whole escape sequence into out unconditionally
            let mut seq = String::from(c);
            for ch in chars.by_ref() {
                seq.push(ch);
                if ch == 'm' {
                    break;
                }
            }
            out.push_str(&seq);
            continue;
        }
        let w = if (c as u32) > 0x2E7F { 2 } else { 1 };
        if cols + w > max_cols.saturating_sub(3) {
            out.push_str("\x1b[0m...");
            break;
        }
        out.push(c);
        cols += w;
    }
    out
}
