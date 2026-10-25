use crate::achievements::{Progress, GALAXY_BRAIN, PAIR_EXTRAORDINAIRE, PULL_SHARK, STARSTRUCK};
use crate::display::{self, Palette};
use crate::github::GithubClient;
use anyhow::Result;

pub async fn run(username: &str, pal: Palette, json: bool) -> Result<()> {
    let client = GithubClient::new()?;

    let username = if username.is_empty() {
        match client.authenticated_user().await {
            Some(name) => name,
            None => anyhow::bail!(
                "no username provided and no GITHUB_TOKEN configured for authentication"
            ),
        }
    } else {
        username.to_string()
    };

    let (prs_result, stars_result, answers_result, coauthored_result) = tokio::join!(
        client.merged_pr_count(&username),
        client.top_repo_stars(&username),
        client.accepted_discussion_answers(&username),
        client.coauthored_commit_count(&username),
    );
    let prs = match prs_result {
        Ok(v) => v,
        Err(e) => {
            eprintln!("⚠️  Pull Shark unavailable: {e}");
            0
        }
    };
    let (star_repo, stars) = match stars_result {
        Ok(Some((repo, s))) => (Some(repo), s),
        Ok(None) => (None, 0),
        Err(e) => {
            eprintln!("⚠️  Starstruck unavailable: {e}");
            (None, 0)
        }
    };
    let answers = match answers_result {
        Ok(v) => v,
        Err(e) => {
            eprintln!("⚠️  Galaxy Brain unavailable: {e}");
            None
        }
    };
    let coauthored = match coauthored_result {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("⚠️  Pair Extraordinaire unavailable: {e}");
            None
        }
    };

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

    if json {
        let achievements: Vec<_> = items.iter().map(|p| {
            serde_json::json!({
                "name": p.def.name,
                "emoji": p.def.emoji,
                "current": p.current,
                "next_tier": p.next_tier().map(|(t, th)| serde_json::json!({"tier": t.name(), "threshold": th})),
                "remaining": p.remaining(),
                "fraction": p.fraction(),
                "detail": p.detail,
            })
        }).collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "username": username,
                "has_token": client.has_token(),
                "achievements": achievements,
            }))
            .unwrap()
        );
    } else {
        print!(
            "{}",
            display::render_check(&username, &items, client.has_token(), pal)
        );
    }
    Ok(())
}
