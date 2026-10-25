use crate::achievements::{Progress, GALAXY_BRAIN, PAIR_EXTRAORDINAIRE, PULL_SHARK, STARSTRUCK};
use crate::display;
use crate::github::GithubClient;
use anyhow::Result;

pub async fn run(username: &str) -> Result<()> {
    let client = GithubClient::new()?;

    let (prs, stars, answers, coauthored) = tokio::join!(
        client.merged_pr_count(username),
        client.top_repo_stars(username),
        client.accepted_discussion_answers(username),
        client.coauthored_commit_count(username),
    );
    let prs = prs?;
    let (star_repo, stars) = match stars? {
        Some((repo, stars)) => (Some(repo), stars),
        None => (None, 0),
    };
    let answers = match answers {
        Ok(v) => v,
        Err(e) => {
            eprintln!("⚠️  Galaxy Brain unavailable: {e}");
            None
        }
    };
    let coauthored = match coauthored {
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

    display::render_check(username, &items, client.has_token());
    Ok(())
}
