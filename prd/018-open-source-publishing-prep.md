# PRD 018: Open-Source Publishing Preparation

**Status:** TICKETS READY
**Created:** 2026-02-27
**Author:** PRD Writer Agent

---

## Problem Statement

EventfoldDB is functionally complete through PRD 009, but the repository is not ready for public release. License files are absent, `Cargo.toml` metadata fields required by crates.io are missing, `data/` and `.env` are unguarded in `.gitignore`, CI does not exist, the crate-level documentation is too sparse for a public audience, and PRD 009 was completed but still carries a DRAFT status. These gaps must be resolved before the repository can be pushed to GitHub and published to crates.io.

## Goals

- All hard blockers that would prevent `cargo publish` or a clean GitHub push are resolved before implementation is marked complete.
- The GitHub Actions CI workflow executes all five quality gates (`cargo build`, `cargo test`, `cargo clippy`, `cargo fmt --check`, `cargo doc --no-deps`) on every push and pull request to `main`.
- Both crates (`eventfold-db` and `eventfold-console`) have complete, crates.io-compatible `Cargo.toml` metadata.
- A first-time visitor to the repository can understand what EventfoldDB is, how to install it, and how to run it from the README alone.

## Non-Goals

- Releasing to crates.io (the PRD covers publish-readiness, not the act of publishing).
- Adding new features, RPCs, or runtime behavior of any kind.
- Writing a contributor guide (`CONTRIBUTING.md`) or security policy (`SECURITY.md`).
- Setting up GitHub repository settings, branch protection rules, or secrets.
- Adding `rust-version` enforcement via `Cargo.lock` or MSRV-aware CI matrix jobs beyond the single stable job.
- Creating a `docs.rs` custom landing page (`[package.metadata.docs.rs]`).

## User Stories

- As a potential adopter browsing GitHub, I want to see badges for CI status, crates.io version, docs.rs, and license so I can quickly assess the project's health and viability.
- As a Rust developer evaluating the library, I want `cargo add eventfold-db` and working usage examples in `src/lib.rs` so I can understand the public API without reading source files.
- As a contributor opening a PR, I want CI to run automatically so I receive immediate feedback that my change compiles, passes tests, and meets style requirements.
- As a legal/compliance reviewer, I want `LICENSE-MIT` and `LICENSE-APACHE` files at the repo root so I can verify the dual-license terms without following external links.
- As a developer cloning the repository, I want `data/` and `.env` in `.gitignore` so that runtime data files and local secrets are never accidentally committed.

## Technical Approach

### Files affected

| File | Action | Notes |
|------|--------|-------|
| `LICENSE-MIT` | Create | Standard MIT text, copyright holder: Foxworks Studios |
| `LICENSE-APACHE` | Create | Apache 2.0 full text (copy from apache.org/licenses/LICENSE-2.0.txt) |
| `.gitignore` | Edit | Append `data/` and `.env` lines |
| `Cargo.toml` (root) | Edit | Add `repository`, `readme`, `keywords`, `categories`, `rust-version`, `authors` |
| `eventfold-console/Cargo.toml` | Edit | Same set of metadata fields |
| `src/lib.rs` | Edit | Expand module-level doc comment |
| `CHANGELOG.md` | Create | Keep a Changelog format, single `[Unreleased]` section initially |
| `README.md` | Edit | Add badges block, `cargo add` snippet, mention `eventfold-console` |
| `.github/workflows/ci.yml` | Create | GitHub Actions workflow |
| `prd/009-console-tui-status.md` | Create | Status file marking PRD 009 complete |

### LICENSE files

`LICENSE-MIT` uses the standard OSI MIT template with `Copyright (c) 2026 Foxworks Studios`.
`LICENSE-APACHE` is the verbatim Apache License 2.0 text from `https://www.apache.org/licenses/LICENSE-2.0.txt`.

### .gitignore additions

Append two lines to the existing `.gitignore` (which currently contains only `/target`):

```
data/
.env
```

### Cargo.toml metadata (both crates)

Fields to add to the `[package]` section of both `Cargo.toml` files:

```toml
authors      = ["Foxworks Studios"]
repository   = "https://github.com/Foxworks-Studios/eventfold-db"
readme       = "README.md"                    # root crate only; omit for eventfold-console
keywords     = ["event-store", "event-sourcing", "cqrs", "grpc", "database"]
categories   = ["database", "asynchronous", "network-programming"]
rust-version = "1.85"
```

`eventfold-console/Cargo.toml` uses the same values except `readme` is omitted (the sub-crate has no separate README).

The `keywords` array must contain five or fewer entries; the list above has exactly five. Each keyword must be 20 characters or fewer and contain only alphanumeric characters, `-`, or `_`.

### src/lib.rs crate-level docs

Replace the single-line doc comment at the top of `src/lib.rs` with a multi-paragraph module doc that includes:

1. A one-paragraph description of what EventfoldDB is and is not.
2. A "# Quick Start" section showing how to open a `Store`, create a `WriterHandle`, and append a `ProposedEvent` using a `tokio::test` pattern.
3. A "# Key Types" section listing `Store`, `WriterHandle`, `ReadIndex`, `Broker`, `ProposedEvent`, `RecordedEvent`, `ExpectedVersion`, and `Error` with one-line descriptions and intra-doc links (`[Store]`, `[WriterHandle]`, etc.).
4. A "# Library vs Binary" section explaining that the crate ships both a library (the storage engine and gRPC service) and a standalone server binary, and that the binary is not suitable for embedding.

### CHANGELOG.md

Create `CHANGELOG.md` at the repo root following the [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) format with a single `[Unreleased]` section:

```markdown
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
```

### README.md changes

Two additions to the existing README:

1. **Badges block** — insert immediately after the `# EventfoldDB` heading, before the prose paragraph:

```markdown
[![CI](https://github.com/Foxworks-Studios/eventfold-db/actions/workflows/ci.yml/badge.svg)](https://github.com/Foxworks-Studios/eventfold-db/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/eventfold-db.svg)](https://crates.io/crates/eventfold-db)
[![docs.rs](https://docs.rs/eventfold-db/badge.svg)](https://docs.rs/eventfold-db)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
```

2. **Library usage section** — add a new `## Library Usage` section before the existing `## Building` section, containing:
   - A `cargo add` snippet: `cargo add eventfold-db`
   - A brief note pointing readers to `docs.rs/eventfold-db` for full API docs

3. **eventfold-console mention** — add a new `## Console` section after `## Running` pointing readers to the `eventfold-console/` sub-crate and its `--addr` flag.

### GitHub Actions CI workflow

Create `.github/workflows/ci.yml`. The workflow runs on `push` and `pull_request` events targeting the `main` branch. It uses a single job (`ci`) on `ubuntu-latest` with a matrix-free stable Rust toolchain. Steps in order:

1. `actions/checkout@v4`
2. `dtolnay/rust-toolchain@stable` with `components: clippy, rustfmt`
3. Cache: `Swatinem/rust-cache@v2`
4. `cargo build --locked`
5. `cargo test --locked`
6. `cargo clippy --all-targets --all-features --locked -- -D warnings`
7. `cargo fmt --check`
8. `cargo doc --no-deps --locked`

The workflow file must use `--locked` on all cargo invocations to enforce `Cargo.lock` reproducibility. `RUSTDOCFLAGS: "-D warnings"` must be set as a job-level env var so `cargo doc` fails on doc warnings.

### PRD 009 status file

Create `prd/009-console-tui-status.md` with content that marks PRD 009 as complete: all nine acceptance criteria passed, the `eventfold-console` crate is present at `eventfold-console/` and the workspace builds cleanly.

## Acceptance Criteria

1. `LICENSE-MIT` exists at the repo root, contains the word "MIT" and "Foxworks Studios", and is at least 200 bytes long.
2. `LICENSE-APACHE` exists at the repo root and contains the phrase "Apache License" and "Version 2.0".
3. Running `grep -E "^(data/|\.env)" .gitignore` returns both `data/` and `.env` as matches.
4. `cargo metadata --no-deps --format-version 1 | jq '.packages[] | select(.name=="eventfold-db") | .metadata'` — the root `Cargo.toml` `[package]` contains `repository`, `readme`, `keywords` (exactly 5 entries), `categories`, `rust-version = "1.85"`, and `authors` fields; `cargo publish --dry-run --allow-dirty` exits with code 0 for the root crate.
5. `cargo metadata --no-deps --format-version 1 | jq '.packages[] | select(.name=="eventfold-console") | .metadata'` — the `eventfold-console/Cargo.toml` `[package]` contains `repository`, `keywords`, `categories`, `rust-version = "1.85"`, and `authors` fields.
6. `src/lib.rs` module-level doc comment (lines beginning with `//!`) contains all four sections: a description paragraph, `# Quick Start`, `# Key Types`, and `# Library vs Binary`; and `cargo doc --no-deps` exits with code 0 with `RUSTDOCFLAGS="-D warnings"`.
7. `CHANGELOG.md` exists at the repo root, contains the string `[Unreleased]`, and contains the string `Keep a Changelog`.
8. `README.md` contains a badge line referencing `ci.yml`, a `cargo add eventfold-db` code block, and a section heading that mentions `eventfold-console`.
9. `.github/workflows/ci.yml` exists and contains all five quality-gate steps: `cargo build`, `cargo test`, `cargo clippy`, `cargo fmt`, `cargo doc`; and `--locked` appears in each cargo invocation.
10. `prd/009-console-tui-status.md` exists and contains the word "complete" (case-insensitive).
11. `cargo build --locked`, `cargo test --locked`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, and `cargo fmt --check` all exit with code 0 after all changes are applied.

## Open Questions

- **Copyright holder name**: "Foxworks Studios" is used throughout based on the repository URL. Verify the exact legal name before publishing.
- **crates.io publish token**: Not covered by this PRD. A `CARGO_REGISTRY_TOKEN` GitHub secret will be needed at publish time.
- **`eventfold-console` publishability**: The console crate depends on `eventfold-db` via `path = ".."`. This path dependency must be changed to a version dependency before the console crate can be published. This PRD does not resolve that dependency since it is deferred to the actual publish step.

## Dependencies

- PRDs 001–009 (all complete — the full feature set is implemented)
- No new runtime dependencies introduced by this PRD
- `dtolnay/rust-toolchain`, `actions/checkout`, `Swatinem/rust-cache` GitHub Actions are external dependencies for CI
