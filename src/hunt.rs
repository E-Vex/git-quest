use crate::cli::SortBy;
use crate::display::{self, Palette};
use crate::github::GithubClient;
use anyhow::Result;

pub async fn run(lang: &str, limit: u8, label: &str, sort: &SortBy, pal: Palette) -> Result<()> {
    let client = GithubClient::new()?;
    let (total, issues) = client
        .good_first_issues(lang, limit, label, sort.as_query())
        .await?;
    print!("{}", display::render_hunt(lang, label, total, &issues, pal));
    Ok(())
}
