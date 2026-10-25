//! Validation for user input that ends up inside GitHub URLs and Search
//! API queries.

/// GitHub's maximum username length.
const MAX_USERNAME_LEN: usize = 39;
/// Generous upper bound for a language name ("Jupyter Notebook" is 16).
const MAX_LANGUAGE_LEN: usize = 50;
/// Generous upper bound for a label name ("help wanted" is 12).
const MAX_LABEL_LEN: usize = 50;

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
    if name.contains("--") {
        return Err("username cannot contain consecutive hyphens".into());
    }
    Ok(name.to_string())
}

/// Validate a programming language name for use in a `language:"…"`
/// search qualifier.
///
/// Spaces and symbols are fine (`C++`, `C#`, `Jupyter Notebook`); control
/// characters and double quotes are not, because the value is wrapped in
/// quotes and a stray quote would end the qualifier early.
/// Validate a label name for use in a `label:"…"` search qualifier.
pub fn label(input: &str) -> Result<String, String> {
    let label_str = input.trim();

    if label_str.is_empty() {
        return Err("label must not be empty".into());
    }
    if label_str.chars().count() > MAX_LABEL_LEN {
        return Err(format!(
            "label name is too long (max {MAX_LABEL_LEN} characters)"
        ));
    }
    if let Some(c) = label_str.chars().find(|c| c.is_control() || *c == '"') {
        return Err(format!("invalid character {c:?} in label name"));
    }
    Ok(label_str.to_string())
}

pub fn language(input: &str) -> Result<String, String> {
    let lang = input.trim();

    if lang.is_empty() {
        return Err("language must not be empty".into());
    }
    if lang.chars().count() > MAX_LANGUAGE_LEN {
        return Err(format!(
            "language name is too long (max {MAX_LANGUAGE_LEN} characters)"
        ));
    }
    if let Some(c) = lang.chars().find(|c| c.is_control() || *c == '"') {
        return Err(format!("invalid character {c:?} in language name"));
    }
    Ok(lang.to_string())
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
    fn rejects_consecutive_hyphens() {
        assert!(username("octo--cat").is_err());
        assert!(username("a--b").is_err());
        assert!(username("octocat").is_ok()); // single hyphen is fine
        assert!(username("octo-cat").is_ok());
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

    #[test]
    fn accepts_real_language_names() {
        for ok in ["rust", "C++", "C#", "F#", "Objective-C", "Jupyter Notebook"] {
            assert_eq!(language(ok).as_deref(), Ok(ok), "{ok}");
        }
    }

    #[test]
    fn language_is_trimmed() {
        assert_eq!(language("  rust ").unwrap(), "rust");
    }

    #[test]
    fn rejects_empty_or_oversized_language() {
        assert!(language("").is_err());
        assert!(language("   ").is_err());
        assert!(language(&"x".repeat(51)).is_err());
        assert!(language(&"x".repeat(50)).is_ok());
    }

    #[test]
    fn rejects_quotes_and_control_characters_in_language() {
        for bad in ["rust\" label:bug", "ru\nst", "ru\tst", "\"rust\""] {
            assert!(language(bad).is_err(), "{bad:?} should be rejected");
        }
    }
}
