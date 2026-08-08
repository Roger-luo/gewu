# Gewu Agent Guide

## Product

Gewu is a human-guided code and design review tool. It walks reviewers through
an AI-suggested or user-selected corpus, explains relevant code, and helps
reviewers reach their own conclusions. AI assists navigation and focus; it does
not replace human judgment or publish review conclusions on the user's behalf.

The current interface is a CLI. Keep domain behavior independent of the CLI so
a native macOS application can reuse it later.

## Tooling

`mise` is the source of truth for development tools. After cloning:

```bash
mise install
mise exec -- prek install
```

Common commands:

```bash
mise exec -- cargo build --workspace
mise exec -- cargo test --workspace --all-features
mise exec -- cargo test --workspace test_name
mise run check
mise exec -- prek run --all-files
```

## Architecture

- `src/main.rs`: minimal executable entry point; delegates to `gewu-cli`.
- `src/lib.rs`: public facade containing re-exports only.
- `crates/gewu-cli`: CLI-specific runtime and presentation.
- `crates/gewu-review`: interface-independent review domain.
- Add focused workspace crates as boundaries become clear. Do not put domain
  behavior in the root facade or couple it to terminal UI concerns.

Keep files focused. Consider splitting files larger than 200 lines into a
module, using `mod.rs`. Prefer methods on domain types over unrelated free
functions, narrow imports over many imported names, and borrowing over cloning.

Use `anyhow::Result` at application boundaries and typed `thiserror` errors in
reusable library APIs when dependencies are introduced.

## Required Checks

`prek.toml` is the source of truth for Git hooks. Before handing off changes,
run `mise run check`, which executes:

1. `cargo fmt --all -- --check`
2. `cargo check --workspace --all-targets --all-features`
3. `cargo clippy --workspace --all-targets --all-features -- -D warnings`
4. `cargo test --workspace --all-features`

Never suppress lint failures merely to pass these checks.

## Commit Convention

All commits must follow Conventional Commits (semantic commit messages):
`type(optional-scope)!: description`.

Allowed types are `feat`, `fix`, `docs`, `test`, `ci`, `refactor`, `perf`,
`build`, and `chore`. Use `!` and/or a `BREAKING CHANGE:` footer for breaking
changes. Examples: `feat(cli): add review scope prompt` and
`fix(review)!: reject invalid corpus roots`.
