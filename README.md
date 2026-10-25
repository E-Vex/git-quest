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
git-quest check <username>
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
```

```text
🎯 Hunt — good first issues for `rust`

 1. Avoiding async entirely
    📦 rust-lang/wg-async  👍 60  💬 16  📅 2021-03-19
    https://github.com/rust-lang/wg-async/issues/238
```

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

## Roadmap

- [x] v0.1.0 — `check` + `hunt`
- [ ] v0.2.0 — `track`: watch your progress, notify when a tier is within reach
- [ ] v0.2.0 — `readme`: generate a progress block for your profile README
- [ ] `help wanted` label support in `hunt`
- [ ] Publish to crates.io and Homebrew

## Contributing

Issues and PRs are welcome — this repo labels its own `good first issue`s,
and you can find them with the tool itself:

```bash
git-quest hunt --lang rust
```

## License

MIT © [E-Vex](https://github.com/E-Vex)
