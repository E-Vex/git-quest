# Changelog

## v0.2.1

### Fixes
- `hunt` no longer lists pull requests: GitHub's issue search matches
  issues *and* pull requests, so a PR carrying the `good first issue`
  label could be presented as a contribution task (and counted in the
  reported total). The query now pins `is:issue`.
- `hunt` also skips issues in archived repositories (`archived:false`) —
  they cannot accept contributions, so pointing a hunter there is a dead
  end.
- `check`/`track`/`readme` without a username no longer blame a missing
  `GITHUB_TOKEN` when one is configured but broken: a revoked or expired
  token, an exhausted quota or a network failure now reports the real
  error (`could not resolve the token owner's login: …`).

### Improvements
- `readme --json` exposes `partial` and `errors` just like `check --json`
  and `track --json`, so all three report commands share one JSON
  contract and consumers can tell a degraded run from a genuine zero.
- MSRV corrected to 1.85 and now verified by CI: the locked dependency
  stack had silently drifted past the declared 1.82 (the TLS/QUIC
  closure of reqwest always contains edition-2024 crates, which cargo
  1.82 cannot even parse). `idna_adapter` is pinned to the ICU4X 1.5
  line so the resolver does not pull the rustc-1.88 ICU4X 2.4 chain,
  and a new CI job builds on Rust 1.85 to keep the claim honest.

## v0.2.0

### Features
- `track`: a one-shot "what's next" report — the achievement tiers within
  reach (at most `--reach N` steps away, default 5), sorted by how close
  they are, with the remaining targets listed further out. Completes the
  v0.2.0 roadmap item.
- `--json`: raw JSON output for `check`, `hunt`, `track` and `readme`
- `check` without a username uses the token's authenticated user
- `complete`: generate shell completions (bash, zsh, fish, etc.)
- `readme`: generate a markdown progress block for profile READMEs
- Automatic retry on network errors, timeouts and rate limits

### Fixes
- Retry policy is now aware of *why* a request failed: transient
  network errors, timeouts and 5xx responses use exponential backoff,
  secondary rate limits honor GitHub's `Retry-After` when it is short,
  and an exhausted primary quota fails fast with the actionable message
  instead of burning retries (and quota) on doomed re-attempts
- A user that cannot be queried at all — does not exist or is hidden
  (definitive 404), every source failing (network down, revoked token,
  exhausted quota) — now exits non-zero with the actionable error
  instead of rendering an all-zero report; GitHub's Search API answers
  plain *zero results* for users that do not exist, so the partial
  output would silently lie
- `check --json` and `track --json` expose `partial` and `errors` so
  scripts can tell "genuinely 0" from "source unavailable"
- Starstruck counts the best single repository, not the sum across repos
- Correct achievement tier numbers (Pull Shark: 2/16/128/1024, etc.)
- Colors disable properly when piping to a file or when `NO_COLOR` is set
- System certificate support for corporate proxies
- Request timeouts to avoid hanging
- `--limit 0` no longer sent as-is to the API
- Clear typed errors instead of long stack traces

### Improvements
- Partial results: if one achievement source fails, the rest are still
  shown (a warning goes to stderr and the failure is reported in the
  JSON output)
- Shared progress fetch for `check`/`readme`/`track`, so the commands
  cannot disagree about fallbacks or error handling
- Release workflow now ships binaries for Linux, macOS (Apple Silicon)
  and Windows instead of Linux only
- Username, language and label validation prevents injection into
  search queries
