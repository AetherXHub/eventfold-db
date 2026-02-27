# Implementation Report: Ticket 2 -- Integrate `DedupIndex` into writer task

**Ticket:** 2 - Integrate `DedupIndex` into writer task (`src/writer.rs`)
**Date:** 2026-02-27 00:30
**Status:** COMPLETE

---

## Files Changed

### Modified
- `src/writer.rs` - Added `DedupIndex` integration: `validate_batch_unique_ids()` helper, dedup check/record in `run_writer` loop, `dedup_capacity` parameter to `spawn_writer` with seed-from-log, 4 new tests, updated all existing test call sites.
- `src/dedup.rs` - Changed `check()` from `&self`/`peek()` to `&mut self`/`get()` for LRU promotion on hit. Removed `#![allow(dead_code)]`. Updated all unit tests to use `mut index`.
- `src/main.rs` - Added `dedup_capacity: NonZeroUsize` to `Config`, `EVENTFOLD_DEDUP_CAPACITY` env var parsing in `from_env()`, passed to `spawn_writer`. Added `DEFAULT_DEDUP_CAPACITY` constant. Updated config tests.
- `src/broker.rs` - Updated all `spawn_writer` call sites in broker tests to pass 4th `dedup_capacity` argument.
- `tests/broker_integration.rs` - Added `test_dedup_cap()` helper, updated `spawn_writer` calls.
- `tests/grpc_service.rs` - Added `test_dedup_cap()` helper, updated `spawn_writer` calls.
- `tests/server_binary.rs` - Added `test_dedup_cap()` helper, updated `spawn_writer` calls.
- `tests/writer_integration.rs` - Added `test_dedup_cap()` helper, updated `spawn_writer` call.

## Implementation Notes
- The implementer necessarily updated ALL `spawn_writer` call sites (including `src/main.rs` and integration tests) to maintain compilation. This overlaps with Ticket 3's scope but was required for the code to compile.
- `check()` signature changed from `&self` to `&mut self` and implementation from `peek()` to `get()`, enabling LRU promotion on dedup hits as the PRD intended.
- `validate_batch_unique_ids()` uses `HashSet` with early return on first duplicate, returning `Error::InvalidArgument`.
- Dedup check happens BEFORE `store.append()`. On hit, cached `Vec<RecordedEvent>` is cloned from Arc and returned without disk write or broker publish.
- On successful append, `dedup.record(recorded.clone())` is called BEFORE `broker.publish()`.
- `spawn_writer` constructs `DedupIndex`, seeds from `store.log().read().events`, then moves dedup into the spawned task.

## Acceptance Criteria
- [x] AC 1: `run_writer` signature gains `dedup: &mut DedupIndex` parameter.
- [x] AC 2: Dedup check before `store.append()`; hit returns cached result, skips append and broker.
- [x] AC 3: On successful append, `dedup.record(recorded.clone())` called before `broker.publish()`.
- [x] AC 4: `spawn_writer` signature: `(store, channel_capacity, broker, dedup_capacity: NonZeroUsize)`.
- [x] AC 5: Inside `spawn_writer`, `DedupIndex::new(dedup_capacity)` constructed and `seed_from_log` called.
- [x] AC 6: All existing `spawn_writer` call sites updated to pass `NonZeroUsize` dedup_capacity.
- [x] AC 7: Test: duplicate append returns Ok with same global_position - `dedup_hit_returns_same_positions`.
- [x] AC 8: Test: after dedup hit, `read_all` returns same count - `dedup_hit_does_not_duplicate_events_in_log`.
- [x] AC 9: Test: after dedup hit, broker receives no new messages - `dedup_hit_does_not_publish_to_broker`.
- [x] AC 10: Test: duplicate event_id within batch rejected with InvalidArgument - `duplicate_event_id_within_batch_rejected`.
- [x] AC 11: Quality gates pass.

## Test Results
- Build: PASS (`cargo build` -- zero warnings)
- Lint: PASS (`cargo clippy --all-targets --all-features --locked -- -D warnings` -- zero warnings)
- Format: PASS (`cargo fmt --check` -- no diffs)
- Tests: PASS (`cargo test` -- 220 tests, 0 failures)
- New tests added:
  - `src/writer.rs::tests::dedup_hit_returns_same_positions`
  - `src/writer.rs::tests::dedup_hit_does_not_duplicate_events_in_log`
  - `src/writer.rs::tests::dedup_hit_does_not_publish_to_broker`
  - `src/writer.rs::tests::duplicate_event_id_within_batch_rejected`

## Concerns / Blockers
- Scope extended beyond Ticket 2 to include Ticket 3 changes (main.rs config, integration test updates) because `spawn_writer` signature change required all call sites to be updated for compilation. Ticket 3 may be considered substantially complete.
- None other.
