//! Shared data gathering for the report commands (`check`, `readme`,
//! `track`).
//!
//! All three commands need the same four achievement sources; keeping
//! the fetch in one place means they can never disagree about fallback
//! behaviour or error handling.

use crate::achievements::{
    AchievementDef, Progress, GALAXY_BRAIN, PAIR_EXTRAORDINAIRE, PULL_SHARK, STARSTRUCK,
};
use crate::github::{ApiError, GithubClient};
use anyhow::Result;

/// Everything the report commands need to render a run.
#[derive(Debug)]
pub struct ProgressReport {
    /// The user the numbers belong to (resolved to the token owner when
    /// the caller passed no username).
    pub username: String,
    /// Whether a token was available for the queries.
    pub has_token: bool,
    /// Progress for every achievement source that produced data.
    pub items: Vec<Progress>,
    /// Messages for achievement sources that failed. When non-empty the
    /// report is *partial*: counts may understate reality and the
    /// affected achievements either show 0 or are omitted entirely.
    pub errors: Vec<String>,
    /// The achievement sources behind [`Self::errors`], so consumers can
    /// tell which counts are untrustworthy without parsing the message
    /// strings (used by `track --watch` to suppress delta chips for
    /// counts that are 0 only because their source failed).
    pub(crate) failed: Vec<&'static AchievementDef>,
}

impl ProgressReport {
    /// Achievements sorted by how close the next tier is, split into
    /// `(within reach, further out)` relative to `reach` remaining
    /// steps. Gold-complete achievements belong to neither list — they
    /// are done, not "within reach".
    pub fn tiers_by_proximity(&self, reach: u8) -> (Vec<&Progress>, Vec<&Progress>) {
        let mut sorted: Vec<&Progress> = self.items.iter().collect();
        sorted.sort_by_key(|p| (p.remaining(), p.def.name));
        let within_reach = sorted
            .iter()
            .copied()
            .filter(|p| p.next_tier().is_some() && p.remaining() <= u64::from(reach))
            .collect();
        let further_out = sorted
            .iter()
            .copied()
            .filter(|p| p.next_tier().is_some() && p.remaining() > u64::from(reach))
            .collect();
        (within_reach, further_out)
    }
}

/// Resolve an empty username to the token owner's login.
///
/// Three cases, each with its own message:
///
/// - a username was given — use it as-is;
/// - no token is configured — explain what authentication is missing;
/// - the lookup itself failed (revoked token, exhausted quota, network
///   down) — surface the real failure instead of claiming the token is
///   absent, so the fix hint matches the actual problem.
///
/// `pub(crate)` so `track --watch` can resolve once before its loop
/// instead of spending a request on `/user` every refresh.
pub(crate) async fn resolve_username(client: &GithubClient, username: &str) -> Result<String> {
    if !username.is_empty() {
        return Ok(username.to_string());
    }
    match client.authenticated_user().await {
        Ok(Some(login)) => Ok(login),
        Ok(None) => Err(anyhow::anyhow!(
            "no username provided and no GITHUB_TOKEN configured for authentication"
        )),
        Err(e) => Err(e),
    }
}

/// Number of achievement sources that are attempted for a run: the
/// three REST searches, plus the GraphQL query when a token is present.
fn attempted_sources(has_token: bool) -> usize {
    usize::from(has_token) + 3
}

/// When every source that was attempted failed, the report would be
/// all-zero guesses — that is a precondition failure (the user does
/// not exist, the network is down, the token was revoked), not a
/// partial result. Surface the error and stop.
fn all_sources_failed(errors: &[String], has_token: bool) -> bool {
    errors.len() >= attempted_sources(has_token)
}

/// A 404 from `/users/{user}/repos` is a definitive signal that the
/// user does not exist (or is hidden). GitHub's Search API answers
/// plain *zero results* for such users, so the rest of the report
/// would be meaningless zeros — bail instead.
fn is_missing_user(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<ApiError>(),
        Some(ApiError::NotFound { .. })
    )
}

/// Fetch all achievement sources concurrently, degrading gracefully
/// when an individual source fails: a warning goes to stderr, the
/// failure is recorded in [`ProgressReport::errors`] and the rest of
/// the report stays intact. When *every* source fails, the first error
/// is returned instead — an empty report is worse than an error.
pub async fn fetch(client: &GithubClient, username: &str) -> Result<ProgressReport> {
    let username = resolve_username(client, username).await?;
    let has_token = client.has_token();

    let (prs_result, stars_result, answers_result, coauthored_result) = tokio::join!(
        client.merged_pr_count(&username),
        client.top_repo_stars(&username),
        client.accepted_discussion_answers(&username),
        client.coauthored_commit_count(&username),
    );

    let mut errors = Vec::new();
    let mut failed: Vec<&'static AchievementDef> = Vec::new();
    let prs = match prs_result {
        Ok(v) => v,
        Err(e) => {
            errors.push(format!("Pull Shark unavailable: {e}"));
            failed.push(&PULL_SHARK);
            0
        }
    };
    let (star_repo, stars) = match stars_result {
        Ok(Some((repo, s))) => (Some(repo), s),
        Ok(None) => (None, 0),
        Err(e) if is_missing_user(&e) => {
            return Err(anyhow::anyhow!(
                "user `{username}` does not exist (or is hidden): check the spelling"
            ));
        }
        Err(e) => {
            errors.push(format!("Starstruck unavailable: {e}"));
            failed.push(&STARSTRUCK);
            (None, 0)
        }
    };
    let answers = match answers_result {
        Ok(v) => v,
        Err(e) => {
            errors.push(format!("Galaxy Brain unavailable: {e}"));
            failed.push(&GALAXY_BRAIN);
            None
        }
    };
    let coauthored = match coauthored_result {
        Ok(v) => Some(v),
        Err(e) => {
            errors.push(format!("Pair Extraordinaire unavailable: {e}"));
            failed.push(&PAIR_EXTRAORDINAIRE);
            None
        }
    };

    if all_sources_failed(&errors, has_token) {
        // Every source failed the same way; the first message carries
        // the representative cause and fix hint.
        return Err(anyhow::anyhow!(errors[0].clone()));
    }

    for msg in &errors {
        eprintln!("⚠️  {msg}");
    }

    let mut items = vec![
        Progress {
            def: &PULL_SHARK,
            current: prs,
            detail: None,
        },
        Progress {
            def: &STARSTRUCK,
            current: stars,
            detail: star_repo.map(|repo| format!("best repo: {repo}")),
        },
    ];
    if let Some(a) = answers {
        items.push(Progress {
            def: &GALAXY_BRAIN,
            current: a,
            detail: None,
        });
    }
    if let Some(c) = coauthored {
        items.push(Progress {
            def: &PAIR_EXTRAORDINAIRE,
            current: c,
            detail: None,
        });
    }

    Ok(ProgressReport {
        username,
        has_token,
        items,
        errors,
        failed,
    })
}

/// The JSON shape shared by `check --json` and `track --json`.
pub fn achievement_json(p: &Progress) -> serde_json::Value {
    serde_json::json!({
        "name": p.def.name,
        "emoji": p.def.emoji,
        "current": p.current,
        "next_tier": p
            .next_tier()
            .map(|(t, th)| serde_json::json!({"tier": t.name(), "threshold": th})),
        "remaining": p.remaining(),
        "fraction": p.fraction(),
        "detail": p.detail,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::achievements::PULL_SHARK;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn report(items: Vec<Progress>) -> ProgressReport {
        ProgressReport {
            username: "octocat".into(),
            has_token: false,
            items,
            errors: Vec::new(),
            failed: Vec::new(),
        }
    }

    fn item(current: u64) -> Progress {
        Progress {
            def: &PULL_SHARK,
            current,
            detail: None,
        }
    }

    fn err(msg: &str) -> String {
        format!("{msg}: boom")
    }

    #[test]
    fn a_total_failure_is_detected_for_both_auth_modes() {
        // Without a token three REST sources are attempted.
        let unauth = [
            err("Pull Shark unavailable"),
            err("Starstruck unavailable"),
            err("Pair Extraordinaire unavailable"),
        ];
        assert!(all_sources_failed(&unauth, false));
        // With a token the GraphQL source joins the attempt.
        assert!(all_sources_failed(&unauth, true) == false); // 3 of 4 is partial
        let mut authed = unauth.to_vec();
        authed.push(err("Galaxy Brain unavailable"));
        assert!(all_sources_failed(&authed, true));
    }

    #[test]
    fn a_partial_failure_is_not_a_total_failure() {
        let partial = [err("Pull Shark unavailable"), err("Starstruck unavailable")];
        assert!(!all_sources_failed(&partial, false));
        assert!(!all_sources_failed(&[], true));
    }

    #[test]
    fn a_404_is_recognised_as_a_missing_user() {
        let not_found = anyhow::Error::new(ApiError::NotFound {
            what: "Starstruck repos".into(),
        });
        assert!(is_missing_user(&not_found));

        let rate_limited = anyhow::Error::new(ApiError::RateLimited {
            what: "Starstruck repos".into(),
            resets_in: None,
            authenticated: false,
        });
        assert!(!is_missing_user(&rate_limited));

        let plain = anyhow::anyhow!("network down");
        assert!(!is_missing_user(&plain));
    }

    #[test]
    fn tiers_are_sorted_by_remaining_distance() {
        let r = report(vec![item(72), item(0), item(15)]);
        let (within, further) = r.tiers_by_proximity(5);
        // 15 is one away from Bronze (16), 0 needs 2 to unlock, 72 needs 56.
        assert_eq!(
            within.iter().map(|p| p.current).collect::<Vec<_>>(),
            [15, 0]
        );
        assert_eq!(further.iter().map(|p| p.current).collect::<Vec<_>>(), [72]);
    }

    #[test]
    fn a_tier_exactly_at_the_reach_limit_counts_as_within_reach() {
        let r = report(vec![item(11)]); // 5 remaining to Bronze (16)
        let (within, further) = r.tiers_by_proximity(5);
        assert_eq!(within.len(), 1);
        assert!(further.is_empty());
    }

    #[test]
    fn gold_complete_items_are_excluded_from_both_lists() {
        let r = report(vec![item(2000)]);
        let (within, further) = r.tiers_by_proximity(5);
        assert!(within.is_empty());
        assert!(further.is_empty());
    }

    #[test]
    fn ties_keep_insertion_order_for_stable_output() {
        let shark = Progress {
            def: &PULL_SHARK,
            current: 0,
            detail: None,
        };
        let shark2 = Progress {
            def: &PULL_SHARK,
            current: 0,
            detail: Some("dup".into()),
        };
        let r = report(vec![shark, shark2]);
        let (within, _) = r.tiers_by_proximity(5);
        assert_eq!(within.len(), 2);
        assert!(
            within[0].detail.is_none(),
            "equal remaining keeps insertion order"
        );
    }

    // --- fetch() failure modes, exercised against a mock GitHub API ---

    fn mock_client(server: &MockServer, token: Option<&str>) -> GithubClient {
        GithubClient::with_config(crate::github::ClientConfig {
            token: token.map(str::to_string),
            rest_base: server.uri(),
            graphql_url: format!("{}/graphql", server.uri()),
        })
        .unwrap()
    }

    #[tokio::test]
    async fn fetch_reports_a_missing_user_instead_of_rendering_zeros() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/users/ghost/repos"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "message": "Not Found"
            })))
            .mount(&server)
            .await;

        let err = fetch(&mock_client(&server, None), "ghost")
            .await
            .unwrap_err();
        let text = err.to_string();
        assert!(text.contains("does not exist"), "{text}");
        assert!(text.contains("check the spelling"), "{text}");
    }

    #[tokio::test]
    async fn fetch_fails_when_every_source_fails() {
        let server = MockServer::start().await;
        for verb in ["GET", "POST"] {
            Mock::given(method(verb))
                .respond_with(ResponseTemplate::new(403))
                .mount(&server)
                .await;
        }

        let err = fetch(&mock_client(&server, Some("t")), "octocat")
            .await
            .unwrap_err();
        // The first recorded source failure is surfaced as the cause.
        assert!(err.to_string().contains("Pull Shark unavailable"), "{err}");
    }

    #[tokio::test]
    async fn fetch_keeps_a_partial_report_when_one_source_fails() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search/issues"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "total_count": 10,
                "items": []
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/users/octocat/repos"))
            .respond_with(ResponseTemplate::new(200).set_body_json(vec![
                serde_json::json!({"full_name": "o/hello", "stargazers_count": 20}),
            ]))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/search/commits"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "total_count": 3
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/graphql"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;

        let report = fetch(&mock_client(&server, Some("t")), "octocat")
            .await
            .unwrap();
        assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
        assert!(report.errors[0].contains("Galaxy Brain unavailable"));
        assert!(
            !report.items.iter().any(|p| p.def.name == "Galaxy Brain"),
            "a failed source is omitted, not faked as zero"
        );
        let names: Vec<_> = report.items.iter().map(|p| p.def.name).collect();
        assert_eq!(names, ["Pull Shark", "Starstruck", "Pair Extraordinaire"]);
    }

    #[tokio::test]
    async fn fetch_without_a_username_needs_a_token() {
        let server = MockServer::start().await;

        let err = fetch(&mock_client(&server, None), "").await.unwrap_err();
        assert!(
            err.to_string()
                .contains("no GITHUB_TOKEN configured for authentication"),
            "{err}"
        );
    }

    #[tokio::test]
    async fn fetch_without_a_username_surfaces_a_rejected_token() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "message": "Bad credentials"
            })))
            .mount(&server)
            .await;

        let err = fetch(&mock_client(&server, Some("revoked-token")), "")
            .await
            .unwrap_err();
        // `{:#}` renders the full context chain, exactly what the CLI prints.
        let text = format!("{err:#}");
        assert!(
            text.contains("could not resolve the token owner's login"),
            "{text}"
        );
        assert!(text.contains("401"), "{text}");
        // The old behaviour blamed the missing variable even when a
        // (broken) token was configured — the message must match the
        // actual problem.
        assert!(!text.contains("no GITHUB_TOKEN configured"), "{text}");
    }
}
