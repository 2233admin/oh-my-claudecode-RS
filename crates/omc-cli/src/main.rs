mod commands;

use clap::Parser;
use commands::Cli;

mod dispatch;

fn main() {
    let cli = Cli::parse();

    if let Err(err) = dispatch::run(cli) {
        eprintln!("omc: {err}");
        std::process::exit(1);
    }
}
