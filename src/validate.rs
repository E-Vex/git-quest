//! Validation for user input that ends up inside GitHub URLs and Search
//! API queries.

/// GitHub's maximum username length.
const MAX_USERNAME_LEN: usize = 39;

/// Normalise and validate a GitHub username.
///
/// Surrounding whitespace and a single leading `@` are accepted and
/// stripped. Usernames are interpolated into REST paths and Search API
/// qualifiers (`author:<name>`), so anything outside `[A-Za-z0-9-]` is
/// rejected to rule out path and query-qualifier injection.
pub fn username(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    let name = trimmed.strip_prefix('@').unwrap_or(trimmed);

    if name.is_empty() {
        return Err("username must not be empty".into());
    }
    if name.len() > MAX_USERNAME_LEN {
        return Err(format!(
            "username is too long ({} > {MAX_USERNAME_LEN} characters)",
            name.len()
        ));
    }
    if let Some(c) = name
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || *c == '-'))
    {
        return Err(format!(
            "invalid character {c:?} in username (allowed: letters, digits and '-')"
        ));
    }
    if name.starts_with('-') || name.ends_with('-') {
        return Err("username cannot start or end with a hyphen".into());
    }
    Ok(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ordinary_usernames() {
        for ok in ["octocat", "E-Vex", "a", "user-123", "A1b2"] {
            assert_eq!(username(ok).as_deref(), Ok(ok), "{ok}");
        }
    }

    #[test]
    fn strips_whitespace_and_one_leading_at_sign() {
        assert_eq!(username("@octocat").unwrap(), "octocat");
        assert_eq!(username("  octocat\n").unwrap(), "octocat");
        assert!(username("@@octocat").is_err());
    }

    #[test]
    fn accepts_the_maximum_length_and_rejects_one_more() {
        assert!(username(&"a".repeat(39)).is_ok());
        assert!(username(&"a".repeat(40)).is_err());
    }

    #[test]
    fn rejects_empty_input() {
        assert!(username("").is_err());
        assert!(username("   ").is_err());
        assert!(username("@").is_err());
    }

    #[test]
    fn rejects_hyphens_at_the_edges() {
        assert!(username("-octocat").is_err());
        assert!(username("octocat-").is_err());
    }

    #[test]
    fn rejects_search_qualifier_and_path_injection() {
        for bad in [
            "octocat label:bug",
            "octocat/repos",
            "../user",
            "a:b",
            "a;b",
            "a%20b",
            "octo\"cat",
            "über",
        ] {
            assert!(username(bad).is_err(), "{bad} should be rejected");
        }
    }
}
