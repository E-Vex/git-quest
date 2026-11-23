<div align="center">

# 🎯 git-quest

**The missing CLI for GitHub Achievements.**

See exactly how close you are to Pull Shark, Starstruck, Galaxy Brain and
Pair Extraordinaire — and hunt `good first issue` tasks that get you there.

[![CI](https://github.com/E-Vex/git-quest/actions/workflows/ci.yml/badge.svg)](https://github.com/E-Vex/git-quest/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

`Rust` · single binary · zero config · no database

</div>

---

## The problem

GitHub gamified developer profiles with **Achievements**, then provided zero
tracking for them. Developers keep asking:

- *How close am I to Pull Shark Bronze?*
- *Why am I not getting Starstruck?*
- *Where do I even find legitimate `good first issue` tasks?*

No single tool aggregates that data and hands you a next step.
**git-quest is that tool.**

## Install

Published to crates.io soon — until then, build from source (needs
[Rust](https://rustup.rs)):

```bash
git clone https://github.com/E-Vex/git-quest.git
cd git-quest
cargo install --path .
```

## Usage

### `check` — your achievement progress

```bash
git-quest check <username>          # check progress for a user
git-quest check                      # check progress for token owner
```

```text
Achievement progress for E-Vex

🦈 Pull Shark           66/128 to Silver  ██████░░░░░░  62 merged PRs remaining
⭐ Starstruck           13/128 to Silver   ██████████░░  115 stars remaining (best repo: E-Vex/git-quest)
🧠 Galaxy Brain         0/2 to unlock     ░░░░░░░░░░░░  2 accepted answers remaining
👥 Pair Extraordinaire  4/10 to Bronze    █████░░░░░░░  6 co-authored commits remaining
```

### `hunt` — find your next contribution

```bash
git-quest hunt --lang rust                  # top 3, sorted by community reactions
git-quest hunt --lang go --limit 5          # more results
git-quest hunt --lang python --sort created # freshest first
git-quest hunt --lang rust --max-age 90     # only issues opened in the last 90 days
```

Results are real issues only (`is:issue`) in repositories that still
accept contributions (`archived:false`); PRs and archived repos are
filtered out automatically. A `good first issue` labelled years ago is
usually not one anymore — `--max-age N` cuts the search off at issues
created within the last N days.

```text
🎯 Hunt — good first issues for `rust`

 1. Avoiding async entirely
    📦 rust-lang/wg-async  👍 60  💬 16  📅 2021-03-19
    https://github.com/rust-lang/wg-async/issues/238
```

### `track` — what's within reach

```bash
git-quest track <username>          # tiers you could hit soon
git-quest track --reach 10          # widen the window
git-quest track                     # token owner (needs GITHUB_TOKEN)
```

```text
🎯 Track — tiers within reach (≤5 more) for E-Vex

 1. 👥 Pair Extraordinaire 6 co-authored commits remaining (4/10 to Bronze)

Further out:
 🦈 Pull Shark 62 merged PRs remaining (66/128 to Silver)
```

### `track --watch` — a live dashboard

```bash
git-quest track --watch                     # refresh every 60s until Ctrl+C
git-quest track --watch --interval 5m       # slower cadence (90s, 5m, 2h, …)
git-quest track --watch --json              # one JSON event per refresh (NDJSON)
git-quest track --watch --reach 10          # widen the window, then watch it
```

On a terminal the frame is redrawn with a countdown and highlights what
changed since the previous refresh (`↑1` next to the achievement):

```text
🎯 Track — watching E-Vex (reach ≤5, every 5m)
next refresh in 4m53s · Ctrl+C to stop

 1. 👥 Pair Extraordinaire ↑1 5 co-authored commits remaining (5/10 to Bronze)

Further out:
 ⭐ Starstruck 12 stars remaining (4/16 to unlock)
```

- One refresh costs 3–4 API requests, so the interval has enforced
  floors: **20s with a token, 180s without** (the unauthenticated core
  quota is only 60 requests/hour). Anything faster is rejected with the
  quota math in the message.
- A refresh that fails keeps the previous snapshot on screen and says so;
  the first refresh failing is a hard error, exactly like the one-shot
  report. Ctrl+C exits cleanly with a summary.
- `--watch --json` emits one compact JSON object per line: `event:
  "refresh"` carries the same fields as `track --json` plus `deltas` and
  `interval_secs`; a failed refresh emits `event: "refresh_failed"`.
  Counts whose source failed in that refresh are never given a delta —
  their 0 is a placeholder, not an observation.

## Achievements tracked

| Achievement | Measures | Default | Bronze | Silver | Gold |
|---|---|---:|---:|---:|---:|
| 🦈 Pull Shark | merged pull requests you authored | 2 | 16 | 128 | 1024 |
| ⭐ Starstruck | highest star count on a single repository you own | 16 | 128 | 512 | 4096 |
| 🧠 Galaxy Brain | accepted answers in Discussions | 2 | 8 | 16 | 32 |
| 👥 Pair Extraordinaire | commits co-authored with someone else | 1 | 10 | 24 | 48 |

## Authentication (optional, recommended)

Without a token you get 60 requests/hour. With one: 5000/hour plus Galaxy
Brain tracking (Discussions answers need the GraphQL API).

```bash
export GITHUB_TOKEN=ghp_xxx   # create at https://github.com/settings/tokens
```

No scopes needed for public data — leave every checkbox unchecked.

## How it counts

- **Pull Shark** — GitHub Search: `is:merged type:pr author:<you>`
- **Starstruck** — highest `stargazers_count` on any single repository you own
- **Galaxy Brain** — GraphQL: `repositoryDiscussionComments(onlyAnswers: true)`
- **Pair Extraordinaire** — commit search for commits you authored that carry a
  `Co-Authored-By:` trailer (an approximation of GitHub's internal count)

```bash
git-quest hunt --label "help wanted"     # search any label
git-quest hunt --lang rust --label "good first issue"
```

```bash
git-quest check --json <username>      # raw JSON output
git-quest hunt --json --lang rust
git-quest track --json <username>      # what's within reach, as JSON
git-quest readme --json <username>     # the markdown block, as JSON
```

JSON consumers can rely on the `partial` and `errors` fields in `check`,
`track` and `readme`: when a source failed, the affected counts are 0 or
omitted and `partial` is `true` — a genuine zero is never faked.

```bash
git-quest complete bash > /etc/bash_completion.d/git-quest  # shell completions
```

## Roadmap

- [x] v0.1.0 — `check` + `hunt`
- [x] v0.2.0 — `track`: watch your progress, see when a tier is within reach
- [x] v0.3.0 — `track --watch`: a live dashboard with deltas and quota-safe intervals
- [x] `hunt --label`
- [x] `hunt --max-age`: freshness filter
- [x] `--json`
- [x] `check` without name
- [x] `complete`
- [x] `readme`
- [x] automatic retry
- [x] partial results
- [ ] Publish to crates.io and Homebrew

## Contributing

Issues and PRs are welcome — this repo labels its own `good first issue`s,
and you can find them with the tool itself:

```bash
git-quest hunt --lang rust
```

## License

MIT © [E-Vex](https://github.com/E-Vex)
