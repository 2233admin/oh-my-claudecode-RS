mod cache;
mod config;
mod elements;
mod i18n;
mod input;
mod mission_board;
mod omc_state;
mod render;
mod terminal;
mod transcript;
mod usage_api;

use std::io::{self, Read, Write};

#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Animation frame interval in ms.
/// 80ms matches the Braille spinner frame rate (10-frame × 80ms = 800ms cycle).
const ANIM_MS: u64 = 80;

fn main() {
    if let Err(err) = run() {
        eprintln!("omc-hud: {err}");
    }
    std::process::exit(0);
}

fn run() -> Result<(), String> {
    let mut stdin_buf = String::default();
    io::stdin()
        .read_to_string(&mut stdin_buf)
        .map_err(|e| format!("failed to read stdin: {e}"))?;

    let input = input::parse_stdin_json(&stdin_buf);
    let mut cache = cache::load(&input);
    let now = cache::now_ms();
    cache.record_context(input.tokens_used(), now);
    cache::save(&input, &cache);

    let config = config::load();
    let omc_state = omc_state::OmcState::load(
        input.cwd.as_deref(),
        input.session_id.as_deref(),
    );
    let transcript_data = input
        .transcript_path
        .as_deref()
        .and_then(transcript::parse);
    let usage_data = usage_api::fetch(config.usage_api_poll_interval_ms);
    let mission_board_data = mission_board::load(input.cwd.as_deref());

    let locale = i18n::detect_locale();
    let strings = i18n::strings(locale);
    let color_level = elements::color_degrade::detect_color_level();

    let mut stdout = io::stdout();

    // Animation loop: re-render each frame using the same session data but a
    // fresh now_ms() so animation elements (spinner, cat tail/eyes) advance.
    // Break as soon as a write fails — Claude Code closed the pipe.
    loop {
        let output = render::render_statusline(
            &input,
            &cache,
            color_level,
            strings,
            &omc_state,
            usage_data.as_ref(),
            transcript_data.as_ref(),
            mission_board_data.as_ref(),
            &config,
        );

        if stdout.write_all(output.as_bytes()).is_err()
            || stdout.write_all(b"\n").is_err()
            || stdout.flush().is_err()
        {
            break;
        }

        std::thread::sleep(std::time::Duration::from_millis(ANIM_MS));
    }

    Ok(())
}
