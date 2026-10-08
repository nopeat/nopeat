use std::process::ExitCode;

use clap::Parser;

mod bench;
mod budget;
mod cli_args;
mod discover;
mod payload;
mod pipeline;
mod report;
mod server;

use cli_args::Cli;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match pipeline::run(&cli) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("nopeat: {err:#}");
            ExitCode::from(3)
        }
    }
}
