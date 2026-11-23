# Changelog

## v0.3.0

### Features

- `track --watch`: a live dashboard — the track report re-fetches on an
  interval (default 60s), highlights what changed since the previous
  refresh (`↑2` / `↓1` next to the achievement), shows a countdown to the
  next refresh and exits cleanly on Ctrl+C. A refresh that fails keeps the
  previous snapshot on screen and explains why it did not move; the first
  refresh failing is still a hard error, exactly like the one-shot report.
- `--interval` for `--watch` (`90s`, `5m`, `2h` — bare numbers are
  seconds), with enforced floors that protect the quota: 20s with a token,
  180s without (one refresh costs 3–4 requests and the unauthenticated
  core quota is only 60 requests/hour). Faster intervals are rejected with
  the quota math in the message.
- `track --watch --json`: one NDJSON event per refresh — `event: "refresh"`
  carries the same fields as `track --json` plus `deltas`, `interval_secs`
  and `refreshed_at`; a failed refresh emits `event: "refresh_failed"`.
- `hunt --max-age <days>`: only issues created within the last N days
  (`created:>=<date>` qualifier), because a `good first issue` labelled
  years ago is usually not one anymore. The JSON output reports the
  filter as `max_age_days`.

### Notes

- Delta chips are suppressed for achievements whose source failed in the
  current refresh: their 0 is a placeholder, not an observation, so a
  single 500 can never be displayed as a fabricated `↓N`. Partial state is
  also rendered inside the watch frame — the screen clear would otherwise
  wipe the stderr warnings.

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
