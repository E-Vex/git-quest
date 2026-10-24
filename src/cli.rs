use clap::{Parser, Subcommand};

/// git-quest — The missing CLI for GitHub Achievements.
///
/// Track your progress toward achievements like Pull Shark and
/// Starstruck, and hunt good first issues matching your stack.
#[derive(Parser)]
#[command(name = "git-quest", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Check a GitHub user's achievement progress
    Check {
        /// GitHub username to analyze (e.g. octocat)
        username: String,
    },
    /// Hunt for open `good first issue` tasks matching your stack
    Hunt {
        /// Programming language to filter by (e.g. rust, go, python)
        #[arg(short, long)]
        lang: String,
        /// Maximum number of issues to show
        #[arg(short, long, default_value_t = 3)]
        limit: u8,
    },
}
