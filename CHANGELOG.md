# Changelog

## v0.2.0 (unreleased)

### Features
- `hunt --label`: filter by any label, not just `good first issue`
- `--json`: raw JSON output for both `check` and `hunt`
- `check` without a username uses the token's authenticated user
- `complete`: generate shell completions (bash, zsh, fish, etc.)
- `readme`: generate a markdown progress block for profile READMEs
- Automatic retry on network errors, timeouts and rate limits

### Fixes
- Starstruck counts the best single repository, not the sum across repos
- Correct achievement tier numbers (Pull Shark: 2/16/128/1024, etc.)
- Colors disable properly when piping to a file or when `NO_COLOR` is set
- System certificate support for corporate proxies
- Request timeouts to avoid hanging
- `--limit 0` no longer sent as-is to the API
- Clear typed errors instead of long stack traces

### Improvements
- Partial results: if one achievement source fails, the rest are still shown
- Username and language validation prevents injection into search queries
