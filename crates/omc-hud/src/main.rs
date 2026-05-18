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

fn main() {
    if let Err(err) = run() {
        eprintln!("omc-hud: {err}");
    }
    std::process::exit(0);
}

fn run() -> Result<(), String> {
    let mut stdin = String::default();
    io::stdin()
        .read_to_string(&mut stdin)
        .map_err(|err| format!("failed to read stdin: {err}"))?;

    let input = input::parse_stdin_json(&stdin);
    let mut cache = cache::load(&input);
    let now = cache::now_ms();
    cache.record_context(input.tokens_used(), now);

    // Load config
    let config = config::load();

    // Load omc state files
    let omc_state = omc_state::OmcState::load(
        input.cwd.as_deref(),
        input.session_id.as_deref(),
    );

    // Parse transcript (best-effort, silent failure)
    let transcript_data = input
        .transcript_path
        .as_deref()
        .and_then(transcript::parse);

    // Fetch usage API data (cached, non-blocking via file cache)
    let usage_data = usage_api::fetch(config.usage_api_poll_interval_ms);

    // Load mission board
    let mission_board_data = mission_board::load(input.cwd.as_deref());

    let locale = i18n::detect_locale();
    let strings = i18n::strings(locale);
    let color_level = elements::color_degrade::detect_color_level();

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

    let mut stdout = io::stdout();
    stdout
        .write_all(output.as_bytes())
        .and_then(|_| stdout.write_all(b"\n"))
        .map_err(|err| format!("failed to write stdout: {err}"))?;

    cache::save(&input, &cache);
    Ok(())
}
