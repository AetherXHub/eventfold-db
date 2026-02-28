# Code Review: Ticket 1 -- Add dependencies and implement `src/auth.rs` with unit tests

**Ticket:** 1 -- Add dependencies and implement `src/auth.rs` with unit tests
**Impl Report:** prd/019-jwt-authentication-interceptor-reports/ticket-01-impl.md
**Date:** 2026-02-28 14:30
**Verdict:** APPROVED

---

## AC Coverage

| AC # | Description | Status | Notes |
|------|-------------|--------|-------|
| 1 | `Cargo.toml` contains `jsonwebtoken = "9"` under `[dependencies]` | Met | Line 19 of Cargo.toml: `jsonwebtoken = "9"` |
| 2 | `Cargo.toml` contains `serde = { version = "1", features = ["derive"] }` under `[dependencies]` | Met | Line 20 of Cargo.toml: `serde = { version = "1", features = ["derive"] }` |
| 3 | `src/auth.rs` declares `pub struct JwtInterceptor` with private fields, derives `Clone` | Met | Lines 22-26: `#[derive(Clone)] pub struct JwtInterceptor { decoding_key, validation }` -- both fields private |
| 4 | `JwtInterceptor::new(secret: &str) -> Self` with HS256, validate_exp, required claims exp/sub | Met | Lines 38-48: `DecodingKey::from_secret(secret.as_bytes())`, `Validation::new(Algorithm::HS256)`, `validate_exp = true`, `leeway = 0`, `required_spec_claims` set to `{"exp", "sub"}` |
| 5 | Private `Claims` struct with `sub: String` and `exp: u64`, derives `serde::Deserialize` | Met | Lines 55-63: struct is private (no `pub`), has correct fields, derives `serde::Deserialize` |
| 6 | `tonic::service::Interceptor` impl with specified behavior | Met | Lines 65-106: (1) reads `authorization` metadata, (2) rejects missing/non-ASCII with "missing authorization header", (3) strips "Bearer " or rejects with "invalid authorization header format", (4) decodes JWT, (5) returns `Ok(request)` or `Err(Status::unauthenticated(e.to_string()))`, (6) logs rejections at `tracing::debug!` |
| 7 | All public items have doc comments | Met | Module-level `//!` docs (lines 1-7), struct doc with example (lines 9-21), `new` method doc (lines 29-37), private `Claims` also documented (lines 51-54) |
| 8 | `src/lib.rs` includes `pub mod auth;` | Met | Line 76 of lib.rs |
| 9 | Test: valid token returns `Ok(_)` | Met | `valid_token_returns_ok` at line 175: correct setup, asserts `result.is_ok()` |
| 10 | Test: missing header returns `Err` with Unauthenticated containing "missing" | Met | `missing_header_returns_unauthenticated` at line 142: asserts code and message substring |
| 11 | Test: wrong secret returns `Err` with Unauthenticated | Met | `wrong_secret_returns_unauthenticated` at line 188: token signed with "wrong-secret", interceptor with "correct-secret", asserts Unauthenticated |
| 12 | Test: expired token returns `Err` with Unauthenticated | Met | `expired_token_returns_unauthenticated` at line 203: uses `now_secs() - 1` (expired 1s ago with zero leeway), asserts Unauthenticated. Ticket says `exp=1` but `now_secs()-1` achieves the same intent. |
| 13 | Test: missing Bearer prefix returns `Err` with Unauthenticated containing "format" | Met | `missing_bearer_prefix_returns_unauthenticated` at line 156: raw token without "Bearer " prefix, asserts code and message substring |
| 14 | Quality gates pass | Met | `cargo build`: clean. `cargo clippy --all-targets --all-features --locked -- -D warnings`: clean. `cargo test`: 301 tests all pass. `cargo fmt --check`: only pre-existing `tests/metrics.rs` trailing newline issue (outside scope). |

## Issues Found

### Critical (must fix before merge)

None.

### Major (should fix, risk of downstream problems)

None.

### Minor (nice to fix, not blocking)

1. **Verbose `required_spec_claims` construction** (`src/auth.rs:43`): The line `["exp", "sub"].iter().map(|s| (*s).to_string()).collect()` works but is more verbose than necessary. `jsonwebtoken` v9 provides `validation.set_required_spec_claims(&["exp", "sub"])` which is cleaner and more idiomatic. Not a bug; purely stylistic.

2. **Expired token test uses `now_secs() - 1` instead of `exp = 1`** (`src/auth.rs:207`): The ticket AC literally says `exp` set to `1` (Unix epoch far in the past). The implementation uses `now_secs() - 1` which produces an expired token that is just 1 second past. Both correctly test expiration rejection, but `exp = 1` is more robust since it is unambiguously in the past regardless of timing edge cases. The current approach works because leeway is explicitly 0. Not blocking.

## Suggestions (non-blocking)

- The doc comment on `Claims` (line 51) is thorough even though the struct is private. This is good practice but note that `#![warn(missing_docs)]` only fires for public items, so the doc comment is voluntary here -- appreciated but not required.

- The `#[allow(dead_code)]` annotations on `Claims` fields are correctly justified. The fields are deserialized by `jsonwebtoken::decode` but never read by application code. The alternative would be `#[allow(dead_code)]` on the struct itself, but per-field attribution is clearer about intent.

## Scope Check

- Files within scope: YES
  - Created: `src/auth.rs` (in scope)
  - Modified: `Cargo.toml` (in scope -- added `jsonwebtoken` and `serde`)
  - Modified: `src/lib.rs` (in scope -- added `pub mod auth;`)
  - Modified: `Cargo.lock` (auto-generated, expected side effect)
- Scope creep detected: NO
- Unauthorized dependencies added: NO (`jsonwebtoken = "9"` and `serde = { version = "1", features = ["derive"] }` are both specified by the ticket)

## Risk Assessment

- Regression risk: LOW -- No existing code was modified beyond adding a `pub mod auth;` declaration to `lib.rs` and two dependency lines to `Cargo.toml`. All 301 existing tests continue to pass.
- Security concerns: NONE -- The interceptor correctly uses `jsonwebtoken` for all crypto operations (no custom crypto). Zero leeway is explicitly set to avoid the library's 60-second default. Error messages do not leak key material. Rejections are logged at `debug!` level (not `warn!`/`error!`) to avoid log spam from probes.
- Performance concerns: NONE -- JWT decoding is a lightweight operation (HMAC-SHA256 verification) performed once per gRPC call. The `to_owned()` on the authorization header value (line 70) allocates a string, but this is necessary because `value.to_str()` returns a borrow tied to the request metadata, and the implementation needs to pass the token to `jsonwebtoken::decode` after the borrow ends. No concern at this scale.
