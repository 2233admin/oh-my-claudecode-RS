mod cache;
mod elements;
mod i18n;
mod input;
mod render;
mod terminal;

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
    let mut stdin = String::new();
    io::stdin()
        .read_to_string(&mut stdin)
        .map_err(|err| format!("failed to read stdin: {err}"))?;

    let input = input::parse_stdin_json(&stdin);
    let mut cache = cache::load(&input);
    let now = cache::now_ms();
    cache.record_context(input.context_window_tokens, now);

    let locale = i18n::detect_locale();
    let strings = i18n::strings(locale);
    let color_level = elements::color_degrade::detect_color_level();
    let output = render::render_statusline(&input, &cache, color_level, strings);

    let mut stdout = io::stdout();
    stdout
        .write_all(output.as_bytes())
        .and_then(|_| stdout.write_all(b"\n"))
        .map_err(|err| format!("failed to write stdout: {err}"))?;

    cache::save(&input, &cache);
    Ok(())
}
