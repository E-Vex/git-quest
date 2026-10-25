use crate::cli::SortBy;
use crate::display;
use crate::github::GithubClient;
use anyhow::Result;

pub async fn run(lang: &str, limit: u8, sort: &SortBy) -> Result<()> {
    let client = GithubClient::new()?;
    let (total, issues) = client
        .good_first_issues(lang, limit, sort.as_query())
        .await?;
    print!(
        "{}",
        display::render_hunt(lang, total, &issues, display::Palette::detect())
    );
    Ok(())
}
