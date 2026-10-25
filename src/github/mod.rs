use anyhow::{anyhow, Result};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::time::{Duration, SystemTime};

mod error;

pub use error::ApiError;
use error::{classify, parse_error_body};

const DEFAULT_REST_BASE: &str = "https://api.github.com";

/// Fail fast on unreachable networks instead of hanging the terminal.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Upper bound for a whole request, including reading the body.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Where and how to talk to GitHub.
pub struct ClientConfig {
    /// Personal access token, if any.
    pub token: Option<String>,
    /// REST API root without a trailing slash.
    pub rest_base: String,
    /// GraphQL endpoint.
    pub graphql_url: String,
}

impl ClientConfig {
    /// Read the configuration from the environment:
    ///
    /// - `GITHUB_TOKEN` — personal access token
    /// - `GITHUB_API_URL` — REST root (GitHub Enterprise Server, e.g.
    ///   `https://ghe.example.com/api/v3`); also set by GitHub Actions
    /// - `GITHUB_GRAPHQL_URL` — GraphQL endpoint, derived from the REST
    ///   root when absent
    pub fn from_env() -> Result<Self> {
        Self::from_vars(|name| std::env::var(name).ok())
    }

    /// Testable core of [`ClientConfig::from_env`].
    fn from_vars(get: impl Fn(&str) -> Option<String>) -> Result<Self> {
        let non_empty = |name: &str| {
            get(name)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };

        let token = non_empty("GITHUB_TOKEN");

        let rest_base = match non_empty("GITHUB_API_URL") {
            Some(url) => validate_endpoint("GITHUB_API_URL", &url)?
                .trim_end_matches('/')
                .to_string(),
            None => DEFAULT_REST_BASE.to_string(),
        };
        let graphql_url = match non_empty("GITHUB_GRAPHQL_URL") {
            Some(url) => validate_endpoint("GITHUB_GRAPHQL_URL", &url)?,
            None => default_graphql_url(&rest_base),
        };

        Ok(Self {
            token,
            rest_base,
            graphql_url,
        })
    }
}

/// Require https, except for loopback hosts, so a token is never sent
/// over plain HTTP to a remote machine by accident.
fn validate_endpoint(name: &str, url: &str) -> Result<String> {
    if url.starts_with("https://") {
        return Ok(url.to_string());
    }
    if let Some(rest) = url.strip_prefix("http://") {
        let host = rest.split(['/', ':']).next().unwrap_or_default();
        if matches!(host, "localhost" | "127.0.0.1" | "[::1]") {
            return Ok(url.to_string());
        }
        return Err(anyhow!(
            "{name} must use https:// (plain http is only allowed for localhost)"
        ));
    }
    Err(anyhow!("{name} must start with https://"))
}

/// GitHub.com serves GraphQL at `<rest>/graphql`; GitHub Enterprise
/// Server serves REST at `/api/v3` but GraphQL at `/api/graphql`.
fn default_graphql_url(rest_base: &str) -> String {
    match rest_base.strip_suffix("/v3") {
        Some(api_root) => format!("{api_root}/graphql"),
        None => format!("{rest_base}/graphql"),
    }
}

/// A thin GitHub REST/GraphQL client with optional token auth.
///
/// Reads `GITHUB_TOKEN` from the environment. With a token the rate
/// limit rises from 60 to 5000 requests/hour and Galaxy Brain
/// (GraphQL) becomes trackable.
pub struct GithubClient {
    http: reqwest::Client,
    token: Option<String>,
    rest_base: String,
    graphql_url: String,
}

impl GithubClient {
    /// Build a client from the environment.
    pub fn new() -> Result<Self> {
        Self::with_config(ClientConfig::from_env()?)
    }

    /// Build a client from an explicit configuration.
    pub fn with_config(config: ClientConfig) -> Result<Self> {
        let ClientConfig {
            token,
            rest_base,
            graphql_url,
        } = config;

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

        Ok(Self {
            http,
            token,
            rest_base,
            graphql_url,
        })
    }

    /// Whether a token was found in the environment.
    pub fn has_token(&self) -> bool {
        self.token.is_some()
    }

    /// Send a request and decode JSON, mapping failures to a typed
    /// [`ApiError`] with an actionable message.
    async fn send<T: DeserializeOwned>(
        &self,
        req: reqwest::RequestBuilder,
        what: &str,
    ) -> Result<T, ApiError> {
        let resp = req
            .send()
            .await
            .map_err(|e| ApiError::from_reqwest(what, e))?;
        let status = resp.status();
        if status.is_success() {
            return resp
                .json()
                .await
                .map_err(|e| ApiError::from_decode(what, status.as_u16(), e));
        }

        let headers = resp.headers().clone();
        let body = resp.text().await.unwrap_or_default();
        let message = parse_error_body(&body);
        Err(classify(
            what,
            status.as_u16(),
            &headers,
            message.as_deref(),
            self.token.is_some(),
            SystemTime::now(),
        ))
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
                    .get(format!("{}/search/issues", self.rest_base))
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
                        .get(format!("{}/users/{username}/repos", self.rest_base))
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
                self.http.post(&self.graphql_url).json(&body),
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
                    .get(format!("{}/search/commits", self.rest_base))
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
        label: &str,
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

        let q = issue_query(lang, label);
        let r: R = self
            .send(
                self.http
                    .get(format!("{}/search/issues", self.rest_base))
                    .query(&[
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

/// Build the Search API query for open, unassigned `good first issue`s.
///
/// The language is quoted so multi-word names such as `Jupyter Notebook`
/// stay a single qualifier value.
fn issue_query(lang: &str, label: &str) -> String {
    format!("label:\"{label}\" state:open language:\"{lang}\" no:assignee")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_query_quotes_the_language() {
        assert_eq!(
            issue_query("rust"),
            r#"label:"good first issue" state:open language:"rust" no:assignee"#
        );
    }

    #[test]
    fn issue_query_keeps_multi_word_languages_together() {
        assert!(issue_query("Jupyter Notebook").contains(r#"language:"Jupyter Notebook""#));
    }
}

#[cfg(test)]
mod client_tests;

#[cfg(test)]
mod config_tests {
    use super::*;
    use std::collections::HashMap;

    fn config(vars: &[(&str, &str)]) -> Result<ClientConfig> {
        let map: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        ClientConfig::from_vars(|name| map.get(name).cloned())
    }

    #[test]
    fn defaults_to_github_dot_com() {
        let c = config(&[]).unwrap();
        assert_eq!(c.token, None);
        assert_eq!(c.rest_base, "https://api.github.com");
        assert_eq!(c.graphql_url, "https://api.github.com/graphql");
    }

    #[test]
    fn token_is_trimmed_and_blank_means_none() {
        assert_eq!(
            config(&[("GITHUB_TOKEN", " ghp_x\n")])
                .unwrap()
                .token
                .as_deref(),
            Some("ghp_x")
        );
        assert_eq!(config(&[("GITHUB_TOKEN", "  ")]).unwrap().token, None);
    }

    #[test]
    fn custom_api_url_loses_its_trailing_slash() {
        let c = config(&[("GITHUB_API_URL", "https://api.example.com/")]).unwrap();
        assert_eq!(c.rest_base, "https://api.example.com");
        assert_eq!(c.graphql_url, "https://api.example.com/graphql");
    }

    #[test]
    fn enterprise_server_graphql_lives_outside_v3() {
        let c = config(&[("GITHUB_API_URL", "https://ghe.example.com/api/v3")]).unwrap();
        assert_eq!(c.rest_base, "https://ghe.example.com/api/v3");
        assert_eq!(c.graphql_url, "https://ghe.example.com/api/graphql");
    }

    #[test]
    fn explicit_graphql_url_wins() {
        let c = config(&[
            ("GITHUB_API_URL", "https://api.example.com"),
            ("GITHUB_GRAPHQL_URL", "https://gql.example.com/graphql"),
        ])
        .unwrap();
        assert_eq!(c.graphql_url, "https://gql.example.com/graphql");
    }

    #[test]
    fn plain_http_is_only_allowed_for_loopback() {
        assert!(config(&[("GITHUB_API_URL", "http://localhost:8080")]).is_ok());
        assert!(config(&[("GITHUB_API_URL", "http://127.0.0.1:9")]).is_ok());
        assert!(config(&[("GITHUB_API_URL", "http://evil.example.com")]).is_err());
        assert!(config(&[("GITHUB_API_URL", "ftp://example.com")]).is_err());
        assert!(config(&[("GITHUB_API_URL", "example.com")]).is_err());
    }

    #[test]
    fn plain_http_lookalike_hosts_are_rejected() {
        assert!(config(&[("GITHUB_API_URL", "http://localhost.evil.com")]).is_err());
        assert!(config(&[("GITHUB_GRAPHQL_URL", "http://evil.com/localhost")]).is_err());
    }
}
