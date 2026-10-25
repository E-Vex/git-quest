//! Typed GitHub API errors with actionable messages.
//!
//! Classification is a pure function of the response (status, headers,
//! message and the current time) so it can be tested without a network.

use reqwest::header::HeaderMap;
use std::error::Error;
use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Everything that can go wrong while talking to GitHub.
///
/// `what` is a short label of the failing operation ("Pull Shark
/// search") so messages say *which* request failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    /// 404 — the resource does not exist.
    NotFound { what: String },
    /// 401 — the token was rejected.
    Unauthorized { what: String },
    /// Primary rate limit (403/429 with an exhausted quota).
    RateLimited {
        what: String,
        /// Time until the quota resets, when GitHub told us.
        resets_in: Option<Duration>,
        /// Whether the request carried a token.
        authenticated: bool,
    },
    /// Secondary (abuse-detection) rate limit.
    SecondaryRateLimit {
        what: String,
        retry_after: Option<Duration>,
    },
    /// 403 that is not a rate limit (e.g. SAML enforcement).
    Forbidden { what: String, message: String },
    /// 422 — typically a search for a user that does not exist.
    Unprocessable { what: String, message: String },
    /// 5xx — GitHub is having a bad day.
    Server { what: String, status: u16 },
    /// Any other unsuccessful status.
    Other {
        what: String,
        status: u16,
        message: String,
    },
    /// The request never produced a response (DNS, TLS, timeout, ...).
    Network {
        what: String,
        message: String,
        timeout: bool,
    },
}

impl ApiError {
    /// Build the error for a failed `reqwest` call that produced no response.
    pub fn from_reqwest(what: &str, err: reqwest::Error) -> Self {
        let timeout = err.is_timeout();
        ApiError::Network {
            what: what.to_string(),
            message: error_chain(&err.without_url()),
            timeout,
        }
    }
}

impl ApiError {
    /// Build the error for a successful response whose body could not be
    /// decoded into the expected shape.
    pub fn from_decode(what: &str, status: u16, err: reqwest::Error) -> Self {
        ApiError::Other {
            what: what.to_string(),
            status,
            message: format!(
                "could not decode the response: {}",
                error_chain(&err.without_url())
            ),
        }
    }
}

/// Join an error and its sources as `outer: inner: innermost`.
fn error_chain(err: &dyn Error) -> String {
    let mut parts = vec![err.to_string()];
    let mut source = err.source();
    while let Some(s) = source {
        parts.push(s.to_string());
        source = s.source();
    }
    parts.join(": ")
}

/// Turn a non-success response into an [`ApiError`].
pub fn classify(
    what: &str,
    status: u16,
    headers: &HeaderMap,
    message: Option<&str>,
    authenticated: bool,
    now: SystemTime,
) -> ApiError {
    let what = what.to_string();
    let message_text = message.unwrap_or_default().to_string();
    let lower = message_text.to_lowercase();

    match status {
        401 => ApiError::Unauthorized { what },
        404 => ApiError::NotFound { what },
        403 | 429 => {
            let retry_after = header_u64(headers, "retry-after").map(Duration::from_secs);
            if lower.contains("secondary rate limit") || retry_after.is_some() {
                return ApiError::SecondaryRateLimit { what, retry_after };
            }
            let exhausted = header_u64(headers, "x-ratelimit-remaining") == Some(0);
            if status == 429 || exhausted || lower.contains("rate limit") {
                let resets_in = header_u64(headers, "x-ratelimit-reset").map(|reset| {
                    let now = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                    Duration::from_secs(reset.saturating_sub(now))
                });
                return ApiError::RateLimited {
                    what,
                    resets_in,
                    authenticated,
                };
            }
            ApiError::Forbidden {
                what,
                message: message_text,
            }
        }
        422 => ApiError::Unprocessable {
            what,
            message: message_text,
        },
        500..=599 => ApiError::Server { what, status },
        _ => ApiError::Other {
            what,
            status,
            message: message_text,
        },
    }
}

/// Extract a readable message from a GitHub error body.
///
/// GitHub puts the headline in `message` and the actionable detail in
/// `errors[]` (as objects with a `message`, or bare strings), e.g.
/// `Validation Failed: The listed users cannot be searched ...`.
pub fn parse_error_body(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let headline = value.get("message")?.as_str()?.trim().to_string();
    let details: Vec<String> = value
        .get("errors")
        .and_then(|e| e.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| match item {
                    serde_json::Value::String(s) => Some(s.clone()),
                    other => other.get("message")?.as_str().map(str::to_string),
                })
                .collect()
        })
        .unwrap_or_default();

    if details.is_empty() {
        Some(headline)
    } else {
        Some(format!("{headline}: {}", details.join("; ")))
    }
}

fn header_u64(headers: &HeaderMap, name: &str) -> Option<u64> {
    headers.get(name)?.to_str().ok()?.trim().parse().ok()
}

/// Human-friendly wait time: `45s`, `12 min`, `1h 05min`.
pub fn format_wait(wait: Duration) -> String {
    let secs = wait.as_secs();
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{} min", secs.div_ceil(60))
    } else {
        format!("{}h {:02}min", secs / 3600, (secs % 3600).div_ceil(60) % 60)
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::NotFound { what } => {
                write!(f, "{what}: not found (404) — check the username")
            }
            ApiError::Unauthorized { what } => write!(
                f,
                "{what}: GitHub rejected the credentials (401).\n  Fix: check that GITHUB_TOKEN is valid and has not expired or been revoked."
            ),
            ApiError::RateLimited {
                what,
                resets_in,
                authenticated,
            } => {
                write!(f, "{what}: rate limited by GitHub.")?;
                if let Some(wait) = resets_in {
                    write!(f, " The limit resets in {}.", format_wait(*wait))?;
                }
                if *authenticated {
                    write!(f, "\n  Wait for the limit to reset, then try again.")
                } else {
                    write!(
                        f,
                        "\n  Fix: export GITHUB_TOKEN=ghp_xxx  (raises 60 → 5000 core requests/hour)\n  Create a token at https://github.com/settings/tokens (no scopes needed for public data)"
                    )
                }
            }
            ApiError::SecondaryRateLimit { what, retry_after } => {
                write!(
                    f,
                    "{what}: GitHub secondary rate limit hit (abuse detection).\n  This is temporary — wait "
                )?;
                match retry_after {
                    Some(wait) => write!(f, "{}", format_wait(*wait))?,
                    None => write!(f, "a few minutes")?,
                }
                write!(
                    f,
                    ", then try again.\n  Tip: avoid running several git-quest commands back-to-back within a minute."
                )
            }
            ApiError::Forbidden { what, message } => {
                write!(f, "{what}: access forbidden (403)")?;
                if !message.is_empty() {
                    write!(f, ": {message}")?;
                }
                Ok(())
            }
            ApiError::Unprocessable { what, message } => {
                write!(f, "{what}: GitHub rejected the query (422)")?;
                if message.contains("cannot be searched") {
                    write!(
                        f,
                        ".\n  The user most likely does not exist — check the spelling."
                    )
                } else if message.is_empty() {
                    Ok(())
                } else {
                    write!(f, ": {message}")
                }
            }
            ApiError::Server { what, status } => write!(
                f,
                "{what}: GitHub is having trouble ({status}). Try again in a moment — see https://www.githubstatus.com"
            ),
            ApiError::Other {
                what,
                status,
                message,
            } => {
                write!(f, "{what}: unexpected response ({status})")?;
                if !message.is_empty() {
                    write!(f, ": {message}")?;
                }
                Ok(())
            }
            ApiError::Network {
                what,
                message,
                timeout,
            } => {
                if *timeout {
                    write!(f, "{what}: request timed out — check your connection")
                } else {
                    write!(f, "{what}: network error: {message}")
                }
            }
        }
    }
}

impl Error for ApiError {}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderName, HeaderValue};

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(
                HeaderName::from_static(name),
                HeaderValue::from_str(value).unwrap(),
            );
        }
        map
    }

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn classify_simple(status: u16, message: Option<&str>) -> ApiError {
        classify("op", status, &HeaderMap::new(), message, false, at(1_000))
    }

    #[test]
    fn plain_statuses_are_classified() {
        assert!(matches!(
            classify_simple(401, None),
            ApiError::Unauthorized { .. }
        ));
        assert!(matches!(
            classify_simple(404, None),
            ApiError::NotFound { .. }
        ));
        assert!(matches!(
            classify_simple(502, None),
            ApiError::Server { status: 502, .. }
        ));
        assert!(matches!(
            classify_simple(418, Some("teapot")),
            ApiError::Other { status: 418, .. }
        ));
    }

    #[test]
    fn exhausted_quota_reports_the_reset_time() {
        let h = headers(&[
            ("x-ratelimit-remaining", "0"),
            ("x-ratelimit-reset", "1900"),
        ]);
        let err = classify(
            "op",
            403,
            &h,
            Some("API rate limit exceeded"),
            false,
            at(1_000),
        );
        assert_eq!(
            err,
            ApiError::RateLimited {
                what: "op".into(),
                resets_in: Some(Duration::from_secs(900)),
                authenticated: false,
            }
        );
    }

    #[test]
    fn a_reset_in_the_past_saturates_to_zero() {
        let h = headers(&[("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", "500")]);
        let err = classify("op", 403, &h, None, true, at(1_000));
        assert!(matches!(
            err,
            ApiError::RateLimited { resets_in: Some(d), authenticated: true, .. } if d.is_zero()
        ));
    }

    #[test]
    fn status_429_is_always_a_rate_limit() {
        assert!(matches!(
            classify_simple(429, None),
            ApiError::RateLimited { .. }
        ));
    }

    #[test]
    fn secondary_limits_are_recognised_by_message_or_retry_after() {
        let by_message = classify_simple(403, Some("You have exceeded a secondary rate limit."));
        assert!(matches!(
            by_message,
            ApiError::SecondaryRateLimit {
                retry_after: None,
                ..
            }
        ));

        let h = headers(&[("retry-after", "30")]);
        let by_header = classify("op", 403, &h, None, true, at(0));
        assert_eq!(
            by_header,
            ApiError::SecondaryRateLimit {
                what: "op".into(),
                retry_after: Some(Duration::from_secs(30)),
            }
        );
    }

    #[test]
    fn other_403s_are_not_mistaken_for_rate_limits() {
        let err = classify_simple(
            403,
            Some("Resource protected by organization SAML enforcement."),
        );
        assert!(matches!(err, ApiError::Forbidden { .. }));
    }

    #[test]
    fn unsearchable_users_get_a_hint() {
        let err = classify_simple(
            422,
            Some("Validation Failed: The listed users cannot be searched."),
        );
        assert!(err.to_string().contains("does not exist"));
    }

    #[test]
    fn rate_limit_message_points_unauthenticated_users_at_a_token() {
        let err = ApiError::RateLimited {
            what: "op".into(),
            resets_in: Some(Duration::from_secs(600)),
            authenticated: false,
        };
        let text = err.to_string();
        assert!(text.contains("resets in 10 min"), "{text}");
        assert!(text.contains("GITHUB_TOKEN"), "{text}");

        let authed = ApiError::RateLimited {
            what: "op".into(),
            resets_in: None,
            authenticated: true,
        };
        assert!(!authed.to_string().contains("GITHUB_TOKEN"));
    }

    #[test]
    fn wait_times_are_human_readable() {
        assert_eq!(format_wait(Duration::from_secs(0)), "0s");
        assert_eq!(format_wait(Duration::from_secs(45)), "45s");
        assert_eq!(format_wait(Duration::from_secs(60)), "1 min");
        assert_eq!(format_wait(Duration::from_secs(61)), "2 min");
        assert_eq!(format_wait(Duration::from_secs(3599)), "60 min");
        assert_eq!(format_wait(Duration::from_secs(3600)), "1h 00min");
        assert_eq!(format_wait(Duration::from_secs(3600 + 5 * 60)), "1h 05min");
    }

    #[test]
    fn error_body_with_only_a_message() {
        assert_eq!(
            parse_error_body(r#"{"message":"Not Found"}"#).as_deref(),
            Some("Not Found")
        );
    }

    #[test]
    fn error_body_details_are_appended() {
        let body = r#"{"message":"Validation Failed","errors":[{"message":"The listed users cannot be searched","resource":"Search"}]}"#;
        assert_eq!(
            parse_error_body(body).as_deref(),
            Some("Validation Failed: The listed users cannot be searched")
        );
    }

    #[test]
    fn error_body_accepts_string_details_and_skips_unknown_shapes() {
        let body = r#"{"message":"Bad","errors":["first",{"code":"x"},{"message":"second"}]}"#;
        assert_eq!(
            parse_error_body(body).as_deref(),
            Some("Bad: first; second")
        );
    }

    #[test]
    fn error_body_that_is_not_json_yields_none() {
        assert_eq!(parse_error_body("<html>502</html>"), None);
        assert_eq!(parse_error_body(""), None);
        assert_eq!(parse_error_body(r#"{"no_message":1}"#), None);
    }

    #[test]
    fn detailed_unprocessable_message_triggers_the_user_hint() {
        let message = parse_error_body(
            r#"{"message":"Validation Failed","errors":[{"message":"The listed users cannot be searched either because the users do not exist"}]}"#,
        );
        let err = classify_simple(422, message.as_deref());
        assert!(err.to_string().contains("does not exist"), "{err}");
    }
}
