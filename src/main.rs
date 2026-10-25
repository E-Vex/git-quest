mod achievements;
mod check;
mod cli;
mod display;
mod github;
mod hunt;
mod readme;
mod validate;

use clap::Parser;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    let pal = display::Palette::resolve(cli.color);
    let result = match cli.command {
        cli::Command::Complete { shell: shell_type } => {
            use clap::CommandFactory;
            use clap_complete::generate;
            let mut cmd = cli::Cli::command();
            let name = cmd.get_name().to_string();
            generate(shell_type, &mut cmd, name, &mut std::io::stdout());
            Ok(())
        }
        cli::Command::Readme { username } => readme::run(username.as_deref().unwrap_or(""), pal).await,
        cli::Command::Check { username } => check::run(username.as_deref().unwrap_or(""), pal, cli.json).await,
        cli::Command::Hunt { lang, limit, label, sort } => hunt::run(&lang, limit, &label, &sort, pal, cli.json).await,
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
