# Build Status: PRD 019 -- JWT Authentication Interceptor

**Source PRD:** prd/019-jwt-authentication-interceptor.md
**Tickets:** prd/019-jwt-authentication-interceptor-tickets.md
**Started:** 2026-02-28
**Last Updated:** 2026-02-28
**Overall Status:** QA READY

---

## Ticket Tracker

| Ticket | Title | Status | Impl Report | Review Report | Notes |
|--------|-------|--------|-------------|---------------|-------|
| 1 | Add dependencies and implement `src/auth.rs` with unit tests | DONE | ticket-01-impl.md | ticket-01-review.md | APPROVED |
| 2 | Wire `JwtInterceptor` into `src/main.rs` | DONE | ticket-02-impl.md | ticket-02-review.md | APPROVED |
| 3 | Integration tests in `tests/auth_integration.rs` | DONE | ticket-03-impl.md | ticket-03-review.md | APPROVED |
| 4 | Verification and integration check | DONE | -- | -- | All ACs verified |

## Prior Work Summary

- `src/auth.rs` created: `JwtInterceptor` struct with `tonic::service::Interceptor` impl, HS256 validation, `Claims` struct
- `Cargo.toml`: added `jsonwebtoken = "9"` and `serde = { version = "1", features = ["derive"] }`
- `src/lib.rs`: added `pub mod auth;` -- interceptor accessible at `eventfold_db::auth::JwtInterceptor`
- 5 unit tests pass: valid token, missing header, wrong secret, expired token, missing Bearer prefix
- `validation.leeway = 0` set explicitly (jsonwebtoken v9 defaults to 60s)
- Pre-existing `cargo fmt --check` issue in `tests/metrics.rs` (not introduced by this feature)
- `src/main.rs`: `Config.jwt_secret: Option<String>` parses `EVENTFOLD_JWT_SECRET` (empty/absent -> None)
- `main()` uses `InterceptedService::new(EventStoreServer::new(service), interceptor)` when secret is set
- `tracing::warn!` emitted before `Server::builder()` when auth disabled
- Health service is NOT auth-gated (added outside JWT match)
- 3 new config tests + `clear_jwt_env()` helper added to all 16 existing serial tests

## Follow-Up Tickets

[None yet.]

## Completion Report

**Completed:** 2026-02-28
**Tickets Completed:** 4/4

### Summary of Changes
- `src/auth.rs` (created): `JwtInterceptor` struct with `tonic::service::Interceptor` impl, HS256 validation via `jsonwebtoken` v9, private `Claims` struct, 5 unit tests
- `src/main.rs` (modified): `Config.jwt_secret: Option<String>`, `EVENTFOLD_JWT_SECRET` env var parsing, conditional `InterceptedService` wrapping, `tracing::warn!` when auth disabled, 3 new config tests, `clear_jwt_env()` helper
- `src/lib.rs` (modified): `pub mod auth;` declaration
- `Cargo.toml` (modified): added `jsonwebtoken = "9"` and `serde = { version = "1", features = ["derive"] }`
- `tests/auth_integration.rs` (created): 7 integration tests covering valid token, missing token, expired token, backward-compatibility, streaming RPC rejection, and live subscription with auth

### Known Issues / Follow-Up
- Pre-existing flaky test: `writer::tests::ac11_writer_metrics_appends_and_events_total` occasionally fails due to global metrics counter leakage between parallel tests (not introduced by this feature)
- Pre-existing `cargo fmt --check` issue in `tests/metrics.rs` (trailing blank line)
- Secret rotation (SIGHUP-triggered reload) deferred per PRD open questions
- Clock skew leeway (currently 0) documented as potential future config knob

### Ready for QA: YES
