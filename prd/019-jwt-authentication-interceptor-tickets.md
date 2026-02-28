# Tickets for PRD 019: JWT Authentication Interceptor

**Source PRD:** prd/019-jwt-authentication-interceptor.md
**Created:** 2026-02-28
**Total Tickets:** 4
**Estimated Total Complexity:** 8 (S=1, M=2, L=3 → 2+3+2+1=8)

---

### Ticket 1: Add dependencies and implement `src/auth.rs` with unit tests

**Description:**
Add `jsonwebtoken = "9"` and `serde` (with `derive` feature) to `Cargo.toml`. Create
`src/auth.rs` containing the `JwtInterceptor` struct and its `tonic::service::Interceptor`
implementation, plus the private `Claims` struct used by `jsonwebtoken::decode`. Declare
`pub mod auth;` in `src/lib.rs`. Write all five unit tests in `src/auth.rs`'s `#[cfg(test)]`
module using red/green TDD.

**Scope:**
- Create: `src/auth.rs`
- Modify: `Cargo.toml` (add `jsonwebtoken = "9"` and `serde = { version = "1", features = ["derive"] }` to `[dependencies]`)
- Modify: `src/lib.rs` (add `pub mod auth;`)

**Acceptance Criteria:**
- [ ] `Cargo.toml` contains `jsonwebtoken = "9"` under `[dependencies]`.
- [ ] `Cargo.toml` contains `serde = { version = "1", features = ["derive"] }` under `[dependencies]` (needed for `#[derive(serde::Deserialize)]` on `Claims`).
- [ ] `src/auth.rs` declares `pub struct JwtInterceptor` with private fields `decoding_key: jsonwebtoken::DecodingKey` and `validation: jsonwebtoken::Validation`. The struct derives `Clone`.
- [ ] `JwtInterceptor::new(secret: &str) -> Self` constructs `DecodingKey::from_secret(secret.as_bytes())` and `Validation::new(Algorithm::HS256)` with `validate_exp = true`. Both `exp` and `sub` are required claims (set via `validation.required_spec_claims`).
- [ ] Private `Claims` struct has fields `sub: String` and `exp: u64` and derives `serde::Deserialize`.
- [ ] `tonic::service::Interceptor` is implemented for `JwtInterceptor`. The `call` method: (1) reads the `authorization` metadata key; (2) returns `Err(Status::unauthenticated("missing authorization header"))` if absent or non-ASCII; (3) strips the case-sensitive `"Bearer "` prefix, returning `Err(Status::unauthenticated("invalid authorization header format"))` if absent; (4) calls `jsonwebtoken::decode::<Claims>`; (5) returns `Ok(request)` on success or `Err(Status::unauthenticated(e.to_string()))` on failure; (6) logs rejections at `tracing::debug!` level.
- [ ] All public items in `src/auth.rs` have doc comments.
- [ ] `src/lib.rs` includes `pub mod auth;` so `JwtInterceptor` is accessible at `eventfold_db::auth::JwtInterceptor`.
- [ ] Test: construct `JwtInterceptor::new("testsecret")`, mint a token with `jsonwebtoken::encode` using `Algorithm::HS256` and a valid `exp` (current Unix time + 3600), build a `tonic::Request::new(())` with `authorization: Bearer <token>` metadata, call `interceptor.call(request)` -- asserts `Ok(_)`.
- [ ] Test: construct `JwtInterceptor::new("testsecret")`, build a `tonic::Request::new(())` with no `authorization` metadata key, call `interceptor.call(request)` -- asserts `Err(status)` where `status.code() == tonic::Code::Unauthenticated` and `status.message().contains("missing")`.
- [ ] Test: construct `JwtInterceptor::new("rightsecret")`, mint a token signed with `"wrongsecret"`, call `interceptor.call(request_with_bearer_token)` -- asserts `Err(status)` where `status.code() == tonic::Code::Unauthenticated`.
- [ ] Test: construct `JwtInterceptor::new("testsecret")`, mint a token with `exp` set to `1` (Unix epoch far in the past), call `interceptor.call(request_with_bearer_token)` -- asserts `Err(status)` where `status.code() == tonic::Code::Unauthenticated`.
- [ ] Test: construct `JwtInterceptor::new("testsecret")`, build a `tonic::Request::new(())` with `authorization: NotBearer <token>` (no "Bearer " prefix), call `interceptor.call(request)` -- asserts `Err(status)` where `status.code() == tonic::Code::Unauthenticated` and `status.message().contains("format")`.
- [ ] Quality gates pass: `cargo build`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo test`, `cargo fmt --check` all exit 0.

**Dependencies:** None
**Complexity:** M
**Maps to PRD AC:** AC 8, AC 9

---

### Ticket 2: Wire `JwtInterceptor` into `src/main.rs`

**Description:**
Extend `src/main.rs` to read the `EVENTFOLD_JWT_SECRET` environment variable after reading
existing config. When the variable is set and non-empty, wrap `EventStoreServer::new(service)`
with the interceptor using `tonic::service::interceptor`. When absent or empty, add the service
unwrapped and emit a `tracing::warn!` that auth is disabled. Extend `Config` with a
`jwt_secret: Option<String>` field and parse it in `Config::from_env`. Add unit tests for
the new `Config` parsing behavior using the existing `#[serial]` pattern.

**Scope:**
- Modify: `src/main.rs` (add `jwt_secret` field to `Config`, parse `EVENTFOLD_JWT_SECRET` in `from_env`, conditionally wrap service in `main`)

**Acceptance Criteria:**
- [ ] `Config` gains a new field `jwt_secret: Option<String>`. `Config::from_env` reads `EVENTFOLD_JWT_SECRET`: `Ok(s)` with non-empty `s` sets `Some(s)`; `Err(_)` or empty string sets `None`.
- [ ] In `main`, after building `service`, a `match config.jwt_secret` branch either: (a) constructs `JwtInterceptor::new(&secret)` and wraps the service as `EventStoreServer::from_interceptor(service, interceptor)` (or equivalent `tonic::service::interceptor` composition); or (b) adds the service unwrapped and calls `tracing::warn!` with a message containing "auth is disabled" (case-insensitive match, so the actual casing is the implementer's choice, but the verification grep uses `(?i)auth is disabled`).
- [ ] The `tracing::warn!` is emitted at startup (before `Server::builder()`) only when `jwt_secret` is `None`.
- [ ] The `Config` doc comment table is updated to include `EVENTFOLD_JWT_SECRET | No | -- | HS256 JWT signing secret; auth disabled when unset`.
- [ ] Implementer note: The tonic 0.13 API for interceptor wiring uses `tonic::service::interceptor(fn)` from `tower`. Check the tonic 0.13 docs/source for the exact wiring with `EventStoreServer` before implementing -- the PRD's suggested snippet may need adjustment. The key invariant is that both unary and streaming RPCs are intercepted.
- [ ] Test (`#[serial]`): set `EVENTFOLD_JWT_SECRET=mysecret` and all required vars, call `Config::from_env()` -- asserts `config.jwt_secret == Some("mysecret".to_string())`.
- [ ] Test (`#[serial]`): unset `EVENTFOLD_JWT_SECRET`, call `Config::from_env()` -- asserts `config.jwt_secret == None`.
- [ ] Test (`#[serial]`): set `EVENTFOLD_JWT_SECRET=""` (empty string), call `Config::from_env()` -- asserts `config.jwt_secret == None`.
- [ ] Existing `Config` unit tests (`from_env_defaults_when_only_data_set` etc.) still pass without modification (they clear only old env vars; add `EVENTFOLD_JWT_SECRET` removal to each helper or `clear_jwt_env()` helper).
- [ ] Quality gates pass: `cargo build`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo test`, `cargo fmt --check` all exit 0.

**Dependencies:** Ticket 1 (requires `eventfold_db::auth::JwtInterceptor`)
**Complexity:** M
**Maps to PRD AC:** AC 1, AC 2, AC 3, AC 4, AC 5

---

### Ticket 3: Integration tests in `tests/auth_integration.rs`

**Description:**
Create `tests/auth_integration.rs` with a helper `start_authed_test_server` that spins up a
real in-process tonic server with `JwtInterceptor` wired, mirroring the pattern in
`tests/tls_integration.rs`. Write four integration tests covering: valid-token `Append`
succeeds; no-token `Append` returns `UNAUTHENTICATED`; expired-token `Append` returns
`UNAUTHENTICATED`; and unauthenticated `Append` on a no-secret server succeeds
(backward-compatibility). Include two streaming RPC tests: `SubscribeAll` and `SubscribeStream`
with missing/invalid tokens return `UNAUTHENTICATED` on stream open.

**Scope:**
- Create: `tests/auth_integration.rs`

**Acceptance Criteria:**
- [ ] `start_authed_test_server(secret: &str)` helper spins up `EventfoldService` with `JwtInterceptor::new(secret)` wired to `EventStoreServer`, listening on an ephemeral `[::1]:0` port, and returns `(SocketAddr, TempDir)`. Uses `tempfile::tempdir()` for data isolation.
- [ ] `mint_token(secret: &str, exp_offset_secs: i64) -> String` helper mints a HS256 JWT with `sub = "test"` and `exp = now + exp_offset_secs`. When `exp_offset_secs` is negative, the token is already expired.
- [ ] Test `auth_valid_token_append_succeeds`: start authed server with `"testsecret"`, mint a fresh token (exp +3600), attach `authorization: Bearer <token>` header to an `AppendRequest` via `request.metadata_mut().insert(...)`, call `client.append(request)` -- asserts `Ok(_)`.
- [ ] Test `auth_missing_token_append_rejected`: start authed server, call `client.append(request_without_auth_header)` -- asserts `Err(status)` where `status.code() == tonic::Code::Unauthenticated`.
- [ ] Test `auth_expired_token_append_rejected`: start authed server with `"testsecret"`, mint token with `exp_offset_secs = -3600` (expired), attach as Bearer header, call `client.append` -- asserts `Err(status)` where `status.code() == tonic::Code::Unauthenticated`.
- [ ] Test `auth_disabled_no_secret_append_succeeds`: start a plaintext server (no interceptor) by directly using `EventStoreServer::new(service)` without wrapping, call `client.append` without any auth header -- asserts `Ok(_)`. (This directly tests the backward-compatibility path that `main.rs` wires when `EVENTFOLD_JWT_SECRET` is absent.)
- [ ] Test `auth_streaming_subscribe_all_rejected_without_token`: start authed server, call `client.subscribe_all(SubscribeAllRequest { from_position: 0 })` without auth header, attempt to receive the first message from the returned stream -- asserts the stream immediately yields `Err(status)` where `status.code() == tonic::Code::Unauthenticated`.
- [ ] Test `auth_streaming_subscribe_stream_rejected_without_token`: start authed server, call `client.subscribe_stream(SubscribeStreamRequest { stream_id: ..., from_version: 0 })` without auth header, attempt to receive from the stream -- asserts `Err(status)` where `status.code() == tonic::Code::Unauthenticated`.
- [ ] Test `auth_valid_token_subscribe_all_stays_open`: start authed server, mint a valid token, open a `subscribe_all` stream with the Bearer header, append one event via a separate client with a valid token, receive from the subscription stream -- asserts `Ok(SubscriptionMessage)` is received (stream stays open after initial validation).
- [ ] Quality gates pass: `cargo build`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo test`, `cargo fmt --check` all exit 0.

**Dependencies:** Ticket 1 (requires `JwtInterceptor`), Ticket 2 (validates `main.rs` backward-compat path exists but test uses direct in-process wiring)
**Complexity:** L
**Maps to PRD AC:** AC 1, AC 2, AC 3, AC 4, AC 5, AC 6, AC 7

---

### Ticket 4: Verification and integration check

**Description:**
Run the full PRD acceptance criteria checklist end-to-end. Verify all tickets integrate
correctly as a cohesive feature. Confirm that all quality gates pass on the full codebase.

**Acceptance Criteria:**
- [ ] AC 1: `grep -r "EVENTFOLD_JWT_SECRET" src/main.rs` returns a match; running the binary with `EVENTFOLD_JWT_SECRET=mysecret` and making an `Append` without a token returns gRPC status `UNAUTHENTICATED` (verified by integration tests).
- [ ] AC 2: Integration test `auth_valid_token_append_succeeds` passes -- `Append` with a valid future-exp token returns `OK`.
- [ ] AC 3: Integration test `auth_expired_token_append_rejected` passes -- `Append` with an expired token returns `UNAUTHENTICATED`.
- [ ] AC 4: Integration test verifies wrong-secret token returns `UNAUTHENTICATED` (add a dedicated test `auth_wrong_secret_append_rejected` in `tests/auth_integration.rs` if not present).
- [ ] AC 5: `grep -i "auth is disabled" src/main.rs` returns a match; `auth_disabled_no_secret_append_succeeds` integration test passes.
- [ ] AC 6: Integration tests `auth_streaming_subscribe_all_rejected_without_token` and `auth_streaming_subscribe_stream_rejected_without_token` pass.
- [ ] AC 7: Integration test `auth_valid_token_subscribe_all_stays_open` passes.
- [ ] AC 8: `grep "pub mod auth" src/lib.rs` returns a match; `grep 'jsonwebtoken = "9"' Cargo.toml` returns a match; `src/auth.rs` exists.
- [ ] AC 9: All five `src/auth.rs` unit tests pass (valid token, missing header, wrong secret, expired token, missing Bearer prefix).
- [ ] AC 10: `cargo build --locked` exits 0; `cargo test --locked` exits 0; `cargo clippy --all-targets --all-features --locked -- -D warnings` exits 0; `cargo fmt --check` exits 0.
- [ ] No regressions in existing tests (broker, gRPC service, TLS, health check, idempotent appends, writer, metrics).

**Dependencies:** Tickets 1, 2, 3
**Complexity:** S
**Maps to PRD AC:** AC 1, AC 2, AC 3, AC 4, AC 5, AC 6, AC 7, AC 8, AC 9, AC 10

---

## AC Coverage Matrix

| PRD AC # | Description                                                                                          | Covered By Ticket(s)      | Status  |
|----------|------------------------------------------------------------------------------------------------------|---------------------------|---------|
| 1        | No-token `Append` returns `UNAUTHENTICATED` when `EVENTFOLD_JWT_SECRET` is set                       | Ticket 2, 3, 4            | Covered |
| 2        | Valid HS256 token with future `exp` on `Append` returns `OK`                                         | Ticket 2, 3, 4            | Covered |
| 3        | Expired JWT on `Append` returns `UNAUTHENTICATED`                                                   | Ticket 2, 3, 4            | Covered |
| 4        | JWT signed with wrong secret on `Append` returns `UNAUTHENTICATED`                                  | Ticket 1 (unit), 3, 4     | Covered |
| 5        | No `EVENTFOLD_JWT_SECRET`: WARN log "auth is disabled"; requests without auth succeed                | Ticket 2, 3, 4            | Covered |
| 6        | `SubscribeAll` and `SubscribeStream` return `UNAUTHENTICATED` on initial call with missing/invalid token | Ticket 3, 4           | Covered |
| 7        | Valid token at stream open allows `SubscribeAll` to remain open indefinitely without re-validation   | Ticket 3, 4               | Covered |
| 8        | `src/auth.rs` exports `JwtInterceptor`; `jsonwebtoken` v9 in `Cargo.toml` `[dependencies]`          | Ticket 1, 4               | Covered |
| 9        | All unit tests pass: valid token, missing header, wrong secret, expired token, missing Bearer prefix | Ticket 1, 4               | Covered |
| 10       | `cargo build --locked`, `cargo test --locked`, `cargo clippy ... -D warnings`, `cargo fmt --check` exit 0 | Ticket 1, 2, 3, 4    | Covered |
