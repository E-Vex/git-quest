//! Client tests against a local mock of the GitHub API.

use super::*;
use serde_json::json;
use wiremock::matchers::{body_partial_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client_for(server: &MockServer, token: Option<&str>) -> GithubClient {
    GithubClient::with_config(ClientConfig {
        token: token.map(str::to_string),
        rest_base: server.uri(),
        graphql_url: format!("{}/graphql", server.uri()),
    })
    .unwrap()
}

fn api_error(err: anyhow::Error) -> ApiError {
    err.downcast::<ApiError>().expect("expected an ApiError")
}

#[tokio::test]
async fn merged_pr_count_reads_total_count() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/search/issues"))
        .and(query_param("q", "is:merged type:pr author:octocat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total_count": 72,
            "items": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    let count = client_for(&server, None)
        .merged_pr_count("octocat")
        .await
        .unwrap();
    assert_eq!(count, 72);
}

#[tokio::test]
async fn requests_identify_the_tool_and_accept_the_github_media_type() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(header("accept", "application/vnd.github+json"))
        .and(header(
            "user-agent",
            format!("git-quest/{}", env!("CARGO_PKG_VERSION")).as_str(),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "total_count": 1 })))
        .expect(1)
        .mount(&server)
        .await;

    client_for(&server, None)
        .merged_pr_count("o")
        .await
        .unwrap();
}

#[tokio::test]
async fn token_is_sent_as_a_bearer_credential() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(header("authorization", "Bearer ghp_secret"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "total_count": 1 })))
        .expect(1)
        .mount(&server)
        .await;

    let client = client_for(&server, Some("ghp_secret"));
    assert!(client.has_token());
    client.merged_pr_count("o").await.unwrap();
}

#[tokio::test]
async fn no_token_means_no_authorization_header() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "total_count": 1 })))
        .mount(&server)
        .await;

    let client = client_for(&server, None);
    assert!(!client.has_token());
    client.merged_pr_count("o").await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert!(!requests[0].headers.contains_key("authorization"));
}

fn repo(name: &str, stars: u64) -> serde_json::Value {
    json!({ "full_name": name, "stargazers_count": stars })
}

#[tokio::test]
async fn top_repo_stars_picks_the_best_single_repository_across_pages() {
    let server = MockServer::start().await;

    let mut first_page: Vec<_> = (0..100).map(|i| repo(&format!("u/r{i}"), 1)).collect();
    first_page[57] = repo("u/popular", 50);
    Mock::given(method("GET"))
        .and(path("/users/u/repos"))
        .and(query_param("page", "1"))
        .and(query_param("per_page", "100"))
        .and(query_param("type", "owner"))
        .respond_with(ResponseTemplate::new(200).set_body_json(first_page))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/users/u/repos"))
        .and(query_param("page", "2"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(vec![repo("u/huge", 300), repo("u/tiny", 0)]),
        )
        .expect(1)
        .mount(&server)
        .await;

    let best = client_for(&server, None).top_repo_stars("u").await.unwrap();
    assert_eq!(best, Some(("u/huge".to_string(), 300)));
}

#[tokio::test]
async fn top_repo_stars_does_not_sum_repositories() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/u/repos"))
        .respond_with(ResponseTemplate::new(200).set_body_json(vec![
            repo("u/a", 10),
            repo("u/b", 10),
            repo("u/c", 10),
        ]))
        .mount(&server)
        .await;

    let (_, stars) = client_for(&server, None)
        .top_repo_stars("u")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stars, 10, "three 10-star repos must not count as 30");
}

#[tokio::test]
async fn top_repo_stars_is_none_without_repositories() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/empty/repos"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;

    assert_eq!(
        client_for(&server, None)
            .top_repo_stars("empty")
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn coauthored_commits_use_the_commit_search() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/search/commits"))
        .and(query_param("q", r#"author:octocat "Co-Authored-By:""#))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "total_count": 4 })))
        .expect(1)
        .mount(&server)
        .await;

    let n = client_for(&server, None)
        .coauthored_commit_count("octocat")
        .await
        .unwrap();
    assert_eq!(n, 4);
}

#[tokio::test]
async fn discussion_answers_need_a_token_and_skip_the_network_without_one() {
    let server = MockServer::start().await;
    let answers = client_for(&server, None)
        .accepted_discussion_answers("octocat")
        .await
        .unwrap();
    assert_eq!(answers, None);
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn discussion_answers_are_read_from_graphql() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(body_partial_json(
            json!({ "variables": { "login": "octocat" } }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "user": { "repositoryDiscussionComments": { "totalCount": 3 } } }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let answers = client_for(&server, Some("t"))
        .accepted_discussion_answers("octocat")
        .await
        .unwrap();
    assert_eq!(answers, Some(3));
}

#[tokio::test]
async fn discussion_answers_surface_graphql_errors() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "errors": [{ "message": "Something exploded" }]
        })))
        .mount(&server)
        .await;

    let err = client_for(&server, Some("t"))
        .accepted_discussion_answers("octocat")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Something exploded"), "{err}");
}

#[tokio::test]
async fn discussion_answers_are_none_for_an_unknown_user() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "user": null }
        })))
        .mount(&server)
        .await;

    let answers = client_for(&server, Some("t"))
        .accepted_discussion_answers("ghost")
        .await
        .unwrap();
    assert_eq!(answers, None);
}

#[tokio::test]
async fn good_first_issues_sends_the_expected_query_and_maps_items() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/search/issues"))
        .and(query_param(
            "q",
            r#"label:"good first issue" state:open language:"rust" no:assignee"#,
        ))
        .and(query_param("sort", "reactions-+1"))
        .and(query_param("order", "desc"))
        .and(query_param("per_page", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total_count": 99,
            "items": [
                {
                    "title": "Fix the thing",
                    "html_url": "https://github.com/o/r/issues/1",
                    "repository_url": "https://api.github.com/repos/o/r",
                    "comments": 5,
                    "created_at": "2024-01-02T03:04:05Z",
                    "reactions": { "total_count": 9 }
                },
                {
                    "title": "No reactions field",
                    "html_url": "https://github.com/o/r/issues/2",
                    "repository_url": "https://api.github.com/repos/o/r",
                    "comments": 0,
                    "created_at": "2024-02-03T04:05:06Z"
                }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let (total, issues) = client_for(&server, None)
        .good_first_issues("rust", 2, "reactions-+1")
        .await
        .unwrap();

    assert_eq!(total, 99);
    assert_eq!(issues.len(), 2);
    assert_eq!(issues[0].title, "Fix the thing");
    assert_eq!(issues[0].repo, "o/r");
    assert_eq!(issues[0].reactions, 9);
    assert_eq!(issues[0].comments, 5);
    assert_eq!(
        issues[1].reactions, 0,
        "a missing reactions object counts as zero"
    );
}

#[tokio::test]
async fn a_404_becomes_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({ "message": "Not Found" })))
        .mount(&server)
        .await;

    let err = api_error(
        client_for(&server, None)
            .top_repo_stars("nobody")
            .await
            .unwrap_err(),
    );
    assert!(matches!(err, ApiError::NotFound { .. }));
}

#[tokio::test]
async fn an_exhausted_quota_reports_when_it_resets() {
    let server = MockServer::start().await;
    let reset = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 600;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(403)
                .insert_header("x-ratelimit-remaining", "0")
                .insert_header("x-ratelimit-reset", reset.to_string().as_str())
                .set_body_json(json!({ "message": "API rate limit exceeded for 1.2.3.4" })),
        )
        .mount(&server)
        .await;

    let err = api_error(
        client_for(&server, None)
            .merged_pr_count("o")
            .await
            .unwrap_err(),
    );
    match err {
        ApiError::RateLimited {
            resets_in,
            authenticated,
            ..
        } => {
            assert!(!authenticated);
            let secs = resets_in.expect("reset header was sent").as_secs();
            assert!((590..=600).contains(&secs), "{secs}");
        }
        other => panic!("expected RateLimited, got {other:?}"),
    }
}

#[tokio::test]
async fn an_undecodable_body_is_reported_as_such() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let err = client_for(&server, None)
        .merged_pr_count("o")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("could not decode"), "{err}");
}
