# PRD 019: JWT Authentication Interceptor

**Status:** TICKETS READY
**Created:** 2026-02-28
**Author:** PRD Writer Agent

---

## Problem Statement

EventfoldDB currently exposes every gRPC endpoint with no authentication; any client that can reach the network port has full read and write access. GitHub issue #2 tracks the need for a token-based auth layer. An HS256 JWT interceptor applied at the tonic/tower layer gives operators an opt-in gate that is backward-compatible when the secret is absent, requires no schema changes, and is verifiable by any standard JWT library on the client side.

## Goals

- When `EVENTFOLD_JWT_SECRET` is set, every gRPC call (unary and streaming) that arrives without a valid `authorization: Bearer <jwt>` header is rejected with `UNAUTHENTICATED` before reaching service logic.
- When `EVENTFOLD_JWT_SECRET` is unset, the server starts and handles all requests exactly as it did before this feature (zero behavioral change for existing deployments).
- The `jsonwebtoken` crate is used for all token decoding and validation; no custom crypto is written.

## Non-Goals

- Role-based access control (RBAC), per-stream permissions, or any authorization beyond "valid token = full access."
- Token issuance, refresh, or a `/token` endpoint of any kind.
- Asymmetric signing algorithms (RS256, ES256, etc.); only HS256 is in scope.
- Per-RPC token rotation mid-stream; streaming RPCs validate once at stream open and do not re-validate on subsequent messages.
- TLS/mTLS (covered by PRD 011); this PRD assumes the transport layer is already hardened or is intentionally plaintext.
- Token revocation, blocklists, or expiry shorter than the `exp` claim enforces.
- Adding any new gRPC RPCs or changing the proto schema.
- Propagating the `sub` claim into event metadata or the in-memory index.

## User Stories

- As an operator running EventfoldDB in a shared environment, I want to set `EVENTFOLD_JWT_SECRET` so that unauthenticated clients cannot read or write any events.
- As a developer testing locally, I want to omit `EVENTFOLD_JWT_SECRET` so that existing integration tests and local tooling continue to work without tokens.
- As a client application, I want a deterministic `UNAUTHENTICATED` status code when my token is expired or malformed so that I can surface a clear error to the end user.
- As a client application initiating a streaming subscription, I want the token validated once at stream open so that long-lived subscriptions are not interrupted mid-flight by re-validation.

## Technical Approach

### Overview

A new module `src/auth.rs` implements the interceptor logic. The interceptor is conditionally wired into the tonic `Server` in `src/main.rs` only when `EVENTFOLD_JWT_SECRET` is present in the environment. No changes are made to `src/service.rs`, `src/store.rs`, or any other existing module.

`jsonwebtoken` (version `9`) is added as a regular dependency in `Cargo.toml`. No other new dependencies are required.

### File changes

| File | Action | Notes |
|------|--------|-------|
| `Cargo.toml` | Edit | Add `jsonwebtoken = "9"` to `[dependencies]` |
| `src/auth.rs` | Create | `JwtInterceptor` struct + `tonic::service::Interceptor` impl |
| `src/lib.rs` | Edit | `pub mod auth;` declaration |
| `src/main.rs` | Edit | Read `EVENTFOLD_JWT_SECRET`; conditionally wrap server with interceptor |

### `src/auth.rs` design

```rust
/// Holds the decoded signing key and validation config for HS256 JWT verification.
pub struct JwtInterceptor {
    decoding_key: jsonwebtoken::DecodingKey,
    validation: jsonwebtoken::Validation,
}
```

`JwtInterceptor::new(secret: &str) -> Self` constructs the struct:
- `DecodingKey::from_secret(secret.as_bytes())`
- `Validation::new(Algorithm::HS256)` with `validate_exp = true`; no additional required claims beyond `exp` and `sub` (the `sub` field is declared in the claims struct but its value is not acted upon).

The `Claims` struct used with `jsonwebtoken::decode`:

```rust
#[derive(serde::Deserialize)]
struct Claims {
    sub: String,
    exp: u64,
}
```

`tonic::service::Interceptor` is implemented for `JwtInterceptor`. The `call` method:

1. Reads the `authorization` metadata key from `request.metadata()`.
2. If missing or not parseable as ASCII, returns `Err(Status::unauthenticated("missing authorization header"))`.
3. Strips the `"Bearer "` prefix (case-sensitive). If the prefix is absent, returns `Err(Status::unauthenticated("invalid authorization header format"))`.
4. Calls `jsonwebtoken::decode::<Claims>(&token, &self.decoding_key, &self.validation)`.
5. On `Ok(_)`, returns `Ok(request)`.
6. On `Err(e)`, maps the `jsonwebtoken::errors::ErrorKind` to an appropriate message and returns `Err(Status::unauthenticated(e.to_string()))`.

The interceptor is `Clone` (derived) so tonic can clone it per-connection as required by the `Interceptor` trait.

### `src/main.rs` changes

After reading config and before `Server::builder()`, read `std::env::var("EVENTFOLD_JWT_SECRET")`:

- If `Ok(secret)` and non-empty: construct `JwtInterceptor::new(&secret)` and wrap the service with `.add_service(service.into_inner().intercept_with(interceptor))` (or the `InterceptedService` wrapper via `ServiceBuilder`).
- If `Err(_)` or empty: add the service without wrapping, emitting a `tracing::warn!` that auth is disabled.

The exact tonic API to use is `tonic::service::interceptor(interceptor_fn)` combined with `ServiceBuilder::layer` from the `tower` crate, which is already a transitive dependency. Specifically:

```rust
use tonic::service::interceptor;
// ...
let svc = interceptor(jwt_interceptor).layer(event_store_service);
server.add_service(svc)
```

This approach works identically for both unary and server-streaming RPCs because the interceptor runs once per request initiation at the tonic layer, before the handler is invoked.

### Error mapping

All JWT validation failures produce `tonic::Status::unauthenticated(message)`. The `message` field includes the `jsonwebtoken` error string. No internal error details (e.g., stack traces, key material) are included. The server logs the rejection at `tracing::debug!` level (not `warn!` or `error!`, to avoid log spam from brute-force probes).

### Testing

Unit tests in `src/auth.rs` (`#[cfg(test)]` module):

- A test token is minted inline using `jsonwebtoken::encode` with a known secret and a valid `exp` (current time + 3600 s using `std::time::SystemTime`).
- Test that a valid token returns `Ok(request)`.
- Test that a missing `authorization` header returns `Err` with `UNAUTHENTICATED` code.
- Test that a token signed with the wrong secret returns `Err` with `UNAUTHENTICATED` code.
- Test that a token with `exp` in the past returns `Err` with `UNAUTHENTICATED` code.
- Test that a header value without the `Bearer ` prefix returns `Err` with `UNAUTHENTICATED` code.

Integration tests in `tests/` add a new test file `tests/auth_integration.rs`:

- Spin up a real tonic server with `JwtInterceptor` configured (using `tempfile::tempdir()` for data isolation, consistent with existing integration tests).
- Assert that `Append` with a valid token succeeds.
- Assert that `Append` with no token returns gRPC status `UNAUTHENTICATED`.
- Assert that `Append` with an expired token returns gRPC status `UNAUTHENTICATED`.
- Assert that when `EVENTFOLD_JWT_SECRET` is not set, `Append` without a token succeeds (backward-compatibility test).

## Acceptance Criteria

1. When `EVENTFOLD_JWT_SECRET=mysecret cargo run` is started and a gRPC `Append` request is made without an `authorization` header, the server returns gRPC status code `UNAUTHENTICATED` and the event is not appended.
2. When `EVENTFOLD_JWT_SECRET=mysecret cargo run` is started and a gRPC `Append` request is made with `authorization: Bearer <valid_hs256_token>` where the token has a future `exp`, the server returns `OK` and the event is appended.
3. When `EVENTFOLD_JWT_SECRET=mysecret cargo run` is started and a gRPC `Append` request is made with an expired JWT (past `exp`), the server returns `UNAUTHENTICATED`.
4. When `EVENTFOLD_JWT_SECRET=mysecret cargo run` is started and a gRPC `Append` request is made with a JWT signed using a different secret, the server returns `UNAUTHENTICATED`.
5. When `EVENTFOLD_JWT_SECRET` is not set (or is an empty string) and `cargo run` is started, a `WARN`-level log line containing "auth is disabled" (case-insensitive) is emitted at startup, and gRPC requests without any `authorization` header succeed as before.
6. `SubscribeAll` and `SubscribeStream` streaming RPCs both return `UNAUTHENTICATED` on the initial call when the token is missing or invalid; no events are streamed before the rejection.
7. A valid token presented at the start of a `SubscribeAll` stream allows the stream to remain open and receive events indefinitely without any re-validation mid-stream.
8. `src/auth.rs` exists and exports `JwtInterceptor`; the `jsonwebtoken` crate appears in `Cargo.toml` under `[dependencies]` with a `9.x` version constraint.
9. All unit tests in `src/auth.rs` pass: valid token, missing header, wrong secret, expired token, and missing `Bearer ` prefix each produce the expected outcome.
10. `cargo build --locked`, `cargo test --locked`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, and `cargo fmt --check` all exit with code 0 after all changes are applied.

## Open Questions

- **Secret rotation**: The current design reads `EVENTFOLD_JWT_SECRET` once at startup. If the secret needs to be rotated without restarting the server, a SIGHUP-triggered reload would be required. This is out of scope for this PRD but should be noted in a follow-up issue.
- **Clock skew tolerance**: `jsonwebtoken`'s `Validation` supports a `leeway` field (seconds of clock skew forgiveness on `exp`/`nbf`). The default is 0. Whether to set a non-zero leeway (e.g., 30 s) is left as an open question; the implementation should default to 0 and document it as a potential future config knob.

## Dependencies

- PRD 007 (server binary and `main.rs` entrypoint) — must be complete; the interceptor is wired in `main.rs`.
- `jsonwebtoken` crate v9 (new runtime dependency).
- `serde` with `derive` feature — already a dependency via the existing codebase.
- `tonic::service::Interceptor` trait and `InterceptedService` type — already available via the existing `tonic` dependency.
