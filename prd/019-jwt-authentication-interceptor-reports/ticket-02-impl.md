# Implementation Report: Ticket 2 -- Wire `JwtInterceptor` into `src/main.rs`

**Ticket:** 2 - Wire `JwtInterceptor` into `src/main.rs`
**Date:** 2026-02-28 16:30
**Status:** COMPLETE

---

## Files Changed

### Created
- None

### Modified
- `src/main.rs` - Added `jwt_secret: Option<String>` to `Config`, parsing in `from_env`, conditional interceptor wiring in `main()`, `tracing::warn!` when auth disabled, 3 new unit tests, `clear_jwt_env()` helper, and updated all existing serial tests to clear `EVENTFOLD_JWT_SECRET`.

## Implementation Notes
- Used `tonic::service::interceptor::InterceptedService::new(service, interceptor)` for wiring -- this is the tonic 0.13 idiomatic approach. `InterceptedService` implements `NamedService` when the inner service does, so it works with `add_service()`.
- The `tracing::warn!` is emitted at step 10 (before `Server::builder()` at step 11), satisfying the "before server builder" AC.
- The match branches produce the same `Router<L>` type because `add_service` type-erases the service into routes internally.
- Added `clear_jwt_env()` to all 16 existing `#[serial]` tests to prevent `EVENTFOLD_JWT_SECRET` leaking between tests.
- Step comments in `main()` renumbered from 10 onward to accommodate the new JWT auth status logging step.

## Acceptance Criteria
- [x] AC 1: `Config` gains `jwt_secret: Option<String>`. `from_env` reads `EVENTFOLD_JWT_SECRET`: non-empty `Ok(s)` -> `Some(s)`, `Err(_)` or empty -> `None`.
- [x] AC 2: In `main`, `match config.jwt_secret` either wraps with `InterceptedService::new(EventStoreServer::new(service), interceptor)` or adds unwrapped with `tracing::warn!`.
- [x] AC 3: `tracing::warn!` emitted before `Server::builder()` only when `jwt_secret` is `None`. Message: "JWT auth is disabled -- all requests will be accepted without authentication".
- [x] AC 4: Doc comment table updated: `EVENTFOLD_JWT_SECRET | No | -- | HS256 JWT signing secret; auth disabled when unset`.
- [x] AC 5: Tonic 0.13 interceptor wiring verified -- `InterceptedService` wraps both unary and streaming RPCs.
- [x] AC 6: Test `from_env_jwt_secret_set`: sets `EVENTFOLD_JWT_SECRET=mysecret`, asserts `Some("mysecret".to_string())`.
- [x] AC 7: Test `from_env_jwt_secret_unset`: unsets var, asserts `None`.
- [x] AC 8: Test `from_env_jwt_secret_empty_string`: sets empty string, asserts `None`.
- [x] AC 9: All existing `Config` tests pass -- `clear_jwt_env()` added to each `#[serial]` test.
- [x] AC 10: Quality gates pass (see below).

## Test Results
- Build: PASS (`cargo build` -- zero warnings)
- Clippy: PASS (`cargo clippy --all-targets --all-features --locked -- -D warnings` -- zero warnings)
- Tests: PASS (`cargo test` -- 302 tests pass: 219 lib, 23 bin, 60 integration)
- Fmt: PASS for `src/main.rs`. Pre-existing issue in `tests/metrics.rs` (trailing blank line) causes `cargo fmt --check` exit code 1; this is NOT introduced by this ticket (documented in Ticket 1 prior work summary).
- New tests added:
  - `src/main.rs::tests::from_env_jwt_secret_set`
  - `src/main.rs::tests::from_env_jwt_secret_unset`
  - `src/main.rs::tests::from_env_jwt_secret_empty_string`

## Concerns / Blockers
- Pre-existing `cargo fmt --check` failure in `tests/metrics.rs` (trailing blank line at line 544). Not introduced by this ticket. Should be fixed in a separate cleanup task.
- None of the integration tests in `tests/` exercise the JWT interceptor wiring end-to-end (that is Ticket 3's scope per the ticket breakdown).
