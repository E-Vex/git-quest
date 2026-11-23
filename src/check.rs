use crate::display::{self, Palette};
use crate::github::GithubClient;
use crate::progress::{self, ProgressReport};
use anyhow::Result;

pub async fn run(username: &str, pal: Palette, json: bool) -> Result<()> {
    let client = GithubClient::new()?;
    let report: ProgressReport = progress::fetch(&client, username).await?;

    if json {
        let achievements: Vec<_> = report
            .items
            .iter()
            .map(progress::achievement_json)
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "username": report.username,
                "has_token": report.has_token,
                // `partial` marks runs where a source failed: the affected
                // counts are 0 or omitted, never inflated.
                "partial": !report.errors.is_empty(),
                "errors": report.errors,
                "achievements": achievements,
            }))
            .unwrap()
        );
    } else {
        print!("{}", display::render_check(&report, pal));
    }
    Ok(())
}
