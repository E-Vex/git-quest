use crate::achievements::Progress;
use owo_colors::OwoColorize;

const BAR_WIDTH: usize = 12;

/// Honor the de-facto standard `NO_COLOR` env var.
fn colors_enabled() -> bool {
    std::env::var("NO_COLOR").is_err()
}

/// Colorize by proximity to the next tier.
fn paint(s: String, frac: f64) -> String {
    if !colors_enabled() {
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

fn dim(s: String) -> String {
    if colors_enabled() {
        s.dimmed().to_string()
    } else {
        s
    }
}

fn cyan(s: String) -> String {
    if colors_enabled() {
        s.cyan().to_string()
    } else {
        s
    }
}

fn bar(frac: f64) -> String {
    let filled = (frac * BAR_WIDTH as f64).round() as usize;
    let filled = filled.min(BAR_WIDTH);
    let empty = BAR_WIDTH - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

/// Render the `check` command results.
pub fn render_check(username: &str, items: &[Progress], has_token: bool) {
    println!();
    println!("{}", format!("Achievement progress for {username}").bold());
    println!();
    for p in items {
        let name = format!("{:<19}", p.def.name);
        let frac = p.fraction();
        let (status, remaining) = match p.next_tier() {
            Some((tier, threshold)) => (
                format!("{}/{} to {}", p.current, threshold, tier),
                format!("{} {} remaining", p.remaining(), p.def.unit),
            ),
            None => (
                "GOLD COMPLETE".to_string(),
                "🏆 every tier achieved".to_string(),
            ),
        };
        println!(
            "{} {}  {:<18}{}  {}",
            p.def.emoji,
            name,
            paint(status, frac),
            paint(bar(frac), frac),
            dim(remaining)
        );
    }
    println!();
    println!(
        "{}",
        cyan("💡 Tip: hunt contribution opportunities with `git-quest hunt --lang <lang>`".into())
    );
    if !has_token {
        println!(
            "{}",
            dim("💡 Set GITHUB_TOKEN to track Galaxy Brain and lift rate limits to 5000 req/h".into())
        );
    }
    println!();
}
