use crate::achievements::{Progress, Tier};
use crate::github::IssueItem;
use owo_colors::OwoColorize;
use std::fmt::Write as _;

const BAR_WIDTH: usize = 12;

/// Decides whether output is decorated with ANSI colors.
///
/// Passed explicitly (instead of reading the environment in every helper)
/// so rendering stays a pure function that is easy to test.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    enabled: bool,
}

impl Palette {
    pub const fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    /// Honor the de-facto standard `NO_COLOR` env var.
    pub fn detect() -> Self {
        Self::new(std::env::var("NO_COLOR").is_err())
    }

    fn bold(self, s: impl Into<String>) -> String {
        let s = s.into();
        if self.enabled {
            s.bold().to_string()
        } else {
            s
        }
    }

    fn dim(self, s: impl Into<String>) -> String {
        let s = s.into();
        if self.enabled {
            s.dimmed().to_string()
        } else {
            s
        }
    }

    fn cyan(self, s: impl Into<String>) -> String {
        let s = s.into();
        if self.enabled {
            s.cyan().to_string()
        } else {
            s
        }
    }

    /// Colorize by proximity to the next tier.
    fn paint(self, s: impl Into<String>, frac: f64) -> String {
        let s = s.into();
        if !self.enabled {
            return s;
        }
        if frac >= 1.0 {
            s.bright_green().to_string()
        } else if frac >= 0.75 {
            s.green().to_string()
        } else if frac >= 0.5 {
            s.yellow().to_string()
        } else if frac >= 0.25 {
            s.bright_red().to_string()
        } else {
            s.red().to_string()
        }
    }
}

fn bar(frac: f64) -> String {
    let filled = (frac * BAR_WIDTH as f64).round() as usize;
    let filled = filled.min(BAR_WIDTH);
    let empty = BAR_WIDTH - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

/// Render the `check` command results.
pub fn render_check(username: &str, items: &[Progress], has_token: bool, pal: Palette) -> String {
    let mut out = String::new();
    out.push('\n');
    let _ = writeln!(
        out,
        "{}",
        pal.bold(format!("Achievement progress for {username}"))
    );
    out.push('\n');
    for p in items {
        let name = format!("{:<19}", p.def.name);
        let frac = p.fraction();
        let (status, remaining) = match p.next_tier() {
            Some((tier, threshold)) => (
                match tier {
                    Tier::Default => format!("{}/{} to unlock", p.current, threshold),
                    _ => format!("{}/{} to {}", p.current, threshold, tier),
                },
                format!("{} {} remaining", p.remaining(), p.def.unit),
            ),
            None => (
                "GOLD COMPLETE".to_string(),
                "🏆 every tier achieved".to_string(),
            ),
        };
        let remaining = match &p.detail {
            Some(detail) => format!("{remaining} · {detail}"),
            None => remaining,
        };
        let _ = writeln!(
            out,
            "{} {}  {:<18}{}  {}",
            p.def.emoji,
            name,
            pal.paint(status, frac),
            pal.paint(bar(frac), frac),
            pal.dim(remaining)
        );
    }
    out.push('\n');
    let _ = writeln!(
        out,
        "{}",
        pal.cyan("💡 Tip: hunt contribution opportunities with `git-quest hunt --lang <lang>`")
    );
    if !has_token {
        let _ = writeln!(
            out,
            "{}",
            pal.dim("💡 Set GITHUB_TOKEN to track Galaxy Brain and lift rate limits to 5000 req/h")
        );
    }
    out.push('\n');
    out
}

/// Render the `hunt` command results.
pub fn render_hunt(lang: &str, total: u64, issues: &[IssueItem], pal: Palette) -> String {
    let mut out = String::new();
    out.push('\n');
    let _ = writeln!(
        out,
        "{}",
        pal.bold(format!("🎯 Hunt — good first issues for `{lang}`"))
    );
    out.push('\n');
    if issues.is_empty() {
        let _ = writeln!(
            out,
            "{}",
            pal.dim("No open `good first issue` tasks found for this language.")
        );
        out.push('\n');
        return out;
    }
    for (i, it) in issues.iter().enumerate() {
        let date = it.created_at.get(..10).unwrap_or(it.created_at.as_str());
        let _ = writeln!(out, " {}. {}", i + 1, pal.bold(it.title.clone()));
        let _ = writeln!(
            out,
            "    {}  {}  {}  {}",
            pal.cyan(format!("📦 {}", it.repo)),
            pal.dim(format!("👍 {}", it.reactions)),
            pal.dim(format!("💬 {}", it.comments)),
            pal.dim(format!("📅 {date}")),
        );
        let _ = writeln!(out, "    {}", pal.dim(it.url.clone()));
        out.push('\n');
    }
    let _ = writeln!(
        out,
        "{}",
        pal.dim(format!(
            "🎣 {total} open issues match — showing top {}",
            issues.len()
        ))
    );
    let _ = writeln!(
        out,
        "{}",
        pal.cyan("💡 Read the repo's CONTRIBUTING.md first, then ship a small, focused PR. Good luck! 🚀")
    );
    out.push('\n');
    out
}
