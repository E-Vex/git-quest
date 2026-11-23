# Development log

Engineering notes for each autonomous development pass on git-quest.
Product-facing changes live in the [CHANGELOG](../CHANGELOG.md); this file
records the reasoning behind them so future maintainers can follow the
decisions.

## 2026-11-23 — v0.2.0 pass

Scope: full architecture review, bug fixing, roadmap completion.

### What was reviewed

- `src/github/` client, typed `ApiError` taxonomy, retry loop
- `src/check.rs`, `src/readme.rs`, `src/display.rs` rendering pipeline
- `src/validate.rs` input validation (injection surface)
- CI (`.github/workflows/ci.yml`) and release (`.github/workflows/release.yml`)

### Problems found and fixed

1. **Retry loop wasted quota and ignored GitHub's signals** (`fix(api)`)
   The old loop retried *every* rate limit after a flat 1s sleep. An
   exhausted hourly quota was hit with 3 doomed re-attempts (each one
   against an empty budget, risking abuse detection), `Retry-After` on
   secondary limits was ignored, and transient 5xx — the most retryable
   class of all — was never retried. The policy is now derived from the
   error kind: exponential backoff for network/timeout/5xx, `Retry-After`
   honored when short (≤ 10s), fail-fast on exhausted quotas.
   Rationale: a CLI should surface GitHub's actionable message in
   seconds, not burn 4 requests proving a quota is still empty.

2. **All-zero reports for users that cannot be queried** (`feat(track)` commit)
   GitHub Search answers plain *zero results* for users that do not
   exist, so `check nonexistent` rendered `0/2 to unlock` rows and exited
   0 — a silent lie to humans and scripts. Two rules were added:
   - a definitive 404 on `/users/{user}/repos` means the user does not
     exist (or is hidden) → exit non-zero with the spelling hint;
   - if every attempted source fails (network down, revoked token,
     exhausted quota) → exit non-zero with the representative error.
   Individual source failures keep the documented partial-results
   behaviour (stderr warning + `errors` in JSON).

3. **`check --json` could not distinguish "genuinely 0" from "source
   unavailable"** — added `partial` and `errors` fields to `check`/`track`
   JSON so scripts can react to degraded runs.

4. **~60 lines of fetch/fallback logic duplicated** between `check.rs`
   and `readme.rs` — extracted into `src/progress.rs` (one
   `ProgressReport`, one `fetch`, one `achievement_json`), so the three
   report commands can never drift apart on fallback behaviour.

5. **`validate::label` had a copy-pasted doc comment and zero tests** —
   fixed the docs and added the missing coverage (username and language
   were already tested; label was the gap).

### Roadmap feature delivered

- **`track`** (v0.2.0 roadmap item): a one-shot "what's next" report —
  tiers at most `--reach N` (default 5) steps away, sorted by proximity,
  gold-complete profiles celebrated separately. Implemented on the shared
  report pipeline with `--json` support and 10 new tests.

### Tooling

- Release workflow upgraded from a single Ubuntu binary to a matrix
  (Linux x86_64, macOS Apple Silicon, Windows x86_64) with sha256
  checksums, plus fmt/clippy gates before packaging. First run failed at
  the upload step: the rewrite had dropped the original
  `permissions: contents: write` block, so `action-gh-release` could not
  create the release — restored, tag re-cut, release verified with all
  six assets (three binaries + checksums).
- Dependabot branches `clap-4.6.8` and `clap_complete-4.6.12` merged.

### Verification

- 104 tests, `cargo fmt --check`, `cargo clippy -- -D warnings`,
  `cargo build --locked` all green; live smoke tests against
  api.github.com for `check`, `track`, `readme --json`, `hunt`,
  validation rejections, missing-user errors and completions.
- CI green on `main`; `v0.2.0` tagged and released with binaries.

### Deliberately left alone

- Starstruck/Galaxy Brain approximations (documented in README) — the
  exact counts GitHub uses are not exposed by any API.
- crates.io / Homebrew publishing — requires maintainer credentials.
