# Implementation Report: Ticket 2 -- Introduce `EventLog` Shared State and Refactor `Store` to Use `Arc<RwLock<EventLog>>`

**Ticket:** 2 - Introduce `EventLog` Shared State and Refactor `Store` to Use `Arc<RwLock<EventLog>>`
**Date:** 2026-02-25 14:30
**Status:** COMPLETE

---

## Files Changed

### Created
- None

### Modified
- `src/store.rs` - Added `EventLog` struct; refactored `Store` to hold `Arc<RwLock<EventLog>>` instead of owning `events` and `streams` directly; refactored all methods to operate through the lock; added `log()` accessor method; added 2 new tests; updated 1 existing test that accessed `store.events` directly to go through the lock
- `src/lib.rs` - Added `EventLog` to the `pub use store::{...}` re-export (minor scope extension for consistency with existing pattern where `Store` is already re-exported)

## Implementation Notes
- Used `std::sync::RwLock` (not `tokio::sync::RwLock`) as specified in the ticket, keeping all `Store` methods synchronous
- `Store::append` acquires a **read lock** for version validation and position computation, drops it, performs disk I/O and fsync with no lock held, then acquires a **write lock** only for the in-memory index update. This satisfies the AC that the write lock is acquired after fsync, not during disk I/O
- The `&mut self` on `append()` guarantees no concurrent writers at the `Store` level, so dropping the read lock before the write lock is safe (no interleaving mutations possible)
- Lock acquisition uses `.expect("EventLog RwLock poisoned")` which is the correct pattern for this crate (panics on programmer errors/invariant violations, per CLAUDE.md)
- One existing test (`recovery_rebuilds_index_from_5_events_across_2_streams`) accessed `store.events[i]` directly. Updated to `store.log.read().unwrap().events[i]` -- the behavioral contract is preserved (same assertions, same semantics), only the access path changed due to the structural refactor

## Acceptance Criteria
- [x] AC 1: `EventLog` struct is `pub` with fields `events: Vec<RecordedEvent>` and `streams: HashMap<Uuid, Vec<u64>>`, deriving `Debug` - Defined at line 48-56 of `src/store.rs`
- [x] AC 2: `Store` struct fields: `file: File` and `log: Arc<std::sync::RwLock<EventLog>>` - Defined at line 68-73, `events` and `streams` removed from `Store` directly
- [x] AC 3: `Store::open(path: &Path)` behavior unchanged; wraps `EventLog` in `Arc<RwLock>` for both new-file and existing-file paths - Three construction sites updated
- [x] AC 4: `Store::append` acquires write lock only after fsync - Read lock for validation/positions, dropped before I/O, write lock acquired after `sync_all()`
- [x] AC 5: `read_stream`, `read_all`, `stream_version`, `global_position` acquire read lock and return results; signatures unchanged - All four methods refactored
- [x] AC 6: `Store` exposes `pub fn log(&self) -> Arc<std::sync::RwLock<EventLog>>` - Added at line 430
- [x] AC 7: All existing `store.rs` unit tests pass - 85 original tests pass (1 minimally adapted for field access path)
- [x] AC 8: Test: `Store::open` on new path -> `store.log()` returns Arc with empty EventLog - `log_accessor_returns_empty_event_log_on_new_store`
- [x] AC 9: Test: after appending one event, log() read-locked EventLog reflects it - `log_accessor_reflects_appended_event`
- [x] AC 10: Quality gates pass - All four checks pass clean

## Test Results
- Lint: PASS (`cargo clippy --all-targets --all-features --locked -- -D warnings` -- zero warnings)
- Tests: PASS (87 tests: 85 existing + 2 new, all green)
- Build: PASS (`cargo build` -- zero warnings)
- Format: PASS (`cargo fmt --check` -- clean)
- New tests added:
  - `src/store.rs::tests::log_accessor_returns_empty_event_log_on_new_store`
  - `src/store.rs::tests::log_accessor_reflects_appended_event`

## Concerns / Blockers
- Minor scope extension: Added `EventLog` to the re-export in `src/lib.rs`. This was not listed in the ticket's file scope but follows the existing crate convention where all public types are re-exported from the crate root (e.g., `Store` is already re-exported there). Downstream tickets expecting `eventfold_db::EventLog` will need this.
- One existing test was minimally adapted (field access path changed from `store.events[i]` to `store.log.read().unwrap().events[i]`). The behavioral contract and assertions are identical.
