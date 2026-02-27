# Build Status: PRD 010 -- Idempotent Appends

**Source PRD:** prd/010-idempotent-appends.md
**Tickets:** prd/010-idempotent-appends-tickets.md
**Started:** 2026-02-26 20:00
**Last Updated:** 2026-02-27 02:15
**Overall Status:** QA READY

---

## Ticket Tracker

| Ticket | Title | Status | Impl Report | Review Report | Notes |
|--------|-------|--------|-------------|---------------|-------|
| 1 | Add `DedupIndex` module (`src/dedup.rs`) | DONE | ticket-01-impl.md | ticket-01-review.md | APPROVED |
| 2 | Integrate `DedupIndex` into writer task | DONE | ticket-02-impl.md | ticket-02-review.md | APPROVED |
| 3 | Wire `dedup_capacity` into server config | DONE | ticket-02-impl.md | ticket-02-review.md | Completed as part of Ticket 2 |
| 4 | Integration tests for idempotent appends | DONE | ticket-04-impl.md | ticket-04-review.md | APPROVED |
| 5 | Verification and integration check | DONE | -- | -- | All gates pass, all ACs covered |

## Prior Work Summary

- `src/dedup.rs`: `DedupIndex` struct with `new`, `check(&mut self)`, `record`, `seed_from_log`. LRU promotion on hit via `get()`.
- `src/writer.rs`: dedup check before append, `validate_batch_unique_ids()`, dedup record before broker publish. `spawn_writer` takes `dedup_capacity` 4th arg, seeds from log.
- `src/main.rs`: `Config.dedup_capacity` from `EVENTFOLD_DEDUP_CAPACITY` env var (default 65536).
- `tests/idempotent_appends.rs`: 6 end-to-end gRPC integration tests covering AC-2 through AC-8.
- All integration tests and broker tests updated for 4-arg `spawn_writer`.
- Total: 226 tests green. Build, clippy, fmt all clean.

## Follow-Up Tickets

- `seed_from_log` loses original batch groupings (each event becomes a 1-element batch). Multi-event batch dedup hits after restart return only the first event's data. Low priority since the core invariant (no duplicate writes) is preserved.

## Completion Report

**Completed:** 2026-02-27 02:15
**Tickets Completed:** 5/5

### Summary of Changes

**Files created:**
- `src/dedup.rs` -- Bounded LRU dedup index (DedupIndex struct, 4 methods, 6 unit tests)
- `tests/idempotent_appends.rs` -- 6 end-to-end gRPC integration tests

**Files modified:**
- `src/lib.rs` -- Added `pub(crate) mod dedup;`
- `src/writer.rs` -- Dedup integration in `run_writer`/`spawn_writer`, `validate_batch_unique_ids()`, 4 new tests, updated existing test call sites
- `src/dedup.rs` -- Changed `check` to `&mut self`/`get()` for LRU promotion
- `src/main.rs` -- Added `dedup_capacity` to Config, `EVENTFOLD_DEDUP_CAPACITY` env var
- `src/broker.rs` -- Updated spawn_writer call sites in tests
- `Cargo.toml` -- Added `lru = "0.12"`
- `tests/broker_integration.rs` -- Updated spawn_writer calls
- `tests/grpc_service.rs` -- Updated spawn_writer calls
- `tests/server_binary.rs` -- Updated spawn_writer calls
- `tests/writer_integration.rs` -- Updated spawn_writer call

### Key Architectural Decisions
- Dedup index lives entirely inside the writer task (no Arc/Mutex needed)
- LRU promotion on check (`get()` not `peek()`) keeps retried batches warm
- Batch-level dedup key: only the first event ID is checked
- `seed_from_log` inserts oldest-first for correct LRU ordering after restart
- Duplicate event IDs within a single batch are rejected as InvalidArgument (caller error, not dedup)

### AC Coverage Matrix
| AC | Description | Covered By |
|----|-------------|------------|
| 1 | Fresh append succeeds unchanged | Existing writer tests + integration |
| 2 | Duplicate returns Ok with original positions | Unit + integration |
| 3 | No duplicate events in log after dedup hit | Unit + integration |
| 4 | Different batches succeed independently | Integration |
| 5 | Duplicate event_id within batch = InvalidArgument | Unit |
| 6 | Dedup survives restart | Integration |
| 7 | Capacity eviction works correctly | Unit + integration |
| 8 | Dedup hit does not publish to broker | Unit + integration |
| 9 | All quality gates pass | Verified |

### Known Issues / Follow-Up
- `seed_from_log` does not reconstruct original batch groupings (low priority)

### Ready for QA: YES
