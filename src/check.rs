use crate::achievements::{Progress, GALAXY_BRAIN, PAIR_EXTRAORDINAIRE, PULL_SHARK, STARSTRUCK};
use crate::display;
use crate::github::GithubClient;
use anyhow::Result;

pub async fn run(username: &str) -> Result<()> {
    let client = GithubClient::new()?;

    let (prs, stars, answers, coauthored) = tokio::join!(
        client.merged_pr_count(username),
        client.total_stars(username),
        client.accepted_discussion_answers(username),
        client.coauthored_commit_count(username),
    );
    let prs = prs?;
    let stars = stars?;
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
        },
        Progress {
            def: &STARSTRUCK,
            current: stars,
        },
    ];
    if let Some(a) = answers {
        items.push(Progress {
            def: &GALAXY_BRAIN,
            current: a,
        });
    }
    if let Some(c) = coauthored {
        items.push(Progress {
            def: &PAIR_EXTRAORDINAIRE,
            current: c,
        });
    }

    display::render_check(username, &items, client.has_token());
    Ok(())
}
