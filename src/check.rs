use crate::achievements::{Progress, PULL_SHARK};
use crate::github::GithubClient;
use anyhow::Result;

pub async fn run(username: &str) -> Result<()> {
    let client = GithubClient::new()?;

    let prs = client.merged_pr_count(username).await?;

    let items = vec![Progress {
        def: &PULL_SHARK,
        current: prs,
    }];

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
