use crate::achievements::{Progress, PULL_SHARK, STARSTRUCK};
use crate::github::GithubClient;
use anyhow::Result;

pub async fn run(username: &str) -> Result<()> {
    let client = GithubClient::new()?;

    let (prs, stars) = tokio::join!(
        client.merged_pr_count(username),
        client.total_stars(username),
    );
    let prs = prs?;
    let stars = stars?;

    let items = vec![
        Progress {
            def: &PULL_SHARK,
            current: prs,
        },
        Progress {
            def: &STARSTRUCK,
            current: stars,
        },
    ];

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
