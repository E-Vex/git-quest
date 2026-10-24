use crate::achievements::{
    GALAXY_BRAIN, PAIR_EXTRAORDINAIRE, Progress, PULL_SHARK, STARSTRUCK,
};
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
    } else if !client.has_token() {
        eprintln!("💡 Set GITHUB_TOKEN to also track Galaxy Brain (accepted discussion answers).");
    }
    if let Some(c) = coauthored {
        items.push(Progress {
            def: &PAIR_EXTRAORDINAIRE,
            current: c,
        });
    }

    println!("Achievement progress for {username}\n");
    for p in &items {
        match p.next_tier() {
            Some((tier, threshold)) => println!(
                "{} {}: {}/{} to {} ({} {} remaining)",
                p.def.emoji,
                p.def.name,
                p.current,
                threshold,
                tier,
                p.remaining(),
                p.def.hint
            ),
            None => println!("{} {}: GOLD achieved 🏆", p.def.emoji, p.def.name),
        }
    }
    Ok(())
}
