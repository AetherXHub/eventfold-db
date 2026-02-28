# Code Review: Ticket 3 -- Integration tests in `tests/auth_integration.rs`

**Ticket:** 3 -- Integration tests in `tests/auth_integration.rs`
**Impl Report:** prd/019-jwt-authentication-interceptor-reports/ticket-03-impl.md
**Date:** 2026-02-28 18:15
**Verdict:** APPROVED

---

## AC Coverage

| AC # | Description | Status | Notes |
|------|-------------|--------|-------|
| 1 | `start_authed_test_server(secret: &str)` helper with `InterceptedService`, ephemeral port, `tempfile::tempdir()` | Met | Lines 80-110: Uses `JwtInterceptor::new(secret)` + `InterceptedService::new(EventStoreServer::new(service), interceptor)`, binds `[::1]:0`, returns `(SocketAddr, TempDir)`. Pattern matches `grpc_service.rs` / `tls_integration.rs` exactly. |
| 2 | `mint_token(secret: &str, exp_offset_secs: i64) -> String` helper | Met | Lines 43-73: Mints HS256 JWT with `sub = "test"` and `exp = now + exp_offset_secs`. Uses `saturating_sub` for negative offsets. Correct. |
| 3 | Test `auth_valid_token_append_succeeds` | Met | Lines 148-170: Starts authed server with `"testsecret"`, mints token (exp +3600), attaches `authorization: Bearer <token>`, asserts `Ok(_)`. |
| 4 | Test `auth_missing_token_append_rejected` | Met | Lines 175-199: No auth header, asserts `Err` with `Code::Unauthenticated`. |
| 5 | Test `auth_expired_token_append_rejected` | Met | Lines 203-233: Mints token with `exp_offset_secs = -3600`, asserts `Err` with `Code::Unauthenticated`. |
| 6 | Test `auth_disabled_no_secret_append_succeeds` | Met | Lines 238-260: Uses `start_plain_test_server()` (no interceptor), no auth header, asserts `Ok(_)`. |
| 7 | Test `auth_streaming_subscribe_all_rejected_without_token` | Met | Lines 265-285: No auth header on `subscribe_all`, asserts `Err(status)` with `Code::Unauthenticated`. The interceptor rejects the RPC call itself (before the stream opens), so `client.subscribe_all(request).await` returns `Err` directly -- this is the correct behavior and matches the AC's intent. |
| 8 | Test `auth_streaming_subscribe_stream_rejected_without_token` | Met | Lines 290-314: Same pattern for `subscribe_stream`. |
| 9 | Test `auth_valid_token_subscribe_all_stays_open` | Met | Lines 319-395: Opens `subscribe_all` with valid token, receives CaughtUp, appends event via separate authed client, receives live event with correct `event_type` and `global_position`. Full lifecycle verified. |
| 10 | Quality gates pass | Met | Verified independently: `cargo fmt --check` (clean), `cargo clippy --all-targets --all-features --locked -- -D warnings` (clean), `cargo test --test auth_integration` (7/7 pass), full `cargo test` (all pass, no regressions). |

## Issues Found

### Critical (must fix before merge)

None.

### Major (should fix, risk of downstream problems)

None.

### Minor (nice to fix, not blocking)

1. **Streaming rejection AC wording mismatch (cosmetic).** AC 7 says "attempt to receive the first message from the returned stream -- asserts the stream immediately yields `Err(status)`". The implementation instead asserts on the RPC call result (`client.subscribe_all(request).await`), which fails before a stream is returned. This is strictly better -- the `InterceptedService` rejects at the request level, so there is no stream to read from. The test is correct; the AC wording was slightly off about the failure point. No code change needed.

2. **Helper duplication.** `make_proposed`, `no_stream`, and `test_dedup_cap` are copy-pasted across `grpc_service.rs`, `tls_integration.rs`, and now `auth_integration.rs`. The impl report correctly notes that each integration test file is an independent crate, making this unavoidable without a shared test utility crate. Acceptable as-is, consistent with existing practice.

## Suggestions (non-blocking)

- The `auth_valid_token_subscribe_all_stays_open` test uses a 5-second timeout for receiving messages (`std::time::Duration::from_secs(5)`). This is generous and safe for CI, but could be tightened in the future if test speed becomes a concern. Not a problem now.

## Scope Check

- Files within scope: YES -- only `tests/auth_integration.rs` was created; no other files modified.
- Scope creep detected: NO
- Unauthorized dependencies added: NO

## Risk Assessment

- Regression risk: LOW -- All existing tests pass. This ticket only adds a new test file; no production code was touched.
- Security concerns: NONE -- Test-only code. Secrets used in tests (`"testsecret"`) are test-only values.
- Performance concerns: NONE -- Tests use ephemeral ports and tempdir isolation. 50ms sleep for server startup is consistent with existing test patterns.
