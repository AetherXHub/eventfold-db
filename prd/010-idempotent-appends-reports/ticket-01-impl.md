# Implementation Report: Ticket 1 -- Add `DedupIndex` module (`src/dedup.rs`)

**Ticket:** 1 - Add `DedupIndex` module (`src/dedup.rs`)
**Date:** 2026-02-26 14:30
**Status:** COMPLETE

---

## Files Changed

### Created
- `src/dedup.rs` - New module containing the `DedupIndex` struct backed by `lru::LruCache`, with `new`, `check`, `record`, and `seed_from_log` methods plus 6 unit tests.

### Modified
- `src/lib.rs` - Added `pub(crate) mod dedup;` module declaration (sorted alphabetically by `cargo fmt`).
- `Cargo.toml` - Added `lru = "0.12"` under `[dependencies]`.

## Implementation Notes
- `DedupIndex::check` uses `LruCache::peek()` (no LRU promotion) to match the `&self` receiver specified in the ticket. The PRD mentions promotion on hit, but the ticket's method signature (`&self`) is the implementation spec, and `peek` is the only read method compatible with shared references.
- `DedupIndex::record` creates one `Arc<Vec<RecordedEvent>>` and inserts it under each event ID in the batch, so all event IDs from the same batch share a single heap allocation.
- `DedupIndex::seed_from_log` inserts events one at a time in ascending global-position order, so oldest events enter first and get evicted first when capacity is exceeded. The newest events remain LRU-hottest.
- A module-level `#![allow(dead_code)]` suppresses warnings since no callers exist yet (ticket explicitly states this is a self-contained unit; the writer task integration is a later ticket).
- `DedupIndex` derives nothing -- the private `LruCache` field does not implement common traits like `Clone` or `Debug`, and the ticket does not require them. `RecordedEvent: Clone` (which already exists) is used only within `seed_from_log` to clone events from the borrowed slice into owned `Vec`s.

## Acceptance Criteria
- [x] AC 1: `DedupIndex` struct has private `cache: lru::LruCache<Uuid, Arc<Vec<RecordedEvent>>>` field and derives nothing that requires `RecordedEvent: Clone` beyond what already exists - struct defined at line 28 with private field at line 32; no derive macros on `DedupIndex`.
- [x] AC 2: `DedupIndex::new(capacity: NonZeroUsize) -> Self` - implemented at line 41.
- [x] AC 3: `DedupIndex::check(&self, proposed: &[ProposedEvent]) -> Option<Arc<Vec<RecordedEvent>>>` checks only first event's `event_id`; returns `None` for empty slice - implemented at line 61 using `proposed.first()?` and `peek()`.
- [x] AC 4: `DedupIndex::record(&mut self, recorded: Vec<RecordedEvent>)` inserts one `Arc` shared across all per-event-ID entries - implemented at line 77; single `Arc::new(recorded)` cloned per event ID via `Arc::clone`.
- [x] AC 5: `DedupIndex::seed_from_log(&mut self, events: &[RecordedEvent])` inserts in ascending order so highest-position events are LRU-hottest - implemented at line 97; iterates in order, `put` marks each as most-recently-used.
- [x] AC 6: All public items have doc comments - struct, all 4 methods, and the module itself have `///` or `//!` doc comments.
- [x] AC 7: Test: `new(4)` then `check(&[])` returns `None` - `check_empty_slice_returns_none` test.
- [x] AC 8: Test: record batch of two events, check returns `Some(arc)` with `len() == 2`, both IDs return same `Arc` pointer - `record_batch_then_check_returns_same_arc` test.
- [x] AC 9: Test: check for unrecorded event ID returns `None` - `check_unknown_event_id_returns_none` test.
- [x] AC 10: Test: LRU eviction with capacity=2, three batches, evicted ID returns `None` - `lru_eviction_drops_oldest_entry` test.
- [x] AC 11: Test: seed_from_log with 5 events capacity=3, only positions 2-4 remain - `seed_from_log_evicts_oldest_positions` test.
- [x] AC 12: Test: seed_from_log then check returns correct event data (stream_id, global_position) - `seed_from_log_check_returns_correct_event_data` test.
- [x] AC 13: Quality gates pass - all four commands pass clean.

## Test Results
- Lint: PASS (`cargo clippy --all-targets --all-features --locked -- -D warnings` -- zero warnings)
- Tests: PASS (`cargo test` -- 216 tests, 0 failures)
- Build: PASS (`cargo build` -- zero warnings)
- Format: PASS (`cargo fmt --check` -- no diffs)
- New tests added:
  - `src/dedup.rs::tests::check_empty_slice_returns_none`
  - `src/dedup.rs::tests::record_batch_then_check_returns_same_arc`
  - `src/dedup.rs::tests::check_unknown_event_id_returns_none`
  - `src/dedup.rs::tests::lru_eviction_drops_oldest_entry`
  - `src/dedup.rs::tests::seed_from_log_evicts_oldest_positions`
  - `src/dedup.rs::tests::seed_from_log_check_returns_correct_event_data`

## Concerns / Blockers
- The `check` method uses `peek()` (no LRU promotion) because the ticket specifies `&self`. If a downstream ticket needs promotion-on-hit semantics (as the PRD suggests), the signature would need to change to `&mut self` and use `get()` instead. This is a design decision for the writer integration ticket to resolve.
- None other.
