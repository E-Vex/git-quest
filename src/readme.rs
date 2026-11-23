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
                "markdown": markdown,
            }))
            .unwrap()
        );
    } else {
        print!("{markdown}");
    }
    Ok(())
}
