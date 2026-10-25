mod achievements;
mod check;
mod cli;
mod display;
mod github;
mod hunt;
mod validate;

use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    match cli.command {
        cli::Command::Check { username } => check::run(&username).await?,
        cli::Command::Hunt { lang, limit, sort } => hunt::run(&lang, limit, &sort).await?,
    }
    Ok(())
}
