# Tickets for PRD 018: Open-Source Publishing Preparation

**Source PRD:** prd/018-open-source-publishing-prep.md
**Created:** 2026-02-27
**Total Tickets:** 8
**Estimated Total Complexity:** 14 (S=1, M=2, L=3)

---

### Ticket 1: Add LICENSE-MIT and LICENSE-APACHE Files

**Description:**
Create the two license files required for dual-license `MIT OR Apache-2.0` publishing. Both files
must be placed at the repository root. `LICENSE-MIT` uses the standard OSI MIT template with
copyright holder "Foxworks Studios". `LICENSE-APACHE` is the verbatim Apache License 2.0 text
from `https://www.apache.org/licenses/LICENSE-2.0.txt`. These files are required by crates.io
before `cargo publish` can succeed and are a hard blocker for open-source release.

**Scope:**
- Create: `LICENSE-MIT`
- Create: `LICENSE-APACHE`

**Acceptance Criteria:**
- [ ] `LICENSE-MIT` exists at the repo root, contains "MIT", "Foxworks Studios", and the year "2026"; file is at least 1000 bytes.
- [ ] `LICENSE-APACHE` exists at the repo root, contains "Apache License" and "Version 2.0"; file is at least 10000 bytes (the full license text is ~11 KB).
- [ ] Both files use standard, verbatim license text — no modifications to the legal text beyond the copyright holder name in `LICENSE-MIT`.
- [ ] Test: run `grep -c "MIT" LICENSE-MIT` — asserts exit code 0 and count >= 1.
- [ ] Test: run `grep -c "Foxworks Studios" LICENSE-MIT` — asserts exit code 0 and count >= 1.
- [ ] Test: run `grep -c "Apache License" LICENSE-APACHE` — asserts exit code 0.
- [ ] Test: run `grep -c "Version 2.0" LICENSE-APACHE` — asserts exit code 0.
- [ ] Test: run `wc -c < LICENSE-MIT` — asserts output >= 200.
- [ ] Test: run `wc -c < LICENSE-APACHE` — asserts output >= 10000.
- [ ] Quality gates pass: `cargo build --locked`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo test --locked`, `cargo fmt --check`.

**Dependencies:** None
**Complexity:** S
**Maps to PRD AC:** AC 1, AC 2

---

### Ticket 2: Update .gitignore and Both Cargo.toml Metadata Blocks

**Description:**
Three related config edits that guard against accidental data/secret commits and add the
metadata required by crates.io. Append `data/` and `.env` to `.gitignore`. Add `authors`,
`repository`, `keywords`, `categories`, and `rust-version` to the root `Cargo.toml` `[package]`
section (plus `readme = "README.md"` for the root crate only). Apply the same fields (minus
`readme`) to `eventfold-console/Cargo.toml`. Keywords must be exactly five entries, each at most
20 characters and containing only alphanumeric characters, `-`, or `_`.

**Scope:**
- Modify: `.gitignore`
- Modify: `Cargo.toml`
- Modify: `eventfold-console/Cargo.toml`

**Acceptance Criteria:**
- [ ] `.gitignore` contains a line `data/` and a separate line `.env` (each on its own line, unquoted).
- [ ] Root `Cargo.toml` `[package]` section contains `authors`, `repository`, `readme`, `keywords`, `categories`, and `rust-version` fields, all matching the PRD specification verbatim (repository URL, keywords list of exactly 5, `rust-version = "1.85"`).
- [ ] `eventfold-console/Cargo.toml` `[package]` contains the same fields except `readme` is omitted.
- [ ] Test: run `grep -E "^data/$" .gitignore` — asserts exit code 0.
- [ ] Test: run `grep -E "^\.env$" .gitignore` — asserts exit code 0.
- [ ] Test: run `cargo metadata --no-deps --format-version 1` — exits with code 0 and the JSON output contains `"authors":["Foxworks Studios"]` for both packages.
- [ ] Test: run `cargo publish --dry-run --allow-dirty` from the repo root — exits with code 0 (validates all required crates.io metadata fields are present and valid for the root crate). Note: this will attempt a network call to crates.io for the actual publish check; use `--no-verify` only if crates.io is unreachable, but prefer the full dry-run.
- [ ] Quality gates pass: `cargo build --locked`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo test --locked`, `cargo fmt --check`.

**Dependencies:** Ticket 1 (license files must exist before `cargo publish --dry-run` can succeed, as the license file is referenced in `Cargo.toml`).
**Complexity:** M
**Maps to PRD AC:** AC 3, AC 4, AC 5

---

### Ticket 3: Expand src/lib.rs Crate-Level Documentation

**Description:**
Replace the single-line module doc comment at the top of `src/lib.rs` with a comprehensive
multi-paragraph crate-level doc that a first-time Rust library evaluator would find complete.
The expanded doc must contain exactly four labelled sections: a description paragraph, a
`# Quick Start` section with a `tokio::test`-based example, a `# Key Types` section listing
all eight public API types with intra-doc links, and a `# Library vs Binary` section clarifying
the dual-mode crate. The doc must compile without warnings under `RUSTDOCFLAGS="-D warnings"`.

**Scope:**
- Modify: `src/lib.rs`

**Acceptance Criteria:**
- [ ] The top of `src/lib.rs` begins with `//!` doc lines (not `//` or `///`) that form a continuous module-level doc block.
- [ ] The doc block contains a description paragraph (no heading), a `# Quick Start` section, a `# Key Types` section, and a `# Library vs Binary` section — in that order.
- [ ] The `# Quick Start` section contains a fenced Rust code block demonstrating `Store`, `WriterHandle`, and `ProposedEvent` usage using a `#[tokio::test]`-style async example.
- [ ] The `# Key Types` section lists all eight types: `Store`, `WriterHandle`, `ReadIndex`, `Broker`, `ProposedEvent`, `RecordedEvent`, `ExpectedVersion`, and `Error` — each with an intra-doc link (e.g., `[Store]`) and a one-line description.
- [ ] The `# Library vs Binary` section explains that the crate ships both a library and a standalone server binary, and that the binary is not suitable for embedding.
- [ ] Test: run `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked` — exits with code 0 (no doc warnings).
- [ ] Test: run `grep -c "# Quick Start" src/lib.rs` — asserts output >= 1.
- [ ] Test: run `grep -c "# Key Types" src/lib.rs` — asserts output >= 1.
- [ ] Test: run `grep -c "# Library vs Binary" src/lib.rs` — asserts output >= 1.
- [ ] Test: run `grep -c "\[Store\]" src/lib.rs` — asserts output >= 1 (intra-doc link present).
- [ ] Quality gates pass: `cargo build --locked`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo test --locked`, `cargo fmt --check`.

**Dependencies:** None (all referenced public types already exist in the codebase).
**Complexity:** M
**Maps to PRD AC:** AC 6

---

### Ticket 4: Create CHANGELOG.md

**Description:**
Create `CHANGELOG.md` at the repository root following the Keep a Changelog format. The initial
file contains only the standard header and an `[Unreleased]` section with no entries. This file
establishes the changelog convention for future releases and satisfies the crates.io publishing
practice of having a change history.

**Scope:**
- Create: `CHANGELOG.md`

**Acceptance Criteria:**
- [ ] `CHANGELOG.md` exists at the repo root.
- [ ] File contains the string `[Unreleased]`.
- [ ] File contains the string `Keep a Changelog` (in the link reference per the format spec).
- [ ] File contains the string `Semantic Versioning` (in the adherence line).
- [ ] The format matches the Keep a Changelog 1.0.0 template exactly as specified in the PRD.
- [ ] Test: run `grep -c "\[Unreleased\]" CHANGELOG.md` — asserts exit code 0 and count >= 1.
- [ ] Test: run `grep -c "Keep a Changelog" CHANGELOG.md` — asserts exit code 0 and count >= 1.
- [ ] Quality gates pass: `cargo build --locked`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo test --locked`, `cargo fmt --check`.

**Dependencies:** None
**Complexity:** S
**Maps to PRD AC:** AC 7

---

### Ticket 5: Add Badges, Library Usage Section, and Console Section to README.md

**Description:**
Three additive edits to `README.md`. Insert a four-badge block (CI, crates.io, docs.rs, license)
immediately after the `# EventfoldDB` heading and before the existing prose paragraph. Add a new
`## Library Usage` section before `## Building` containing a `cargo add eventfold-db` snippet and
a pointer to docs.rs. Add a new `## Console` section after `## Running` that mentions the
`eventfold-console/` sub-crate and its `--addr` flag. No existing content should be removed or
rewritten.

**Scope:**
- Modify: `README.md`

**Acceptance Criteria:**
- [ ] `README.md` contains the CI badge line referencing `ci.yml` exactly as specified in the PRD: `[![CI](https://github.com/Foxworks-Studios/eventfold-db/actions/workflows/ci.yml/badge.svg)](...)`.
- [ ] `README.md` contains a fenced code block with the text `cargo add eventfold-db`.
- [ ] `README.md` contains a section heading that mentions `eventfold-console` (e.g., `## Console`).
- [ ] `README.md` contains a mention of `docs.rs/eventfold-db` in the Library Usage section.
- [ ] The badges block appears immediately after `# EventfoldDB` and before the first prose paragraph (verified by inspecting line order in the file).
- [ ] `## Library Usage` section appears before `## Building` section in the file.
- [ ] `## Console` section appears after `## Running` section in the file.
- [ ] No pre-existing content in `README.md` is removed or altered.
- [ ] Test: run `grep -c "ci.yml" README.md` — asserts exit code 0 and count >= 1.
- [ ] Test: run `grep -c "cargo add eventfold-db" README.md` — asserts exit code 0 and count >= 1.
- [ ] Test: run `grep -c "eventfold-console" README.md` — asserts exit code 0 and count >= 1.
- [ ] Quality gates pass: `cargo build --locked`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo test --locked`, `cargo fmt --check`.

**Dependencies:** None (the CI workflow filename `ci.yml` is known from the PRD spec; Ticket 6 will create the actual file).
**Complexity:** M
**Maps to PRD AC:** AC 8

---

### Ticket 6: Create .github/workflows/ci.yml GitHub Actions Workflow

**Description:**
Create the CI workflow file at `.github/workflows/ci.yml`. The workflow triggers on `push` and
`pull_request` events targeting `main`. It runs a single job (`ci`) on `ubuntu-latest` using the
stable Rust toolchain with `clippy` and `rustfmt` components, plus `Swatinem/rust-cache@v2` for
dependency caching. The job executes five cargo quality gates in order: `build`, `test`, `clippy`,
`fmt --check`, `doc --no-deps`. All cargo invocations must use `--locked`. The job-level env var
`RUSTDOCFLAGS: "-D warnings"` must be set so doc warnings fail the build.

**Scope:**
- Create: `.github/workflows/ci.yml`
- Create: `.github/workflows/` directory (if it does not exist)

**Acceptance Criteria:**
- [ ] `.github/workflows/ci.yml` exists and is valid YAML (parseable without error).
- [ ] The `on:` block includes `push` and `pull_request` events both filtered to `branches: [main]`.
- [ ] The job uses `runs-on: ubuntu-latest`.
- [ ] Steps include `actions/checkout@v4`, `dtolnay/rust-toolchain@stable` with `components: clippy, rustfmt`, and `Swatinem/rust-cache@v2`.
- [ ] Steps include `cargo build --locked`, `cargo test --locked`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo fmt --check`, and `cargo doc --no-deps --locked`.
- [ ] `--locked` appears in every `cargo` invocation in the workflow file.
- [ ] `RUSTDOCFLAGS: "-D warnings"` is set at the job level (not just a step level).
- [ ] Test: run `grep -c "\-\-locked" .github/workflows/ci.yml` — asserts exit code 0 and count >= 5 (one per cargo command).
- [ ] Test: run `grep -c "cargo build" .github/workflows/ci.yml` — asserts exit code 0 and count >= 1.
- [ ] Test: run `grep -c "cargo test" .github/workflows/ci.yml` — asserts exit code 0 and count >= 1.
- [ ] Test: run `grep -c "cargo clippy" .github/workflows/ci.yml` — asserts exit code 0 and count >= 1.
- [ ] Test: run `grep -c "cargo fmt" .github/workflows/ci.yml` — asserts exit code 0 and count >= 1.
- [ ] Test: run `grep -c "cargo doc" .github/workflows/ci.yml` — asserts exit code 0 and count >= 1.
- [ ] Test: run `grep -c "RUSTDOCFLAGS" .github/workflows/ci.yml` — asserts exit code 0 and count >= 1.
- [ ] Quality gates pass: `cargo build --locked`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo test --locked`, `cargo fmt --check`.

**Dependencies:** None (YAML file; has no Rust dependency).
**Complexity:** M
**Maps to PRD AC:** AC 9

---

### Ticket 7: Create PRD 009 Status File

**Description:**
Create `prd/009-console-tui-status.md` to record that PRD 009 is complete. The file should
document that all acceptance criteria for the console TUI passed, that the `eventfold-console`
crate is present at `eventfold-console/`, and that the workspace builds cleanly. This closes the
loop on PRD 009's DRAFT status, which was never formally updated despite the implementation being
complete (as evidenced by the `eventfold-console/` directory existing in the repository and the
workspace building successfully).

**Scope:**
- Create: `prd/009-console-tui-status.md`

**Acceptance Criteria:**
- [ ] `prd/009-console-tui-status.md` exists.
- [ ] The file contains the word "complete" (case-insensitive).
- [ ] The file references the `eventfold-console` crate and confirms the workspace builds.
- [ ] The file follows the same status file format used by other completed PRDs in `prd/` (e.g., `prd/008-batch-atomicity-crash-consistency-status.md`).
- [ ] Test: run `grep -ic "complete" prd/009-console-tui-status.md` — asserts exit code 0 and count >= 1.
- [ ] Test: run `grep -c "eventfold-console" prd/009-console-tui-status.md` — asserts exit code 0 and count >= 1.
- [ ] Quality gates pass: `cargo build --locked`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo test --locked`, `cargo fmt --check`.

**Dependencies:** None
**Complexity:** S
**Maps to PRD AC:** AC 10

---

### Ticket 8: Verification and Integration Check

**Description:**
Run the full PRD 018 acceptance criteria checklist end-to-end. Verify that all tickets integrate
correctly — license files are present and valid, `.gitignore` guards data and secrets, both
`Cargo.toml` files have complete crates.io metadata, `src/lib.rs` docs compile warning-free,
`CHANGELOG.md` follows Keep a Changelog, `README.md` has badges and usage sections, the CI
workflow contains all five quality gates with `--locked`, the PRD 009 status file is present,
and all five cargo quality gates pass across the full workspace.

**Acceptance Criteria:**
- [ ] `grep -c "Foxworks Studios" LICENSE-MIT` returns count >= 1 with exit code 0.
- [ ] `grep -c "Apache License" LICENSE-APACHE` returns count >= 1 with exit code 0.
- [ ] `grep -E "^data/$" .gitignore` exits with code 0.
- [ ] `grep -E "^\.env$" .gitignore` exits with code 0.
- [ ] `cargo publish --dry-run --allow-dirty` exits with code 0 for the root crate (validates crates.io metadata).
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked` exits with code 0 (no doc warnings).
- [ ] `grep -c "\[Unreleased\]" CHANGELOG.md` returns count >= 1 with exit code 0.
- [ ] `grep -c "ci.yml" README.md` returns count >= 1 with exit code 0.
- [ ] `grep -c "cargo add eventfold-db" README.md` returns count >= 1 with exit code 0.
- [ ] `grep -c "eventfold-console" README.md` returns count >= 1 with exit code 0.
- [ ] `grep -c "\-\-locked" .github/workflows/ci.yml` returns count >= 5 with exit code 0.
- [ ] `grep -ic "complete" prd/009-console-tui-status.md` returns count >= 1 with exit code 0.
- [ ] `cargo build --locked` exits with code 0 and zero warnings.
- [ ] `cargo test --locked` exits with code 0 (all tests green, no regressions).
- [ ] `cargo clippy --all-targets --all-features --locked -- -D warnings` exits with code 0.
- [ ] `cargo fmt --check` exits with code 0.
- [ ] No previously passing tests have regressed.

**Dependencies:** All previous tickets (1 through 7).
**Complexity:** M
**Maps to PRD AC:** AC 1, AC 2, AC 3, AC 4, AC 5, AC 6, AC 7, AC 8, AC 9, AC 10, AC 11

---

## AC Coverage Matrix

| PRD AC # | Description | Covered By Ticket(s) | Status |
|----------|-------------|----------------------|--------|
| 1 | `LICENSE-MIT` exists, contains "MIT" and "Foxworks Studios", is at least 200 bytes | Ticket 1, Ticket 8 | Covered |
| 2 | `LICENSE-APACHE` exists, contains "Apache License" and "Version 2.0" | Ticket 1, Ticket 8 | Covered |
| 3 | `.gitignore` contains both `data/` and `.env` lines | Ticket 2, Ticket 8 | Covered |
| 4 | Root `Cargo.toml` has all required metadata fields; `cargo publish --dry-run --allow-dirty` exits 0 | Ticket 2, Ticket 8 | Covered |
| 5 | `eventfold-console/Cargo.toml` has all required metadata fields (no `readme`) | Ticket 2, Ticket 8 | Covered |
| 6 | `src/lib.rs` contains all four doc sections; `cargo doc --no-deps` with `RUSTDOCFLAGS="-D warnings"` exits 0 | Ticket 3, Ticket 8 | Covered |
| 7 | `CHANGELOG.md` exists, contains `[Unreleased]` and `Keep a Changelog` | Ticket 4, Ticket 8 | Covered |
| 8 | `README.md` contains CI badge line, `cargo add` block, and `eventfold-console` section heading | Ticket 5, Ticket 8 | Covered |
| 9 | `.github/workflows/ci.yml` exists with all five quality-gate steps and `--locked` on every cargo invocation | Ticket 6, Ticket 8 | Covered |
| 10 | `prd/009-console-tui-status.md` exists and contains "complete" (case-insensitive) | Ticket 7, Ticket 8 | Covered |
| 11 | All four quality gates (`cargo build --locked`, `cargo test --locked`, `cargo clippy`, `cargo fmt --check`) exit 0 after all changes | Ticket 8 | Covered |
