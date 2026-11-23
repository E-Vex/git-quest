use crate::achievements::{Progress, Tier};
use crate::cli::ColorWhen;
use crate::github::IssueItem;
use crate::progress::ProgressReport;
use owo_colors::OwoColorize;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::io::IsTerminal;
use std::time::Duration;

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

    /// Resolve the `--color` flag into a palette.
    pub fn resolve(when: ColorWhen) -> Self {
        match when {
            ColorWhen::Always => Self::new(true),
            ColorWhen::Never => Self::new(false),
            ColorWhen::Auto => Self::detect(),
        }
    }

    /// Auto-detect: colors only on a terminal, and never when the
    /// `NO_COLOR` convention (<https://no-color.org>) asks for none.
    fn detect() -> Self {
        Self::new(colors_allowed(
            std::env::var_os("NO_COLOR").as_deref(),
            std::io::stdout().is_terminal(),
        ))
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

    fn green(self, s: impl Into<String>) -> String {
        let s = s.into();
        if self.enabled {
            s.bright_green().to_string()
        } else {
            s
        }
    }

    fn red(self, s: impl Into<String>) -> String {
        let s = s.into();
        if self.enabled {
            s.bright_red().to_string()
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

/// Pure decision behind [`Palette::detect`].
///
/// Per the NO_COLOR spec the variable only counts when it is set to a
/// *non-empty* value.
fn colors_allowed(no_color: Option<&OsStr>, is_terminal: bool) -> bool {
    let disabled = no_color.is_some_and(|v| !v.is_empty());
    is_terminal && !disabled
}

fn bar(frac: f64) -> String {
    let filled = (frac * BAR_WIDTH as f64).round() as usize;
    let filled = filled.min(BAR_WIDTH);
    let empty = BAR_WIDTH - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

/// Status text for the next tier: `"72/128 to Silver"`,
/// `"0/16 to unlock"` or `"GOLD COMPLETE"`.
fn next_status(p: &Progress) -> String {
    match p.next_tier() {
        Some((tier, threshold)) => match tier {
            Tier::Default => format!("{}/{} to unlock", p.current, threshold),
            _ => format!("{}/{} to {tier}", p.current, threshold),
        },
        None => "GOLD COMPLETE".to_string(),
    }
}

/// Remaining-count text: `"56 merged PRs remaining"` or the
/// gold-complete celebration.
fn remaining_line(p: &Progress) -> String {
    match p.next_tier() {
        Some(_) => format!("{} {} remaining", p.remaining(), p.def.unit),
        None => "🏆 every tier achieved".to_string(),
    }
}

/// Render the `check` command results.
pub fn render_check(report: &ProgressReport, pal: Palette) -> String {
    let mut out = String::new();
    out.push('\n');
    let _ = writeln!(
        out,
        "{}",
        pal.bold(format!("Achievement progress for {}", report.username))
    );
    out.push('\n');
    for p in &report.items {
        let name = format!("{:<19}", p.def.name);
        let frac = p.fraction();
        let status = next_status(p);
        let mut remaining = remaining_line(p);
        if let Some(detail) = &p.detail {
            remaining = format!("{remaining} · {detail}");
        }
        let _ = writeln!(
            out,
            "{} {}  {}{}  {}",
            p.def.emoji,
            name,
            // Pad *before* coloring: width specifiers count the invisible
            // ANSI escape bytes, which collapsed the gap before the bar.
            pal.paint(format!("{status:<18}"), frac),
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
    if !report.has_token {
        let _ = writeln!(
            out,
            "{}",
            pal.dim("💡 Set GITHUB_TOKEN to track Galaxy Brain and lift rate limits to 5000 req/h")
        );
    }
    out.push('\n');
    out
}

/// Render a markdown progress block suitable for a profile README.
pub fn render_readme(report: &ProgressReport) -> String {
    let mut out = String::new();
    out.push_str("<!-- git-quest -->\n");
    out.push_str(&format!("## GitHub Achievements — {}\n\n", report.username));
    for p in &report.items {
        let frac = p.fraction();
        let bar_str = bar(frac);
        // One clean status line, mirroring `render_check`: the count is
        // never repeated and the tier name appears exactly once.
        let status = next_status(p);
        let status = if p.next_tier().is_none() {
            format!("{status} 🏆")
        } else {
            status
        };
        let remaining = match p.next_tier() {
            Some(_) => remaining_line(p),
            None => "every tier achieved".to_string(),
        };
        let detail_part = match &p.detail {
            Some(detail) => format!(" · {detail}"),
            None => String::new(),
        };
        out.push_str(&format!(
            "- **{}** `{}` — `{status}` — `{bar_str}` — {remaining}{detail_part}\n",
            p.def.emoji, p.def.name,
        ));
    }
    out.push_str("<!-- /git-quest -->\n");
    out
}

/// Humanize a refresh interval: `42s`, `5m`, `2h`.
pub fn format_interval(d: Duration) -> String {
    let secs = d.as_secs();
    if secs >= 3600 && secs % 3600 == 0 {
        format!("{}h", secs / 3600)
    } else if secs >= 60 && secs % 60 == 0 {
        format!("{}m", secs / 60)
    } else {
        format!("{secs}s")
    }
}

/// The list part of `track` and `track --watch`: the numbered tiers within
/// reach, then everything further out. `deltas` marks achievements whose
/// count changed since the previous watch refresh; an empty slice renders
/// the exact one-shot layout.
fn track_body(
    within: &[&Progress],
    further: &[&Progress],
    deltas: &[(String, i64)],
    pal: Palette,
) -> String {
    let mut out = String::new();
    let delta_for = |name: &str| deltas.iter().find(|(n, _)| n == name).map(|(_, d)| *d);

    for (i, p) in within.iter().enumerate() {
        let chip = match delta_for(p.def.name) {
            Some(d) if d > 0 => format!(" {}", pal.green(format!("↑{d}"))),
            Some(d) => format!(" {}", pal.red(format!("↓{}", -d))),
            None => String::new(),
        };
        let _ = writeln!(
            out,
            " {}. {} {}{} {} ({})",
            i + 1,
            p.def.emoji,
            pal.bold(p.def.name),
            chip,
            pal.paint(remaining_line(p), p.fraction()),
            next_status(p)
        );
    }

    if !within.is_empty() && !further.is_empty() {
        out.push('\n');
        let _ = writeln!(out, "{}", pal.dim("Further out:"));
        for p in further {
            let _ = writeln!(
                out,
                " {} {} {} ({})",
                pal.dim(p.def.emoji),
                pal.dim(p.def.name),
                pal.dim(remaining_line(p)),
                pal.dim(next_status(p))
            );
        }
    } else if within.is_empty() {
        // Nothing within reach: number the closest targets instead.
        for (i, p) in further.iter().enumerate() {
            let chip = match delta_for(p.def.name) {
                Some(d) if d > 0 => format!(" {}", pal.green(format!("↑{d}"))),
                Some(d) => format!(" {}", pal.red(format!("↓{}", -d))),
                None => String::new(),
            };
            let _ = writeln!(
                out,
                " {}. {} {}{} {} ({})",
                i + 1,
                p.def.emoji,
                pal.bold(p.def.name),
                chip,
                pal.paint(remaining_line(p), p.fraction()),
                next_status(p)
            );
        }
    }
    out
}

/// The closing tip: where to hunt when nothing is close, momentum when
/// something is.
fn track_tip(within_is_empty: bool, pal: Palette) -> String {
    if within_is_empty {
        pal.cyan("💡 Tip: hunt contribution opportunities with `git-quest hunt --lang <lang>`")
    } else {
        pal.cyan("💡 Ship a small, focused PR today — the closest tier is within your grasp. 🚀")
    }
}

/// Render the `track` command results: the tiers within reach first,
/// then everything else as targets further out.
pub fn render_track(report: &ProgressReport, reach: u8, pal: Palette) -> String {
    let mut out = String::new();
    out.push('\n');

    if report.items.iter().all(|p| p.next_tier().is_none()) {
        let _ = writeln!(
            out,
            "{}",
            pal.bold(format!(
                "🏆 Every tracked achievement is GOLD COMPLETE for {}",
                report.username
            ))
        );
        out.push('\n');
        return out;
    }

    let (within, further) = report.tiers_by_proximity(reach);
    let header = if within.is_empty() {
        format!(
            "🎯 Track — nothing within reach (≤{reach} more) for {}",
            report.username
        )
    } else {
        format!(
            "🎯 Track — tiers within reach (≤{reach} more) for {}",
            report.username
        )
    };
    let _ = writeln!(out, "{}", pal.bold(header));
    out.push('\n');

    out.push_str(&track_body(&within, &further, &[], pal));

    out.push('\n');
    let _ = writeln!(out, "{}", track_tip(within.is_empty(), pal));
    out.push('\n');
    out
}

/// Render one `track --watch` frame.
///
/// `countdown` is `Some(seconds until the next refresh)` on live redraws;
/// `None` renders the static "refreshing every …" line for piped output.
/// When `error` is set, the frame keeps the previous snapshot on screen and
/// explains why it did not move.
pub fn render_watch(
    report: Option<&ProgressReport>,
    reach: u8,
    deltas: &[(String, i64)],
    error: Option<&str>,
    interval: Duration,
    countdown: Option<u64>,
    pal: Palette,
) -> String {
    let mut out = String::new();
    out.push('\n');
    let wait_line = match countdown {
        Some(secs) => format!("next refresh in {secs}s · Ctrl+C to stop"),
        None => format!(
            "refreshing every {} · Ctrl+C to stop",
            format_interval(interval)
        ),
    };

    match report {
        None => {
            let _ = writeln!(
                out,
                "{}",
                pal.bold("🎯 Track — waiting for the first refresh")
            );
            let _ = writeln!(
                out,
                "{}",
                pal.red(format!(
                    "⚠ {}",
                    error.unwrap_or("the first refresh has not completed yet")
                ))
            );
        }
        Some(r) if r.items.iter().all(|p| p.next_tier().is_none()) => {
            let _ = writeln!(
                out,
                "{}",
                pal.bold(format!(
                    "🏆 Watch — every tracked achievement is GOLD COMPLETE for {}",
                    r.username
                ))
            );
            let _ = writeln!(out, "{}", pal.dim(wait_line));
        }
        Some(r) => {
            let _ = writeln!(
                out,
                "{}",
                pal.bold(format!(
                    "🎯 Track — watching {} (reach ≤{}, every {})",
                    r.username,
                    reach,
                    format_interval(interval)
                ))
            );
            let _ = writeln!(out, "{}", pal.dim(wait_line));
            // Interactive screens are cleared every redraw, which would
            // wipe the stderr warnings fetch() prints — surface partial
            // state in the frame itself.
            if !r.errors.is_empty() {
                let _ = writeln!(
                    out,
                    "{}",
                    pal.red(format!("⚠ partial data — {}", r.errors.join("; ")))
                );
            }
            if let Some(err) = error {
                let _ = writeln!(
                    out,
                    "{}",
                    pal.red(format!(
                        "⚠ last refresh failed — showing the previous snapshot: {err}"
                    ))
                );
            }
            let (within, further) = r.tiers_by_proximity(reach);
            out.push('\n');
            out.push_str(&track_body(&within, &further, deltas, pal));
            out.push('\n');
            let _ = writeln!(out, "{}", track_tip(within.is_empty(), pal));
        }
    }
    out.push('\n');
    out
}

/// Render the `hunt` command results.
pub fn render_hunt(
    lang: &str,
    label: &str,
    total: u64,
    issues: &[IssueItem],
    pal: Palette,
) -> String {
    let mut out = String::new();
    out.push('\n');
    let _ = writeln!(
        out,
        "{}",
        pal.bold(format!("🎯 Hunt — `{label}` tasks for `{lang}`"))
    );
    out.push('\n');
    if issues.is_empty() {
        let _ = writeln!(
            out,
            "{}",
            pal.dim("No open tasks found for this label and language.")
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::achievements::{AchievementDef, PAIR_EXTRAORDINAIRE, PULL_SHARK, STARSTRUCK};

    fn plain() -> Palette {
        Palette::new(false)
    }

    fn progress(def: &'static AchievementDef, current: u64) -> Progress {
        Progress {
            def,
            current,
            detail: None,
        }
    }

    fn report(has_token: bool, items: Vec<Progress>) -> ProgressReport {
        ProgressReport {
            username: "octocat".into(),
            has_token,
            items,
            errors: Vec::new(),
            failed: Vec::new(),
        }
    }

    fn issue(n: u64) -> IssueItem {
        IssueItem {
            title: format!("Issue {n}"),
            url: format!("https://github.com/o/r/issues/{n}"),
            repo: "o/r".into(),
            comments: 3,
            reactions: 7,
            created_at: "2024-05-06T07:08:09Z".into(),
        }
    }

    #[test]
    fn bar_is_empty_at_zero_and_full_at_one() {
        assert_eq!(bar(0.0), "░".repeat(BAR_WIDTH));
        assert_eq!(bar(1.0), "█".repeat(BAR_WIDTH));
    }

    #[test]
    fn bar_always_has_constant_width() {
        for frac in [0.0, 0.04, 0.25, 0.5, 0.96, 1.0] {
            assert_eq!(bar(frac).chars().count(), BAR_WIDTH, "frac {frac}");
        }
    }

    #[test]
    fn bar_rounds_to_the_nearest_cell() {
        assert_eq!(bar(0.5).matches('█').count(), 6);
        assert_eq!(bar(0.04).matches('█').count(), 0);
        assert_eq!(bar(0.96).matches('█').count(), 12);
    }

    #[test]
    fn plain_output_contains_no_escape_sequences() {
        let items = [progress(&PULL_SHARK, 72)];
        assert!(!render_check(&report(false, items.to_vec()), plain()).contains('\x1b'));
        assert!(!render_hunt("rust", "good first issue", 1, &[issue(1)], plain()).contains('\x1b'));
    }

    #[test]
    fn colored_output_emits_escape_sequences() {
        let items = [progress(&PULL_SHARK, 72)];
        assert!(render_check(&report(false, items.to_vec()), Palette::new(true)).contains("\x1b["));
    }

    #[test]
    fn check_shows_next_tier_and_remaining_count() {
        let items = [progress(&PULL_SHARK, 72)];
        let out = render_check(&report(true, items.to_vec()), plain());
        assert!(out.contains("Achievement progress for octocat"));
        assert!(out.contains("Pull Shark"));
        assert!(out.contains("72/128 to Silver"));
        assert!(out.contains("56 merged PRs remaining"));
    }

    #[test]
    fn check_calls_the_first_level_unlock() {
        let items = [progress(&STARSTRUCK, 0)];
        let out = render_check(&report(true, items.to_vec()), plain());
        assert!(out.contains("0/16 to unlock"), "{out}");
    }

    #[test]
    fn check_reports_gold_as_complete() {
        let items = [progress(&PULL_SHARK, 5000)];
        let out = render_check(&report(true, items.to_vec()), plain());
        assert!(out.contains("GOLD COMPLETE"));
        assert!(out.contains("every tier achieved"));
    }

    #[test]
    fn check_prints_the_detail_next_to_the_remaining_count() {
        let items = [Progress {
            detail: Some("best repo: octocat/hello".into()),
            ..progress(&STARSTRUCK, 4)
        }];
        let out = render_check(&report(true, items.to_vec()), plain());
        assert!(out.contains("12 stars remaining · best repo: octocat/hello"));
    }

    #[test]
    fn check_hints_at_a_token_only_when_there_is_none() {
        let items = [progress(&PULL_SHARK, 1)];
        assert!(render_check(&report(false, items.to_vec()), plain()).contains("GITHUB_TOKEN"));
        assert!(!render_check(&report(true, items.to_vec()), plain()).contains("GITHUB_TOKEN"));
    }

    #[test]
    fn check_renders_every_item_on_its_own_line() {
        let items = [
            progress(&PULL_SHARK, 1),
            progress(&STARSTRUCK, 1),
            progress(&PAIR_EXTRAORDINAIRE, 1),
        ];
        let out = render_check(&report(true, items.to_vec()), plain());
        for name in ["Pull Shark", "Starstruck", "Pair Extraordinaire"] {
            assert_eq!(
                out.lines().filter(|l| l.contains(name)).count(),
                1,
                "{name}"
            );
        }
    }

    #[test]
    fn hunt_without_results_says_so() {
        let out = render_hunt("rust", "good first issue", 0, &[], plain());
        assert!(out.contains("`good first issue` tasks for `rust`"));
        assert!(out.contains("No open tasks found"));
    }

    #[test]
    fn readme_names_the_user_and_wraps_in_markers() {
        let items = [progress(&PULL_SHARK, 72)];
        let out = render_readme(&report(true, items.to_vec()));
        assert!(out.starts_with("<!-- git-quest -->\n"));
        assert!(out.contains("## GitHub Achievements — octocat"));
        assert!(out.trim_end().ends_with("<!-- /git-quest -->"));
    }

    #[test]
    fn readme_status_is_not_duplicated_or_garbled() {
        let items = [progress(&PULL_SHARK, 72)];
        let out = render_readme(&report(true, items.to_vec()));
        // The old renderer produced "72/128 to 128 to Silver"; each tier
        // name and threshold must appear exactly once per line.
        assert!(out.contains("72/128 to Silver"), "{out}");
        assert!(!out.contains("to 128 to"), "{out}");
        assert!(out.contains("56 merged PRs remaining"), "{out}");
    }

    #[test]
    fn readme_uses_unlock_wording_for_the_default_tier() {
        let items = [progress(&STARSTRUCK, 4)];
        let out = render_readme(&report(true, items.to_vec()));
        assert!(out.contains("4/16 to unlock"), "{out}");
        assert!(out.contains("12 stars remaining"), "{out}");
    }

    #[test]
    fn readme_reports_gold_complete_and_details() {
        let items = [
            progress(&PULL_SHARK, 5000),
            Progress {
                detail: Some("best repo: octocat/hello".into()),
                ..progress(&STARSTRUCK, 4)
            },
        ];
        let out = render_readme(&report(true, items.to_vec()));
        assert!(out.contains("GOLD COMPLETE 🏆"), "{out}");
        assert!(out.contains("· best repo: octocat/hello"), "{out}");
    }

    #[test]
    fn hunt_numbers_issues_and_summarises_the_total() {
        let out = render_hunt(
            "rust",
            "good first issue",
            42,
            &[issue(1), issue(2)],
            plain(),
        );
        assert!(out.contains(" 1. Issue 1"));
        assert!(out.contains(" 2. Issue 2"));
        assert!(out.contains("📦 o/r"));
        assert!(out.contains("👍 7"));
        assert!(out.contains("💬 3"));
        assert!(out.contains("📅 2024-05-06"));
        assert!(out.contains("https://github.com/o/r/issues/1"));
        assert!(out.contains("42 open issues match — showing top 2"));
    }

    #[test]
    fn hunt_tolerates_malformed_timestamps() {
        for created_at in ["", "2024", "é"] {
            let mut it = issue(1);
            it.created_at = created_at.into();
            let _ = render_hunt("rust", "good first issue", 1, &[it], plain());
        }
    }

    /// Remove SGR escape sequences (`ESC [ … m`).
    fn strip_ansi(s: &str) -> String {
        let mut out = String::new();
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                for d in chars.by_ref() {
                    if d == 'm' {
                        break;
                    }
                }
            } else {
                out.push(c);
            }
        }
        out
    }

    #[test]
    fn track_lists_tiers_within_reach_first() {
        let items = [
            progress(&PULL_SHARK, 11),         // 5 remaining → within reach of 5
            progress(&STARSTRUCK, 0),          // 16 remaining → further out
            progress(&PAIR_EXTRAORDINAIRE, 8), // 2 remaining → within reach, closest
        ];
        let out = render_track(&report(true, items.to_vec()), 5, plain());
        assert!(
            out.contains("tiers within reach (≤5 more) for octocat"),
            "{out}"
        );
        assert!(out.contains(" 1. 👥 Pair Extraordinaire"), "{out}");
        assert!(out.contains(" 2. 🦈 Pull Shark"), "{out}");
        assert!(
            out.contains("2 co-authored commits remaining (8/10 to Bronze)"),
            "{out}"
        );
        assert!(out.contains("Further out:"), "{out}");
        assert!(out.contains("⭐ Starstruck"), "{out}");
    }

    #[test]
    fn track_says_when_nothing_is_within_reach() {
        let items = [progress(&PULL_SHARK, 0)];
        let out = render_track(&report(true, items.to_vec()), 1, plain());
        assert!(out.contains("nothing within reach (≤1 more)"), "{out}");
        assert!(out.contains(" 1. 🦈 Pull Shark"), "{out}");
        assert!(
            out.contains("2 merged PRs remaining (0/2 to unlock)"),
            "{out}"
        );
    }

    #[test]
    fn track_celebrates_a_gold_complete_report() {
        let items = [progress(&PULL_SHARK, 5000)];
        let out = render_track(&report(true, items.to_vec()), 5, plain());
        assert!(
            out.contains("Every tracked achievement is GOLD COMPLETE"),
            "{out}"
        );
    }

    #[test]
    fn track_excludes_gold_from_the_lists() {
        let items = [progress(&PULL_SHARK, 5000), progress(&STARSTRUCK, 12)];
        let out = render_track(&report(true, items.to_vec()), 5, plain());
        assert!(out.contains("tiers within reach"), "{out}");
        assert!(out.contains("⭐ Starstruck"), "{out}");
        assert!(!out.contains("GOLD COMPLETE"), "{out}");
    }

    #[test]
    fn track_output_contains_no_escape_sequences_when_plain() {
        let items = [progress(&PULL_SHARK, 11), progress(&STARSTRUCK, 5000)];
        assert!(!render_track(&report(true, items.to_vec()), 5, plain()).contains('\x1b'));
    }

    #[test]
    fn colors_never_change_the_layout() {
        let items = [
            progress(&PULL_SHARK, 72),
            progress(&STARSTRUCK, 0),
            progress(&PAIR_EXTRAORDINAIRE, 4),
            progress(&PULL_SHARK, 5000),
        ];
        let colored = render_check(&report(false, items.to_vec()), Palette::new(true));
        let plain_out = render_check(&report(false, items.to_vec()), plain());
        assert_eq!(strip_ansi(&colored), plain_out);

        let colored = render_hunt(
            "rust",
            "good first issue",
            2,
            &[issue(1), issue(2)],
            Palette::new(true),
        );
        let plain_out = render_hunt(
            "rust",
            "good first issue",
            2,
            &[issue(1), issue(2)],
            plain(),
        );
        assert_eq!(strip_ansi(&colored), plain_out);

        let track_items = [progress(&PULL_SHARK, 11), progress(&STARSTRUCK, 0)];
        let colored = render_track(&report(false, track_items.to_vec()), 5, Palette::new(true));
        let plain_out = render_track(&report(false, track_items.to_vec()), 5, plain());
        assert_eq!(strip_ansi(&colored), plain_out);
    }

    #[test]
    fn colors_need_a_terminal() {
        assert!(colors_allowed(None, true));
        assert!(!colors_allowed(None, false));
    }

    #[test]
    fn no_color_disables_colors_unless_empty() {
        assert!(!colors_allowed(Some(OsStr::new("1")), true));
        assert!(!colors_allowed(Some(OsStr::new("anything")), true));
        assert!(colors_allowed(Some(OsStr::new("")), true));
    }

    #[test]
    fn explicit_color_choice_overrides_detection() {
        assert!(Palette::resolve(ColorWhen::Always).enabled);
        assert!(!Palette::resolve(ColorWhen::Never).enabled);
    }

    // --- track --watch frames ---

    fn watch_deltas() -> Vec<(String, i64)> {
        vec![("Pair Extraordinaire".to_string(), 2)]
    }

    #[test]
    fn format_interval_is_humanized() {
        assert_eq!(format_interval(Duration::from_secs(42)), "42s");
        assert_eq!(format_interval(Duration::from_secs(59)), "59s");
        assert_eq!(format_interval(Duration::from_secs(60)), "1m");
        assert_eq!(format_interval(Duration::from_secs(300)), "5m");
        assert_eq!(format_interval(Duration::from_secs(180)), "3m");
        assert_eq!(format_interval(Duration::from_secs(3600)), "1h");
        assert_eq!(format_interval(Duration::from_secs(7200)), "2h");
        // Non-round values stay in seconds.
        assert_eq!(format_interval(Duration::from_secs(61)), "61s");
        assert_eq!(format_interval(Duration::from_secs(3661)), "3661s");
    }

    #[test]
    fn watch_frame_shows_header_countdown_and_delta_chip() {
        let items = [progress(&PULL_SHARK, 11), progress(&PAIR_EXTRAORDINAIRE, 8)];
        let out = render_watch(
            Some(&report(true, items.to_vec())),
            5,
            &watch_deltas(),
            None,
            Duration::from_secs(60),
            Some(42),
            plain(),
        );
        assert!(
            out.contains("🎯 Track — watching octocat (reach ≤5, every 1m)"),
            "{out}"
        );
        assert!(
            out.contains("next refresh in 42s · Ctrl+C to stop"),
            "{out}"
        );
        assert!(out.contains(" 1. 👥 Pair Extraordinaire ↑2"), "{out}");
        assert!(
            out.contains("2 co-authored commits remaining (8/10 to Bronze)"),
            "{out}"
        );
    }

    #[test]
    fn watch_frame_without_countdown_shows_the_cadence() {
        let items = [progress(&PULL_SHARK, 11)];
        let out = render_watch(
            Some(&report(true, items.to_vec())),
            5,
            &[],
            None,
            Duration::from_secs(180),
            None,
            plain(),
        );
        assert!(
            out.contains("refreshing every 3m · Ctrl+C to stop"),
            "{out}"
        );
    }

    #[test]
    fn watch_frame_marks_a_failed_refresh_and_keeps_the_snapshot() {
        let items = [progress(&PULL_SHARK, 11)];
        let out = render_watch(
            Some(&report(true, items.to_vec())),
            5,
            &[],
            Some("Pull Shark unavailable: GitHub is having trouble (500)"),
            Duration::from_secs(60),
            None,
            plain(),
        );
        assert!(
            out.contains("⚠ last refresh failed — showing the previous snapshot"),
            "{out}"
        );
        // The stale values are still visible, never dropped silently.
        assert!(out.contains("Pull Shark"), "{out}");
    }

    #[test]
    fn watch_frame_celebrates_a_gold_complete_report() {
        let items = [progress(&PULL_SHARK, 5000)];
        let out = render_watch(
            Some(&report(true, items.to_vec())),
            5,
            &[],
            None,
            Duration::from_secs(60),
            Some(7),
            plain(),
        );
        assert!(
            out.contains("🏆 Watch — every tracked achievement is GOLD COMPLETE"),
            "{out}"
        );
        assert!(out.contains("next refresh in 7s"), "{out}");
        assert!(!out.contains("GOLD COMPLETE for octocat (reach"), "{out}");
    }

    #[test]
    fn watch_output_contains_no_escape_sequences_when_plain() {
        let items = [progress(&PULL_SHARK, 11), progress(&STARSTRUCK, 0)];
        let out = render_watch(
            Some(&report(true, items.to_vec())),
            5,
            &watch_deltas(),
            None,
            Duration::from_secs(60),
            Some(10),
            plain(),
        );
        assert!(!out.contains('\x1b'), "{out}");

        let colored = render_watch(
            Some(&report(true, items.to_vec())),
            5,
            &watch_deltas(),
            None,
            Duration::from_secs(60),
            Some(10),
            Palette::new(true),
        );
        assert_eq!(
            strip_ansi(&colored),
            out,
            "colors must not change the layout"
        );
    }
}
