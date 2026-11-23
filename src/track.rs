use crate::display::{self, Palette};
use crate::github::GithubClient;
use crate::progress::{self, ProgressReport};
use anyhow::Result;

/// `track` — the roadmap v0.2.0 feature: a one-shot report of which
/// achievement tiers are within reach (at most `reach` steps away), so
/// you always know what to do next.
pub async fn run(username: &str, reach: u8, pal: Palette, json: bool) -> Result<()> {
    let client = GithubClient::new()?;
    let report: ProgressReport = progress::fetch(&client, username).await?;
    let (within, further) = report.tiers_by_proximity(reach);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "username": report.username,
                "reach": reach,
                "partial": !report.errors.is_empty(),
                "errors": report.errors,
                "within_reach": within
                    .iter()
                    .map(|p| progress::achievement_json(p))
                    .collect::<Vec<_>>(),
                "further_out": further
                    .iter()
                    .map(|p| progress::achievement_json(p))
                    .collect::<Vec<_>>(),
            }))
            .unwrap()
        );
    } else {
        print!("{}", display::render_track(&report, reach, pal));
    }
    Ok(())
}
