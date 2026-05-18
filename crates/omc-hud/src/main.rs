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
    let mut stdin_buf = String::default();
    io::stdin()
        .read_to_string(&mut stdin_buf)
        .map_err(|e| format!("failed to read stdin: {e}"))?;

    // Debug: OMC_HUD_DUMP=1 writes raw stdin to %TEMP%\omc-hud-stdin.json
    if std::env::var_os("OMC_HUD_DUMP").is_some() {
        let dump = std::env::temp_dir().join("omc-hud-stdin.json");
        let _ = std::fs::write(&dump, &stdin_buf);
    }

    let input = input::parse_stdin_json(&stdin_buf);
    let mut cache = cache::load(&input);
    let now = cache::now_ms();
    cache.record_context(input.tokens_used(), now);

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
        .map_err(|e| format!("failed to write stdout: {e}"))?;

    cache::save(&input, &cache);
    Ok(())
}
