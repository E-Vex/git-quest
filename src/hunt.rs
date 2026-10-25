use crate::cli::SortBy;
use crate::display::{self, Palette};
use crate::github::GithubClient;
use anyhow::Result;

pub async fn run(lang: &str, limit: u8, label: &str, sort: &SortBy, pal: Palette, json: bool) -> Result<()> {
    let client = GithubClient::new()?;
    let (total, issues) = client
        .good_first_issues(lang, limit, label, sort.as_query())
        .await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "lang": lang,
            "label": label,
            "limit": limit,
            "sort": sort.as_query(),
            "total": total,
            "issues": issues,
        })).unwrap());
    } else {
        print!("{}", display::render_hunt(lang, label, total, &issues, pal));
    }
    Ok(())
}
