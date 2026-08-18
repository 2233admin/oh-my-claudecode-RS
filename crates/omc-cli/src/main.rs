use clap::Parser;
use omc_cli::{commands::Cli, dispatch};

fn main() {
    let result = std::thread::Builder::new()
        .name("omc-main".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(run)
        .expect("failed to start OMC CLI thread")
        .join();
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}

fn run() {
    let cli = Cli::parse();

    if let Err(err) = dispatch::run(cli) {
        eprintln!("omc: {err}");
        std::process::exit(1);
    }
}
