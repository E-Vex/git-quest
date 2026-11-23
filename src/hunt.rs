use crate::cli::SortBy;
use crate::display::{self, Palette};
use crate::github::GithubClient;
use anyhow::Result;

pub async fn run(
    lang: &str,
    limit: u8,
    label: &str,
    max_age_days: Option<u64>,
    sort: &SortBy,
    pal: Palette,
    json: bool,
) -> Result<()> {
    let client = GithubClient::new()?;
    let (total, issues) = client
        .good_first_issues(lang, limit, label, max_age_days, sort.as_query())
        .await?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "lang": lang,
                "label": label,
                "limit": limit,
                // Reported as given on the command line, null when unset.
                "max_age_days": max_age_days,
                "sort": sort.as_query(),
                "total": total,
                "issues": issues,
            }))
            .unwrap()
        );
    } else {
        print!("{}", display::render_hunt(lang, label, total, &issues, pal));
    }
    Ok(())
}
