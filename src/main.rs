mod cli;
mod achievements;
mod github;

use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    match cli.command {
        cli::Command::Check { username } => {
            println!("Checking achievements for {username}… (coming in next commit)");
        }
        cli::Command::Hunt { lang, limit } => {
            println!("Hunting good first issues for {lang} (limit {limit})… (coming in next commit)");
        }
    }
    Ok(())
}
