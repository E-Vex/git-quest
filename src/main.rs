mod achievements;
mod check;
mod cli;
mod display;
mod github;
mod hunt;
mod validate;

use clap::Parser;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    let pal = display::Palette::resolve(cli.color);
    let result = match cli.command {
        cli::Command::Check { username } => check::run(&username, pal).await,
        cli::Command::Hunt { lang, limit, sort } => hunt::run(&lang, limit, &sort, pal).await,
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            // `{:#}` prints the error chain on one line and never a
            // backtrace, unlike the `{:?}` anyhow uses when returned
            // from `main`.
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}
