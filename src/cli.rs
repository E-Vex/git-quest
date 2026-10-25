use clap::{Parser, Subcommand};

/// git-quest — The missing CLI for GitHub Achievements.
///
/// Track your progress toward achievements like Pull Shark and
/// Starstruck, and hunt good first issues matching your stack.
#[derive(Parser)]
#[command(name = "git-quest", version, about, long_about = None)]
pub struct Cli {
    /// When to colorize output
    #[arg(long, global = true, value_enum, default_value_t = ColorWhen::Auto)]
    pub color: ColorWhen,
    /// Output raw JSON instead of formatted text
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Command,
}

/// Value of the global `--color` flag.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorWhen {
    /// Colors on a terminal, unless NO_COLOR is set
    Auto,
    /// Always emit ANSI colors, even when piped
    Always,
    /// Never emit ANSI colors
    Never,
}

#[derive(Subcommand)]
pub enum Command {
    /// Generate shell completion scripts
    Complete {
        /// Shell type (bash, zsh, fish, powershell, elvish)
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// Check a GitHub user's achievement progress
    Check {
        /// GitHub username to analyze (e.g. octocat); omit for the token owner
        #[arg(value_parser = crate::validate::username)]
        username: Option<String>,
    },
    /// Generate a progress block for your profile README
    Readme {
        /// GitHub username to analyze (e.g. octocat); omit for the token owner
        #[arg(value_parser = crate::validate::username)]
        username: Option<String>,
    },
    /// Hunt for open `good first issue` tasks matching your stack
    Hunt {
        /// Programming language to filter by (e.g. rust, go, python)
        #[arg(short = 'l', long, value_parser = crate::validate::language)]
        lang: String,
        /// Maximum number of issues to show (1-100)
        #[arg(
            short = 'n',
            long,
            default_value_t = 3,
            value_parser = clap::value_parser!(u8).range(1..=100)
        )]
        limit: u8,
        /// Label to filter by (e.g. good first issue, help wanted)
        #[arg(long, value_parser = crate::validate::label, default_value = "good first issue")]
        label: String,
        /// Sort results by community validation or freshness
        #[arg(short = 's', long, value_enum, default_value_t = SortBy::Reactions)]
        sort: SortBy,
    },
}

/// Sort criterion for `hunt` results.
#[derive(clap::ValueEnum, Clone, Debug)]
pub enum SortBy {
    /// Most 👍 reactions (community-validated)
    Reactions,
    /// Newest first
    Created,
    /// Recently updated
    Updated,
    /// Most discussed
    Comments,
}

impl SortBy {
    /// Map to the GitHub Search API `sort` value.
    pub fn as_query(&self) -> &'static str {
        match self {
            SortBy::Reactions => "reactions-+1",
            SortBy::Created => "created",
            SortBy::Updated => "updated",
            SortBy::Comments => "comments",
        }
    }
}
