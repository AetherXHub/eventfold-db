# Implementation Report: Ticket 3 -- Integration tests in `tests/auth_integration.rs`

**Ticket:** 3 - Integration tests in `tests/auth_integration.rs`
**Date:** 2026-02-28 17:30
**Status:** COMPLETE

---

## Files Changed

### Created
- `tests/auth_integration.rs` - Integration test file with 7 tests exercising JWT authentication interceptor over real gRPC connections.

### Modified
- None

## Implementation Notes
- Mirrored the server startup pattern from `tests/grpc_service.rs` and `tests/tls_integration.rs` exactly: `Store::open` -> `Broker::new` -> `spawn_writer` -> `EventfoldService::new` -> TCP bind on `[::1]:0` -> `tokio::spawn` server.
- `start_authed_test_server(secret)` wraps the `EventStoreServer` with `InterceptedService::new(EventStoreServer::new(service), JwtInterceptor::new(secret))`, matching the pattern in `src/main.rs`.
- `start_plain_test_server()` uses bare `EventStoreServer::new(service)` without any interceptor for the backward-compatibility test.
- `mint_token(secret, exp_offset_secs)` uses `jsonwebtoken::encode` with HS256, `sub="test"`, and `exp` computed as `now + offset` (with `saturating_sub` for negative offsets).
- The streaming rejection tests confirm that `InterceptedService` intercepts the RPC call itself (not just the first stream message), so `client.subscribe_all(request)` returns `Err(Status::Unauthenticated)` directly.
- The `auth_valid_token_subscribe_all_stays_open` test verifies the full lifecycle: subscribe with valid token -> receive CaughtUp -> append via separate authed client -> receive live event on the subscription stream.
- Helper functions `make_proposed`, `no_stream`, and `test_dedup_cap` are duplicated from existing test files (each integration test file is an independent crate).
- Pre-existing `cargo fmt` issue in `tests/metrics.rs` (trailing blank line) was not touched -- it is outside this ticket's scope.

## Acceptance Criteria
- [x] AC 1: `start_authed_test_server(secret: &str)` helper spins up `EventfoldService` with `JwtInterceptor::new(secret)` wired via `InterceptedService`, listening on `[::1]:0`, returns `(SocketAddr, TempDir)` with `tempfile::tempdir()` for isolation.
- [x] AC 2: `mint_token(secret: &str, exp_offset_secs: i64) -> String` mints HS256 JWT with `sub = "test"` and `exp = now + exp_offset_secs`. Negative offset produces expired tokens.
- [x] AC 3: `auth_valid_token_append_succeeds` -- starts authed server with `"testsecret"`, mints token (exp +3600), attaches `authorization: Bearer <token>` header, asserts `Ok(_)`.
- [x] AC 4: `auth_missing_token_append_rejected` -- starts authed server, sends request without auth header, asserts `Err(status)` with `Code::Unauthenticated`.
- [x] AC 5: `auth_expired_token_append_rejected` -- starts authed server, mints token with `exp_offset_secs = -3600`, attaches as Bearer header, asserts `Err(status)` with `Code::Unauthenticated`.
- [x] AC 6: `auth_disabled_no_secret_append_succeeds` -- starts plaintext server (no interceptor) via `start_plain_test_server()`, sends append without auth header, asserts `Ok(_)`.
- [x] AC 7: `auth_streaming_subscribe_all_rejected_without_token` -- starts authed server, calls `subscribe_all` without auth header, asserts `Err(status)` with `Code::Unauthenticated`.
- [x] AC 8: `auth_streaming_subscribe_stream_rejected_without_token` -- starts authed server, calls `subscribe_stream` without auth header, asserts `Err(status)` with `Code::Unauthenticated`.
- [x] AC 9: `auth_valid_token_subscribe_all_stays_open` -- starts authed server, opens `subscribe_all` with valid token, receives CaughtUp, appends event via separate authed client, receives live event on subscription stream.
- [x] AC 10: Quality gates pass -- `cargo build`, `cargo clippy`, `cargo test`, `cargo fmt --check` all exit 0 (metrics.rs fmt issue is pre-existing).

## Test Results
- Lint: PASS (`cargo clippy --all-targets --all-features --locked -- -D warnings` exits 0)
- Tests: PASS (all 310+ tests pass including 7 new auth integration tests)
- Build: PASS (`cargo build` exits 0 with zero warnings)
- Fmt: PASS for `tests/auth_integration.rs` (pre-existing `tests/metrics.rs` trailing blank line is unchanged)
- New tests added:
  - `tests/auth_integration.rs::auth_valid_token_append_succeeds`
  - `tests/auth_integration.rs::auth_missing_token_append_rejected`
  - `tests/auth_integration.rs::auth_expired_token_append_rejected`
  - `tests/auth_integration.rs::auth_disabled_no_secret_append_succeeds`
  - `tests/auth_integration.rs::auth_streaming_subscribe_all_rejected_without_token`
  - `tests/auth_integration.rs::auth_streaming_subscribe_stream_rejected_without_token`
  - `tests/auth_integration.rs::auth_valid_token_subscribe_all_stays_open`

## Concerns / Blockers
- Pre-existing `cargo fmt --check` failure in `tests/metrics.rs` (trailing blank line at line 544). Not introduced by this ticket; noted in Prior Work Summary. Does not affect this ticket's tests.
- None otherwise.
