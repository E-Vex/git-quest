//! `track --watch` — live refresh loop with deltas.
//!
//! The loop re-runs the shared progress pipeline every `--interval`, redraws
//! the screen and highlights what changed since the previous refresh.
//! Everything mechanical lives in small pure functions ([`parse_interval`],
//! [`min_interval`], [`compute_deltas`]) so the decisions can be tested
//! without a terminal or a clock; [`watch_loop`] glues them together and is
//! testable end-to-end against a mock API through the `max_refreshes` bail.

use crate::achievements::{AchievementDef, Progress};
use crate::display::{self, Palette};
use crate::github::GithubClient;
use crate::progress::{self, ProgressReport};
use anyhow::{Context, Result};
use std::io::{IsTerminal, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// One refresh costs 3 requests without a token (two Search API calls plus
/// the repository listing) and 4 with a token (plus the GraphQL query).
///
/// Safe minimum per mode, derived from GitHub's documented limits:
///
/// - **unauthenticated**: the core quota is 60 requests/hour, so 3 requests
///   per refresh allow at most 20 refreshes/hour → 180s;
/// - **authenticated**: core quota (5000/h) is not binding, but every
///   refresh still hits the Search API (30 requests/minute) and the
///   secondary rate limits, so the floor stays well clear of abuse
///   detection at 20s.
pub fn min_interval(has_token: bool) -> Duration {
    if has_token {
        Duration::from_secs(20)
    } else {
        Duration::from_secs(180)
    }
}

/// Parse the `--interval` value: a bare number means seconds; `s`, `m` and
/// `h` suffixes are accepted (`90s`, `5m`, `2h`). Zero and anything above
/// 24h is rejected — a watch that never sleeps would hammer the API, and
/// one that sleeps a day is not a watch.
pub fn parse_interval(input: &str) -> Result<Duration, String> {
    let input = input.trim();
    let invalid = || {
        format!(
            "invalid interval `{input}` — use a number of seconds or a value with an s/m/h suffix, e.g. 90s, 5m, 2h"
        )
    };

    let (digits, unit_secs): (&str, u64) = match input.chars().next_back() {
        Some('s') => (&input[..input.len() - 1], 1),
        Some('m') => (&input[..input.len() - 1], 60),
        Some('h') => (&input[..input.len() - 1], 3600),
        // Any other last character (including multi-byte ones) means the
        // whole input is parsed as a bare number of seconds, which then
        // produces a clean error instead of a panic on a UTF-8 boundary.
        _ => (input, 1),
    };

    let n: u64 = digits.trim().parse().map_err(|_| invalid())?;
    let secs = n
        .checked_mul(unit_secs)
        .ok_or_else(|| format!("interval `{input}` is too large (max 24h)"))?;
    if secs == 0 {
        return Err(format!(
            "interval `{input}` parses to 0 seconds — a watch must sleep between refreshes"
        ));
    }
    if secs > 86_400 {
        return Err(format!("interval `{input}` is too large (max 24h)"));
    }
    Ok(Duration::from_secs(secs))
}

/// Per-achievement change between two consecutive refreshes, keyed by
/// achievement name.
///
/// Two kinds of items produce no delta:
///
/// - achievements whose source failed in the current refresh: their count
///   is a 0 placeholder, not an observation, so a "↓66" chip would lie;
/// - achievements that appeared or disappeared between snapshots (a
///   source failed in one of the two runs) — there is no honest
///   comparison to make.
pub fn compute_deltas(
    prev: &[Progress],
    now: &[Progress],
    failed: &[&'static AchievementDef],
) -> Vec<(String, i64)> {
    let before: std::collections::HashMap<&str, u64> =
        prev.iter().map(|p| (p.def.name, p.current)).collect();
    let mut deltas = Vec::new();
    for p in now {
        if failed.iter().any(|f| f.name == p.def.name) {
            continue;
        }
        if let Some(old) = before.get(p.def.name) {
            let diff = p.current as i64 - *old as i64;
            if diff != 0 {
                deltas.push((p.def.name.to_string(), diff));
            }
        }
    }
    deltas
}

/// Current Unix time in whole seconds (0 if the clock is before the epoch).
fn epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// One compact JSON object per refresh — the NDJSON contract for
/// `track --watch --json`. Same fields as `track --json`, plus the `deltas`
/// since the previous refresh and the cadence.
fn refresh_json(
    report: &ProgressReport,
    reach: u8,
    deltas: &[(String, i64)],
    interval: Duration,
) -> String {
    let delta_map: serde_json::Map<String, serde_json::Value> = deltas
        .iter()
        .map(|(name, d)| (name.clone(), serde_json::json!(d)))
        .collect();
    serde_json::to_string(&serde_json::json!({
        "event": "refresh",
        "username": report.username,
        "reach": reach,
        "partial": !report.errors.is_empty(),
        "errors": report.errors,
        "within_reach": report
            .tiers_by_proximity(reach)
            .0
            .iter()
            .map(|p| progress::achievement_json(p))
            .collect::<Vec<_>>(),
        "further_out": report
            .tiers_by_proximity(reach)
            .1
            .iter()
            .map(|p| progress::achievement_json(p))
            .collect::<Vec<_>>(),
        "deltas": delta_map,
        "interval_secs": interval.as_secs(),
        "refreshed_at": epoch_secs(),
    }))
    .expect("refresh JSON is always serializable")
}

/// NDJSON event emitted when a refresh failed mid-watch: the previous
/// snapshot stays on screen and this explains why nothing moved.
fn error_json(error: &str, interval: Duration) -> String {
    serde_json::to_string(&serde_json::json!({
        "event": "refresh_failed",
        "error": error,
        "retry_in_secs": interval.as_secs(),
        "refreshed_at": epoch_secs(),
    }))
    .expect("error JSON is always serializable")
}

/// Everything the loop needs to render; built once by [`run`].
struct LoopConfig {
    reach: u8,
    interval: Duration,
    json: bool,
    pal: Palette,
    /// Whether stdout is a terminal: drives screen clearing and the live
    /// countdown. Piped output (and JSON mode) renders one frame per
    /// refresh and never clears.
    interactive: bool,
}

/// Run the watch loop until interrupted.
///
/// The interval floor is enforced here (not by clap) because it depends on
/// whether a token is configured, and the username is resolved exactly
/// once — a per-poll `/user` lookup would waste a request every refresh.
pub async fn run(
    client: &GithubClient,
    username: &str,
    reach: u8,
    interval: Duration,
    json: bool,
    pal: Palette,
) -> Result<()> {
    let min = min_interval(client.has_token());
    if interval < min {
        return Err(if client.has_token() {
            anyhow::anyhow!(
                "--interval {}s is below the safe minimum of {}s with a token: every refresh hits the Search API (30 requests/minute) and faster polls risk abuse detection",
                interval.as_secs(),
                min.as_secs()
            )
        } else {
            anyhow::anyhow!(
                "--interval {}s is below the safe minimum of {}s without a token: each refresh costs 3 requests and the unauthenticated quota is only 60 requests/hour. Set GITHUB_TOKEN (raises it to 5000/h) to watch as often as every 20s",
                interval.as_secs(),
                min.as_secs()
            )
        });
    }

    let username = progress::resolve_username(client, username).await?;
    let interactive = !json && std::io::stdout().is_terminal();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    watch_loop(
        client,
        &username,
        LoopConfig {
            reach,
            interval,
            json,
            pal,
            interactive,
        },
        &mut out,
        None,
    )
    .await
}

/// Outcome of waiting out one interval.
enum Step {
    /// The interval elapsed; refresh again.
    Continue,
    /// Ctrl+C was received; stop cleanly.
    Stop,
}

/// Wait for `duration`, unless Ctrl+C arrives first.
async fn wait_for(duration: Duration) -> Step {
    tokio::select! {
        _ = tokio::time::sleep(duration) => Step::Continue,
        _ = tokio::signal::ctrl_c() => Step::Stop,
    }
}

async fn watch_loop(
    client: &GithubClient,
    username: &str,
    cfg: LoopConfig,
    out: &mut impl Write,
    max_refreshes: Option<u64>,
) -> Result<()> {
    let mut prev: Option<Vec<Progress>> = None;
    let mut report: Option<ProgressReport> = None;
    let mut deltas: Vec<(String, i64)> = Vec::new();
    // Assigned by every loop iteration before it is read (Ok → None,
    // later failure → Some), so it starts uninitialized.
    let mut error: Option<String>;
    let mut refreshes: u64 = 0;

    loop {
        match progress::fetch(client, username).await {
            Ok(r) => {
                deltas = match &prev {
                    Some(p) => compute_deltas(p, &r.items, &r.failed),
                    None => Vec::new(),
                };
                prev = Some(r.items.clone());
                error = None;
                report = Some(r);
            }
            Err(e) => {
                if refreshes == 0 {
                    // Same contract as the one-shot command: a first refresh
                    // that fails is an error, not a watch that shows nothing.
                    return Err(e);
                }
                // Keep the previous snapshot visible and explain why it did
                // not move; the next interval retries on its own.
                error = Some(format!("{e:#}"));
            }
        }
        refreshes += 1;

        if cfg.json {
            // The event reflects what happened on *this* refresh: a failed
            // one emits refresh_failed even though the previous snapshot
            // stays available in `report` for the next frame.
            let line = match (&error, &report) {
                (Some(e), _) => error_json(e, cfg.interval),
                (None, Some(r)) => refresh_json(r, cfg.reach, &deltas, cfg.interval),
                (None, None) => unreachable!("the first refresh either succeeds or fails"),
            };
            writeln!(out, "{line}").context("could not write to stdout")?;
            out.flush().context("could not write to stdout")?;
        } else {
            render_frame(out, report.as_ref(), &cfg, &deltas, error.as_deref(), None)?;
        }

        if max_refreshes.is_some_and(|max| refreshes >= max) {
            return Ok(());
        }

        // Wait out the interval. On a terminal, redraw once per second so
        // the countdown stays honest; JSON and piped modes just sleep.
        if cfg.json || !cfg.interactive {
            if let Step::Stop = wait_for(cfg.interval).await {
                return stopped(refreshes);
            }
        } else {
            let total = cfg.interval.as_secs();
            for remaining in (1..total).rev() {
                if let Step::Stop = wait_for(Duration::from_secs(1)).await {
                    return stopped(refreshes);
                }
                render_frame(
                    out,
                    report.as_ref(),
                    &cfg,
                    &deltas,
                    error.as_deref(),
                    Some(remaining),
                )?;
            }
            if let Step::Stop = wait_for(Duration::from_secs(1)).await {
                return stopped(refreshes);
            }
        }
    }
}

fn stopped(refreshes: u64) -> Result<()> {
    eprintln!("🛑 watch stopped after {refreshes} refresh(es)");
    Ok(())
}

/// Draw one watch frame: clear the screen when interactive, then render.
///
/// `countdown` is `Some(seconds to the next refresh)` on countdown redraws;
/// `None` renders the static "refreshing every …" line.
fn render_frame(
    out: &mut impl Write,
    report: Option<&ProgressReport>,
    cfg: &LoopConfig,
    deltas: &[(String, i64)],
    error: Option<&str>,
    countdown: Option<u64>,
) -> Result<()> {
    let frame = display::render_watch(
        report,
        cfg.reach,
        deltas,
        error,
        cfg.interval,
        countdown,
        cfg.pal,
    );
    if cfg.interactive {
        // Clear screen and home the cursor — layout, not color, so it is
        // independent of --color. Piped output is never cleared.
        write!(out, "\x1b[2J\x1b[1;1H").context("could not write to stdout")?;
    }
    out.write_all(frame.as_bytes())
        .context("could not write to stdout")?;
    out.flush().context("could not write to stdout")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::achievements::{AchievementDef, PAIR_EXTRAORDINAIRE, PULL_SHARK, STARSTRUCK};
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn progress(def: &'static AchievementDef, current: u64) -> Progress {
        Progress {
            def,
            current,
            detail: None,
        }
    }

    // --- parse_interval ---

    #[test]
    fn parses_bare_seconds_and_unit_suffixes() {
        assert_eq!(parse_interval("60").unwrap(), Duration::from_secs(60));
        assert_eq!(parse_interval("90s").unwrap(), Duration::from_secs(90));
        assert_eq!(parse_interval("5m").unwrap(), Duration::from_secs(300));
        assert_eq!(parse_interval("2h").unwrap(), Duration::from_secs(7200));
        assert_eq!(parse_interval(" 5m ").unwrap(), Duration::from_secs(300));
        assert_eq!(parse_interval("5 m").unwrap(), Duration::from_secs(300));
    }

    #[test]
    fn rejects_zero_and_oversized_intervals() {
        assert!(parse_interval("0").is_err());
        assert!(parse_interval("0m").is_err());
        assert!(parse_interval("25h").is_err(), "above the 24h cap");
        assert!(parse_interval("86400").is_ok());
        assert!(parse_interval("86401").is_err());
    }

    #[test]
    fn rejects_garbage_without_panicking_on_multibyte_input() {
        for bad in ["", "   ", "abc", "5x", "-5", "1.5m", "5ñ", "m", "s"] {
            assert!(parse_interval(bad).is_err(), "{bad:?} must be rejected");
        }
    }

    // --- min_interval ---

    #[test]
    fn interval_floors_follow_the_quota_math() {
        assert_eq!(min_interval(false), Duration::from_secs(180));
        assert_eq!(min_interval(true), Duration::from_secs(20));
    }

    // --- compute_deltas ---

    #[test]
    fn deltas_capture_rises_and_falls() {
        let prev = [progress(&PULL_SHARK, 66), progress(&STARSTRUCK, 13)];
        let now = [progress(&PULL_SHARK, 68), progress(&STARSTRUCK, 12)];
        assert_eq!(
            compute_deltas(&prev, &now, &[]),
            [
                ("Pull Shark".to_string(), 2),
                ("Starstruck".to_string(), -1)
            ]
        );
    }

    #[test]
    fn unchanged_values_produce_no_delta() {
        let prev = [progress(&PULL_SHARK, 66)];
        let now = [progress(&PULL_SHARK, 66)];
        assert!(compute_deltas(&prev, &now, &[]).is_empty());
    }

    #[test]
    fn a_value_missing_from_either_snapshot_produces_no_delta() {
        // Appeared since the last refresh: nothing to compare against.
        let now = [progress(&PAIR_EXTRAORDINAIRE, 4)];
        assert!(compute_deltas(&[], &now, &[]).is_empty());
        // Disappeared since the last refresh: no honest comparison.
        let prev = [progress(&PAIR_EXTRAORDINAIRE, 4)];
        assert!(compute_deltas(&prev, &[], &[]).is_empty());
    }

    #[test]
    fn a_failed_source_produces_no_delta() {
        // The source failed in the current refresh, so `now` carries a 0
        // placeholder — reporting ↓66 would misrepresent reality.
        let prev = [progress(&PULL_SHARK, 66)];
        let now = [progress(&PULL_SHARK, 0)];
        assert!(compute_deltas(&prev, &now, &[&PULL_SHARK]).is_empty());
    }

    // --- JSON events ---

    fn json_report() -> ProgressReport {
        ProgressReport {
            username: "octocat".into(),
            has_token: true,
            items: vec![progress(&PULL_SHARK, 14)],
            errors: vec![],
            failed: vec![],
        }
    }

    #[test]
    fn refresh_json_carries_the_full_snapshot_and_deltas() {
        let line = refresh_json(
            &json_report(),
            5,
            &[("Pull Shark".to_string(), 2)],
            Duration::from_secs(60),
        );
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["event"], "refresh");
        assert_eq!(v["username"], "octocat");
        assert_eq!(v["reach"], 5);
        assert_eq!(v["partial"], false);
        assert_eq!(v["deltas"]["Pull Shark"], 2);
        assert_eq!(v["interval_secs"], 60);
        assert!(v["refreshed_at"].as_u64().unwrap() > 0);
        // 14/16 to Bronze: 2 remaining, inside the default reach of 5.
        assert_eq!(v["within_reach"].as_array().unwrap().len(), 1);
        assert_eq!(v["further_out"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn error_json_explains_the_failed_refresh() {
        let line = error_json("boom", Duration::from_secs(60));
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["event"], "refresh_failed");
        assert_eq!(v["error"], "boom");
        assert_eq!(v["retry_in_secs"], 60);
    }

    // --- the loop itself, against a mock GitHub API ---

    fn mock_client(server: &MockServer) -> GithubClient {
        GithubClient::with_config(crate::github::ClientConfig {
            token: None,
            rest_base: server.uri(),
            graphql_url: format!("{}/graphql", server.uri()),
        })
        .unwrap()
    }

    async fn mount_sources(server: &MockServer, prs: u64) {
        // Limited to one hit so a later mock (a changed count, or a server
        // failure) can take over on subsequent refreshes — wiremock matches
        // mounted mocks in mount order.
        Mock::given(method("GET"))
            .and(path("/search/issues"))
            .and(query_param("q", "is:merged type:pr author:octocat"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "total_count": prs, "items": [] })),
            )
            .up_to_n_times(1)
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path("/search/commits"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "total_count": 4 })),
            )
            .up_to_n_times(1)
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path("/users/octocat/repos"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "full_name": "o/hello", "stargazers_count": 20 }
            ])))
            .up_to_n_times(1)
            .mount(server)
            .await;
    }

    async fn two_refresh_json_lines(server: &MockServer) -> Vec<String> {
        // First refresh sees 66 merged PRs, the second sees 68 — the loop
        // must report a +2 delta on the second line. The one-shot stubs
        // from mount_sources are limited to one hit, so steady 200 mocks
        // take over for the sources that do not change.
        mount_sources(server, 66).await;
        Mock::given(method("GET"))
            .and(path("/search/commits"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "total_count": 4 })),
            )
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path("/users/octocat/repos"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "full_name": "o/hello", "stargazers_count": 20 }
            ])))
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path("/search/issues"))
            .and(query_param("q", "is:merged type:pr author:octocat"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "total_count": 68, "items": [] })),
            )
            .mount(server)
            .await;

        let client = mock_client(server);
        let mut out: Vec<u8> = Vec::new();
        watch_loop(
            &client,
            "octocat",
            LoopConfig {
                reach: 5,
                interval: Duration::from_secs(1),
                json: true,
                pal: Palette::new(false),
                interactive: false,
            },
            &mut out,
            Some(2),
        )
        .await
        .unwrap();
        String::from_utf8(out)
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect()
    }

    #[tokio::test]
    async fn the_loop_emits_one_json_event_per_refresh_with_deltas() {
        let server = MockServer::start().await;
        let lines = two_refresh_json_lines(&server).await;
        assert_eq!(lines.len(), 2, "{lines:?}");

        let first: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
        assert_eq!(first["event"], "refresh");
        assert_eq!(first["deltas"], serde_json::json!({}));

        let second: serde_json::Value = serde_json::from_str(&lines[1]).unwrap();
        assert_eq!(second["event"], "refresh");
        assert_eq!(second["deltas"]["Pull Shark"], 2, "{second}");
    }

    #[tokio::test]
    async fn the_loop_renders_text_frames_without_a_terminal() {
        let server = MockServer::start().await;
        mount_sources(&server, 66).await;

        let client = mock_client(&server);
        let mut out: Vec<u8> = Vec::new();
        watch_loop(
            &client,
            "octocat",
            LoopConfig {
                reach: 5,
                interval: Duration::from_secs(1),
                json: false,
                pal: Palette::new(false),
                interactive: false,
            },
            &mut out,
            Some(1),
        )
        .await
        .unwrap();

        let frames = String::from_utf8(out).unwrap();
        assert!(frames.contains("watching octocat"), "{frames}");
        assert!(frames.contains("refreshing every 1s"), "{frames}");
        // Piped output must not contain clear-screen sequences.
        assert!(!frames.contains("\x1b[2J"), "{frames}");
    }

    #[tokio::test]
    async fn the_first_failed_refresh_is_a_hard_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(404)
                    .set_body_json(serde_json::json!({ "message": "Not Found" })),
            )
            .mount(&server)
            .await;

        let client = mock_client(&server);
        let mut out: Vec<u8> = Vec::new();
        let err = watch_loop(
            &client,
            "ghost",
            LoopConfig {
                reach: 5,
                interval: Duration::from_secs(1),
                json: true,
                pal: Palette::new(false),
                interactive: false,
            },
            &mut out,
            Some(2),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("does not exist"), "{err}");
        assert!(out.is_empty(), "no frames for a failed first refresh");
    }

    #[tokio::test]
    async fn a_later_failed_refresh_keeps_the_previous_snapshot() {
        let server = MockServer::start().await;
        // First refresh: every source answers once. From the second on, a
        // catch-all 500 (mounted after the one-shot stubs) fails *all*
        // sources — a partial failure would still be a valid `refresh`.
        mount_sources(&server, 66).await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
            .mount(&server)
            .await;

        let client = mock_client(&server);
        let mut out: Vec<u8> = Vec::new();
        watch_loop(
            &client,
            "octocat",
            LoopConfig {
                reach: 5,
                interval: Duration::from_secs(1),
                json: true,
                pal: Palette::new(false),
                interactive: false,
            },
            &mut out,
            Some(2),
        )
        .await
        .unwrap();

        let lines: Vec<serde_json::Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["event"], "refresh");
        assert_eq!(lines[1]["event"], "refresh_failed", "{}", lines[1]);
        assert!(lines[1]["error"].as_str().unwrap().contains("500"));
    }
}
