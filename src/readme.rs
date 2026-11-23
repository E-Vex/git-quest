use crate::display;
use crate::github::GithubClient;
use crate::progress;
use anyhow::Result;

pub async fn run(username: &str, json: bool) -> Result<()> {
    let client = GithubClient::new()?;
    let report = progress::fetch(&client, username).await?;

    let markdown = display::render_readme(&report);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "username": report.username,
                // Same contract as `check --json` and `track --json`: a
                // source that failed is omitted from the markdown block
                // (never faked as zero) and `partial` flags the run.
                "partial": !report.errors.is_empty(),
                "errors": report.errors,
                "markdown": markdown,
            }))
            .unwrap()
        );
    } else {
        print!("{markdown}");
    }
    Ok(())
}
