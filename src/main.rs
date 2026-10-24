mod achievements;
mod check;
mod display;
mod cli;
mod github;

use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    match cli.command {
        cli::Command::Check { username } => check::run(&username).await?,
        cli::Command::Hunt { lang, limit } => {
            println!("Hunting good first issues for {lang} (limit {limit})… (coming in next commit)");
        }
    }
    Ok(())
}
