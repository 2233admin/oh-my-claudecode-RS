use clap::Parser;
use omc_cli::{commands::Cli, dispatch};

fn main() {
    let cli = Cli::parse();

    if let Err(err) = dispatch::run(cli) {
        eprintln!("omc: {err}");
        std::process::exit(1);
    }
}
