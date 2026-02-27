# Code Review: Ticket 4 -- Integration tests for idempotent appends

**Ticket:** 4 -- Integration tests for idempotent appends (`tests/idempotent_appends.rs`)
**Impl Report:** prd/010-idempotent-appends-reports/ticket-04-impl.md
**Date:** 2026-02-27 19:15
**Verdict:** APPROVED

---

## AC Coverage

| AC # | Description | Status | Notes |
|------|-------------|--------|-------|
| 1 | Test helper `start_server(dedup_capacity)` starts an in-process gRPC server | Met | `start_server()` at line 57 accepts `data_path` and `dedup_capacity`, returns `(EventStoreClient, ServerHandle)`. `start_simple_server()` at line 108 provides a no-shutdown variant for subscription tests. Both follow the `tests/grpc_service.rs` pattern. |
| 2 | Test (AC-2): duplicate batch returns identical positions | Met | `dedup_duplicate_batch_returns_identical_positions` (line 166): appends 2-event batch, re-sends same batch, asserts all four position fields (`first_global_position`, `last_global_position`, `first_stream_version`, `last_stream_version`) are identical. |
| 3 | Test (AC-3): after duplicate append, ReadAll count matches first append only | Met | `dedup_no_duplicate_records_in_log` (line 227): appends 2-event batch, re-sends, then `ReadAll(from=0, max=1000)` asserts exactly 2 events. |
| 4 | Test (AC-4): two different batches succeed independently | Met | `dedup_different_batches_succeed_independently` (line 285): batch A (a1, a2) and batch B (b1, b2) to same stream, asserts positions are contiguous (0,1 then 2,3), and `ReadStream` returns 4 events total. |
| 5 | Test (AC-6): dedup survives restart | Met | `dedup_survives_restart` (line 357): appends single-event batch, shuts down server via `ServerHandle::shutdown()`, re-opens at same `data_path` (which triggers `seed_from_log`), re-sends same batch, asserts identical positions and ReadAll returns 1 event. Single-event batch is a deliberate workaround for `seed_from_log` losing batch grouping -- clearly documented in both test comments and impl report. |
| 6 | Test (AC-7): eviction allows re-append of evicted IDs | Met | `dedup_eviction_allows_reappend` (line 439): capacity=2, appends 3 single-event batches (id1 evicted), re-sends id1 (gets new higher position), re-sends id3 (dedup hit, same position). Assertions are specific and clear. |
| 7 | Test (AC-8): dedup hit does not publish to subscription | Met | `dedup_hit_does_not_publish_to_subscription` (line 530): subscribes before append, receives CaughtUp + live event, re-appends same batch, asserts 300ms timeout with no message. Uses `start_simple_server` to avoid shutdown-related streaming hangs. |
| 8 | All tests use tempfile for isolation | Met | Every test creates its own `tempfile::tempdir()` and derives `data_path` from it. No fixed paths. |
| 9 | Quality gates pass | Met | Verified: `cargo build` (zero warnings), `cargo clippy --all-targets --all-features --locked -- -D warnings` (clean), `cargo fmt --check` (clean), `cargo test` (226 passed, 0 failed). |

## Issues Found

### Critical (must fix before merge)

None.

### Major (should fix, risk of downstream problems)

None.

### Minor (nice to fix, not blocking)

None.

## Suggestions (non-blocking)

1. **Sleep duration inconsistency:** `start_server` and `start_simple_server` use `from_millis(100)` for the server startup sleep, while `tests/grpc_service.rs` and `tests/server_binary.rs` use `from_millis(50)`. Not a correctness issue (100ms is more conservative), but worth noting for consistency. The 100ms delay may have been chosen for reliability in the restart test scenario, which is a reasonable justification.

2. **`seed_from_log` batch-grouping limitation (documented):** The restart test uses single-event batches to work around `seed_from_log` not reconstructing original batch groupings. The implementer documented this clearly both in the test comments and the impl report's concerns section. If multi-event batch dedup fidelity after restart is needed in the future, `seed_from_log` would need to group consecutive same-stream events. This is acknowledged as a known gap, not a bug in this ticket.

## Scope Check

- Files within scope: YES -- Only `tests/idempotent_appends.rs` was created.
- Scope creep detected: NO
- Unauthorized dependencies added: NO

## Risk Assessment

- Regression risk: LOW -- This ticket only adds new tests; no production code was modified. All 226 existing tests continue to pass.
- Security concerns: NONE
- Performance concerns: NONE -- The 300ms timeout in the subscription test and 100ms server startup sleeps are acceptable for integration tests.
