# Development log

Engineering notes for each autonomous development pass on git-quest.
Product-facing changes live in the [CHANGELOG](../CHANGELOG.md); this file
records the reasoning behind them so future maintainers can follow the
decisions.

## 2026-11-23 — v0.3.0 pass

Scope: the quality pass suggested at the end of v0.2.1 — `track --watch`
plus a `hunt` freshness filter. crates.io / Homebrew publishing still
requires maintainer credentials and remains open on the roadmap.

### `track --watch` — design decisions

1. **Interval floors come from quota math, not taste** (`feat(track)`)
   One refresh costs 3 requests without a token (two Search API calls +
   the repository listing) and 4 with (plus GraphQL). GitHub's limits are
   60 core requests/hour unauthenticated and 30 Search requests/minute
   authenticated, so the safe minimums are 180s and 20s respectively — an
   unauthenticated 10s interval would burn the hourly core quota in about
   four minutes. `parse_interval` accepts `s`/`m`/`h` suffixes, rejects
   zero (a watch that never sleeps hammers the API) and caps at 24h (one
   that sleeps a day is not a watch). Multibyte suffix lookalikes fall
   into the bare-number path and produce a clean error, never a panic on
   a UTF-8 boundary.

2. **The username is resolved once, not per refresh**
   `track` without a username resolves the token owner through `/user`;
   doing that every refresh would waste one request per poll, so the
   watch resolves before the loop starts. This required exposing
   `progress::resolve_username` to the crate.

3. **Delta honesty: failed sources produce no delta** (`feat(track)`)
   When a source fails, `fetch()` reports the achievement as 0 (the
   documented partial-results contract). A naive diff would display
   "↓66 merged PRs" after a single 500 — a fabricated regression.
   `ProgressReport` now records which achievement sources failed
   (`failed`, crate-internal), `compute_deltas` skips them, and counts
   that appeared or disappeared between snapshots are skipped too (no
   honest comparison exists).

4. **Partial state is rendered inside the frame**
   The interactive frame clears the screen on every redraw, which would
   wipe the stderr warnings `fetch()` prints — so the frame repeats a
   compact "partial data" line itself when `errors` is non-empty.

5. **JSON watch mode is NDJSON, not pretty JSON**
   One compact object per refresh: `event: "refresh"` carries the same
   fields as `track --json` plus `deltas`, `interval_secs` and
   `refreshed_at`; a failed refresh emits `event: "refresh_failed"` with
   the error and the retry cadence. A subtle bug surfaced in testing: the
   event selection initially keyed off the retained report instead of
   what happened on the current refresh, which made `refresh_failed` dead
   code — the loop test observed 12 requests (4 attempts × 3 sources)
   followed by a full `refresh` event, and the match now keys off the
   current outcome.

6. **Interactive vs piped output**
   Screen clearing and the per-second countdown only run when stdout is a
   terminal and `--json` is off; piped output renders one frame per
   refresh (grep-able, log-friendly) and never emits clear sequences.
   Ctrl+C is handled through `tokio::signal` (the new `signal` feature
   pulls `signal-hook-registry`/`errno` into the lock; no MSRV impact,
   verified by building on 1.85) and exits 0 with a summary line.

### `hunt --max-age` — why and how

The README's own example result was dated 2021; a `good first issue`
labelled years ago is usually not one anymore. `--max-age N` appends
`created:>=<date>`. Computing the date needs calendar math; to avoid
pulling a date crate into a deliberately lean dependency tree (the MSRV
history in this file shows why every new crate matters), the
implementation is Howard Hinnant's `civil_from_days` — a dozen lines of
pure integer math, tested against independently verified vectors
including a leap day and a year boundary.

### Deliberately not done

- **More achievements (Quickdraw et al.)** — considered and rejected.
  GitHub exposes no achievements API, and every countable approximation
  is unfaithful: Quickdraw needs item-level created/closed timestamps and
  is capped by the Search API's 1000-result window, so a badge earned
  years ago would read as "not unlocked" — exactly the "silent lie" class
  of bug this project keeps fixing. The four tracked achievements are the
  ones with reliable API queries; revisit only if GitHub ships real
  achievement data.
- crates.io / Homebrew publishing — still requires maintainer credentials.

### Verification

- 139 tests (25 new): interval parsing/floors, delta computation
  (including the failed-source case), NDJSON event shapes, the full watch
  loop against a mock API (two-refresh delta, mid-watch total failure,
  first-refresh hard error, non-interactive text frames), watch frame
  rendering (countdown, cadence, partial state, gold-complete, colored ==
  plain layout), and the `created:>=` qualifier with date vectors.
- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo build
  --locked` green on stable (1.100); `cargo +1.85 build --locked` green
  on the MSRV toolchain.
- Live smoke tests against api.github.com: `check`, `track --watch
  --json` (NDJSON event), `track --watch` piped (one frame per refresh,
  no clear sequences), the interactive TTY countdown (via a pty: "next
  refresh in 19s → 18s → …", five screen clears in six seconds), both
  interval-floor rejections, `hunt --max-age 90` (5,894 fresh results,
  newest 2026-11-06) and a real SIGINT (exit 0, "watch stopped after 1
  refresh(es)").

## 2026-11-23 — v0.2.1 pass

Scope: quality sweep after the v0.2.0 release — bugs found by re-reading
the whole pipeline, JSON-contract consistency, CI hardening, and an
MSRV correction.

### Problems found and fixed

1. **`hunt` could hand the user a pull request** (`fix(hunt)`)
   GitHub's issue search covers issues *and* pull requests, and the hunt
   query did not pin `is:issue`. Verified live before the fix: for
   `language:rust`, 33 open PRs matched the query, so a PR labelled
   `good first issue` could be presented as a contribution task — and
   inflated the reported `total`. Archived repositories (which cannot
   accept contributions) were also in scope. The query now starts with
   `is:issue` and pins `archived:false`; both are documented qualifiers
   and were validated against the live API before committing.
   A follow-up smoke test confirmed zero `/pull/` URLs in the top 20.

2. **A broken token was reported as a missing one** (`fix(progress)`)
   `GithubClient::authenticated_user()` swallowed every failure
   (`.ok()`), so `check` with no username against a revoked or expired
   token answered "no GITHUB_TOKEN configured for authentication" — the
   wrong fix hint for the actual problem. The lookup now returns
   `Ok(None)` only when no token exists, and surfaces the request
   failure with `could not resolve the token owner's login: …` context.
   The distinction is covered at two levels: the client tests (login
   resolved, `Ok(None)` without a token and *without any network
   round-trip*, 401 surfaced with context) and `fetch()`-level tests
   (no username without a token → actionable error; rejected token →
   the real cause).

3. **`readme --json` broke the documented JSON contract** (`feat(readme)`)
   `check --json` and `track --json` expose `partial`/`errors`; `readme
   --json` did not, so a consumer could not tell that the markdown
   block was silently missing an achievement whose source had failed.
   Aligned; the README now says the contract covers all three commands.

### Testing

- 10 new tests (114 total): hunt query qualifiers, `authenticated_user`
  behaviour, and four `fetch()` failure modes exercised against a
  wiremock mock server — missing user (404) → error, every source
  failing → error with the first cause, one source failing → partial
  report with the failed achievement omitted (never faked as zero), and
  the two username-resolution cases.
- `Debug` derived on `AchievementDef`/`Progress`/`ProgressReport` so
  assertion failures print the report instead of a compile error.

### MSRV correction (the important note for future maintainers)

The declared `rust-version = 1.82` was no longer true, and could not be
made true by pinning alone:

- reqwest's `__rustls-ring` feature references `quinn?/ring`, which
  forces cargo to *resolve and lock* the QUIC closure (quinn → rand
  0.10 → chacha20 → cpufeatures 0.3.x) even though `http3` is never
  enabled. cpufeatures 0.3.x is edition-2024, which cargo 1.82 cannot
  even *parse* — so `cargo build --locked` failed on 1.82 while
  succeeding on stable. Pruning the unused closure from the lock does
  not help: any re-resolve (even offline, even a fresh
  `cargo generate-lockfile`) re-adds it, as verified three ways.
- A fresh resolve also floated the `url → idna → idna_adapter` chain
  onto ICU4X 2.4.0, which declares rustc 1.88.

Actions taken:

- `idna_adapter` pinned to 1.2.0 (the ICU4X 1.5 line) so the resolver
  stays off the rustc-1.88 chain.
- `rust-version` corrected to **1.85** — the true build floor
  (edition-2024 crates in the locked graph; verified by building with
  the 1.85 toolchain locally before tagging).
- CI gained an `msrv` job that builds on 1.85. It deliberately does not
  run the test suite there: the dev-dependency wiremock uses if-let
  chains stabilized in rustc 1.88, and dev-dependencies are not part of
  what a user compiles. The job is the guardrail: if a dependency bump
  breaks the MSRV again, CI fails and the choice (pin or bump) is made
  consciously.

### Verification

- 114 tests, `cargo fmt --check`, `cargo clippy -- -D warnings`,
  `cargo build --locked` green on stable (1.100); `cargo build --locked`
  green on 1.85.
- Live smoke tests against api.github.com: `check` (token owner, named
  user, unauthenticated), `track`, `readme --json` (new keys present),
  `hunt` (no PR URLs in results), missing-user error path, and both
  username-resolution error paths (no token vs rejected token).

### Deliberately left alone

- crates.io / Homebrew publishing — still requires maintainer
  credentials.
- wiremock downgrade to make the MSRV job run tests — rejected: the
  user-facing build floor is what `rust-version` describes, and
  contributors run a current stable toolchain anyway.

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
