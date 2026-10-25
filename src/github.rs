use anyhow::{anyhow, Result};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::time::Duration;

const REST_BASE: &str = "https://api.github.com";
const GRAPHQL_URL: &str = "https://api.github.com/graphql";

/// Fail fast on unreachable networks instead of hanging the terminal.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Upper bound for a whole request, including reading the body.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// A thin GitHub REST/GraphQL client with optional token auth.
///
/// Reads `GITHUB_TOKEN` from the environment. With a token the rate
/// limit rises from 60 to 5000 requests/hour and Galaxy Brain
/// (GraphQL) becomes trackable.
pub struct GithubClient {
    http: reqwest::Client,
    token: Option<String>,
}

impl GithubClient {
    pub fn new() -> Result<Self> {
        let token = std::env::var("GITHUB_TOKEN")
            .ok()
            .filter(|t| !t.trim().is_empty());

        let mut headers = HeaderMap::new();
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );
        let ua = format!("git-quest/{}", env!("CARGO_PKG_VERSION"));
        headers.insert(USER_AGENT, HeaderValue::from_str(&ua)?);
        if let Some(t) = &token {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {t}"))?,
            );
        }

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()?;

        Ok(Self { http, token })
    }

    /// Whether a token was found in the environment.
    pub fn has_token(&self) -> bool {
        self.token.is_some()
    }

    /// Send a request and decode JSON, mapping common failures to
    /// actionable error messages.
    async fn send<T: DeserializeOwned>(
        &self,
        req: reqwest::RequestBuilder,
        what: &str,
    ) -> Result<T> {
        let resp = req.send().await?;
        let status = resp.status();
        match status.as_u16() {
            404 => Err(anyhow!("{what}: not found (404) — check the username")),
            403 | 429 => {
                #[derive(Deserialize)]
                struct Body {
                    message: String,
                }
                let body: Option<Body> = resp.json().await.ok();
                if body
                    .as_ref()
                    .is_some_and(|b| b.message.contains("secondary rate limit"))
                {
                    Err(anyhow!(
                        "{what}: GitHub secondary rate limit hit (abuse detection).\n  This is temporary — wait a few minutes, then try again.\n  Tip: avoid running several git-quest commands back-to-back within a minute."
                    ))
                } else {
                    Err(anyhow!(
                        "{what}: rate limited by GitHub ({status}).\n  Fix: export GITHUB_TOKEN=ghp_xxx  (raises 60 → 5000 core requests/hour)\n  Create a token at https://github.com/settings/tokens (no scopes needed for public data)"
                    ))
                }
            }
            _ => {
                let resp = resp.error_for_status()?;
                Ok(resp.json().await?)
            }
        }
    }

    /// Pull Shark — merged pull requests authored by `username`.
    pub async fn merged_pr_count(&self, username: &str) -> Result<u64> {
        #[derive(Deserialize)]
        struct R {
            total_count: u64,
        }
        let r: R = self
            .send(
                self.http
                    .get(format!("{REST_BASE}/search/issues"))
                    .query(&[("q", format!("is:merged type:pr author:{username}"))]),
                "Pull Shark search",
            )
            .await?;
        Ok(r.total_count)
    }

    /// Starstruck — the single most-starred repository owned by
    /// `username`, as `(full_name, stars)`.
    ///
    /// The achievement is awarded when *one* repository reaches a star
    /// count, so summing stars across repositories would overcount.
    /// Returns `None` when the user owns no repositories.
    pub async fn top_repo_stars(&self, username: &str) -> Result<Option<(String, u64)>> {
        #[derive(Deserialize)]
        struct Repo {
            full_name: String,
            stargazers_count: u64,
        }
        let mut best: Option<(String, u64)> = None;
        let mut page: u32 = 1;
        loop {
            let repos: Vec<Repo> = self
                .send(
                    self.http
                        .get(format!("{REST_BASE}/users/{username}/repos"))
                        .query(&[
                            ("per_page", "100".to_string()),
                            ("page", page.to_string()),
                            ("type", "owner".to_string()),
                        ]),
                    "Starstruck repos",
                )
                .await?;
            let n = repos.len();
            for repo in repos {
                if best
                    .as_ref()
                    .is_none_or(|(_, s)| repo.stargazers_count > *s)
                {
                    best = Some((repo.full_name, repo.stargazers_count));
                }
            }
            if n < 100 || page >= 30 {
                break;
            }
            page += 1;
        }
        Ok(best)
    }

    /// Galaxy Brain — accepted answers in Discussions (GraphQL).
    ///
    /// Returns `None` when no token is configured, since the GraphQL
    /// API requires authentication.
    pub async fn accepted_discussion_answers(&self, login: &str) -> Result<Option<u64>> {
        if self.token.is_none() {
            return Ok(None);
        }

        #[derive(Deserialize)]
        struct Resp {
            data: Option<Data>,
            #[serde(default)]
            errors: Vec<GraphError>,
        }
        #[derive(Deserialize)]
        struct Data {
            user: Option<User>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct User {
            repository_discussion_comments: Count,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Count {
            total_count: u64,
        }
        #[derive(Deserialize)]
        struct GraphError {
            message: String,
        }

        let query = r#"
            query($login: String!) {
                user(login: $login) {
                    repositoryDiscussionComments(onlyAnswers: true) {
                        totalCount
                    }
                }
            }"#;
        let body = serde_json::json!({ "query": query, "variables": { "login": login } });

        let resp: Resp = self
            .send(
                self.http.post(GRAPHQL_URL).json(&body),
                "Galaxy Brain GraphQL",
            )
            .await?;

        if let Some(err) = resp.errors.first() {
            return Err(anyhow!("Galaxy Brain GraphQL: {}", err.message));
        }
        Ok(resp
            .data
            .and_then(|d| d.user)
            .map(|u| u.repository_discussion_comments.total_count))
    }

    /// Pair Extraordinaire — commits authored by `username` that
    /// carry a `Co-Authored-By:` trailer (approximation).
    pub async fn coauthored_commit_count(&self, username: &str) -> Result<u64> {
        #[derive(Deserialize)]
        struct R {
            total_count: u64,
        }
        let r: R = self
            .send(
                self.http
                    .get(format!("{REST_BASE}/search/commits"))
                    .query(&[("q", format!("author:{username} \"Co-Authored-By:\""))]),
                "Pair Extraordinaire commit search",
            )
            .await?;
        Ok(r.total_count)
    }

    /// Hunt — open `good first issue` tasks for a language, sorted by
    /// the given criterion (`reactions-+1`, `created`, `updated`,
    /// `comments`). Returns (total available, top `limit` items).
    pub async fn good_first_issues(
        &self,
        lang: &str,
        limit: u8,
        sort: &str,
    ) -> Result<(u64, Vec<IssueItem>)> {
        #[derive(Deserialize)]
        struct R {
            total_count: u64,
            #[serde(default)]
            items: Vec<Item>,
        }
        #[derive(Deserialize)]
        struct Item {
            title: String,
            html_url: String,
            repository_url: String,
            comments: u64,
            created_at: String,
            #[serde(default)]
            reactions: Option<Reactions>,
        }
        #[derive(Deserialize)]
        struct Reactions {
            total_count: u64,
        }

        let q = format!("label:\"good first issue\" state:open language:{lang} no:assignee");
        let r: R = self
            .send(
                self.http.get(format!("{REST_BASE}/search/issues")).query(&[
                    ("q", q),
                    ("sort", sort.to_string()),
                    ("order", "desc".to_string()),
                    ("per_page", limit.to_string()),
                ]),
                "Hunt issue search",
            )
            .await?;

        let items = r
            .items
            .into_iter()
            .map(|it| {
                let repo = it
                    .repository_url
                    .rsplit_once("/repos/")
                    .map(|(_, s)| s.to_string())
                    .unwrap_or_default();
                IssueItem {
                    title: it.title,
                    url: it.html_url,
                    repo,
                    comments: it.comments,
                    reactions: it.reactions.map(|x| x.total_count).unwrap_or(0),
                    created_at: it.created_at,
                }
            })
            .collect();
        Ok((r.total_count, items))
    }
}

/// A single issue found by `hunt`.
pub struct IssueItem {
    pub title: String,
    pub url: String,
    /// `owner/name` derived from the API repository URL.
    pub repo: String,
    pub comments: u64,
    pub reactions: u64,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
}
