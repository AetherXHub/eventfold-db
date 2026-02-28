# Code Review: Ticket 2 -- Wire `JwtInterceptor` into `src/main.rs`

**Ticket:** 2 -- Wire `JwtInterceptor` into `src/main.rs`
**Impl Report:** prd/019-jwt-authentication-interceptor-reports/ticket-02-impl.md
**Date:** 2026-02-28 17:15
**Verdict:** APPROVED

---

## AC Coverage

| AC # | Description | Status | Notes |
|------|-------------|--------|-------|
| 1 | `Config` gains `jwt_secret: Option<String>`. `from_env` reads `EVENTFOLD_JWT_SECRET`: non-empty -> `Some(s)`, absent/empty -> `None`. | Met | `src/main.rs:57`: field `jwt_secret: Option<String>` on `Config`. Lines 183-187: `match std::env::var("EVENTFOLD_JWT_SECRET")` with guard `!val.is_empty()` -> `Some(val)`, wildcard -> `None`. Handles all three cases (set+non-empty, set+empty, absent) correctly. |
| 2 | In `main`, match on `config.jwt_secret`: wraps with `InterceptedService` or adds unwrapped with `tracing::warn!`. | Met | Lines 361-368: `match config.jwt_secret { Some(ref secret) => { InterceptedService::new(EventStoreServer::new(service), interceptor) } None => router.add_service(EventStoreServer::new(service)) }`. Both branches produce the same `Router` type via `add_service` type-erasure. Import at line 8: `use tonic::service::interceptor::InterceptedService`. |
| 3 | `tracing::warn!` emitted before `Server::builder()` only when `jwt_secret` is `None`. | Met | Lines 306-310: `if config.jwt_secret.is_none() { tracing::warn!(...) }` at step 10. `Server::builder()` is at step 11, line 313. The warning is conditional on `is_none()` and placed before the builder. |
| 4 | Config doc comment table updated with `EVENTFOLD_JWT_SECRET` row. | Met | Line 39: `/// \| \`EVENTFOLD_JWT_SECRET\` \| No \| -- \| HS256 JWT signing secret; auth disabled when unset \|`. Matches the AC specification exactly. |
| 5 | Tonic 0.13 interceptor wiring verified. | Met | `InterceptedService::new(EventStoreServer::new(service), interceptor)` at line 364. Verified `InterceptedService` exists at `tonic::service::interceptor::InterceptedService` in tonic 0.13.1 source. `InterceptedService` wraps both unary and streaming RPCs. It also implements `NamedService` when the inner service does, so `add_service` and health reporter type matching both work correctly. |
| 6 | Test `from_env_jwt_secret_set`: sets `EVENTFOLD_JWT_SECRET=mysecret`, asserts `Some("mysecret")`. | Met | Lines 852-864: `#[serial]` test, sets var to "mysecret", asserts `config.jwt_secret == Some("mysecret".to_string())`. |
| 7 | Test `from_env_jwt_secret_unset`: unsets var, asserts `None`. | Met | Lines 868-880: `#[serial]` test, calls `clear_jwt_env()`, asserts `config.jwt_secret == None`. |
| 8 | Test `from_env_jwt_secret_empty_string`: sets empty string, asserts `None`. | Met | Lines 884-896: `#[serial]` test, sets `EVENTFOLD_JWT_SECRET=""`, asserts `config.jwt_secret == None`. |
| 9 | Existing Config tests still pass with `clear_jwt_env()` added. | Met | `clear_jwt_env()` helper defined at lines 451-454. All 16 pre-existing `#[serial]` tests call `clear_jwt_env()` to prevent env var leakage. All 302 tests pass (219 lib + 23 bin + 60 integration). |
| 10 | Quality gates pass. | Met | `cargo build`: zero warnings. `cargo clippy --all-targets --all-features --locked -- -D warnings`: zero warnings. `cargo test`: all 302+ tests pass. `cargo fmt --check`: only pre-existing `tests/metrics.rs` trailing blank line issue (outside ticket scope, documented in Ticket 1 review). |

## Issues Found

### Critical (must fix before merge)

None.

### Major (should fix, risk of downstream problems)

None.

### Minor (nice to fix, not blocking)

1. **New JWT tests don't call `clear_jwt_env()` at start** (`src/main.rs:852,884`): `from_env_jwt_secret_set` and `from_env_jwt_secret_empty_string` explicitly call `set_var("EVENTFOLD_JWT_SECRET", ...)` instead of first clearing and then setting. This works because `set_var` overwrites any existing value, and both tests set the variable to a deterministic value. However, for consistency with the pattern used by `from_env_jwt_secret_unset` (which clears other vars then sets what it needs), it would be marginally cleaner to call `clear_jwt_env()` and then `set_var`. Not a correctness issue -- purely a pattern consistency note.

## Suggestions (non-blocking)

- The health service is correctly added outside the JWT match (line 360), meaning health checks are not gated by authentication. This is good practice for load balancer health probes.

- The step comment renumbering (10-17) is clean and makes the code easy to follow.

- The `ref` pattern in `match config.jwt_secret { Some(ref secret) => ... }` avoids moving `config.jwt_secret` out of the struct, which is correct since `config` is used later (implicitly, by the compiler retaining it). Using `ref` is slightly more explicit than `match &config.jwt_secret` -- both are fine; consistency with existing code matters more.

## Scope Check

- Files within scope: YES -- only `src/main.rs` was modified (as specified by the ticket).
- Scope creep detected: NO
- Unauthorized dependencies added: NO

## Risk Assessment

- Regression risk: LOW -- Changes to `src/main.rs` are additive: a new field on `Config`, a new parsing step in `from_env`, and a conditional service-wrapping branch in `main`. All 16 existing `#[serial]` config tests have `clear_jwt_env()` added to prevent env var leakage. All existing tests continue to pass.
- Security concerns: NONE -- The JWT secret is read from an environment variable (not hardcoded). When unset, a `tracing::warn!` alerts operators. The actual crypto validation is delegated to `JwtInterceptor` from Ticket 1.
- Performance concerns: NONE -- The interceptor wiring is a one-time setup cost at server startup. No per-request overhead is added beyond what `JwtInterceptor::call` already does (reviewed in Ticket 1).
