# Code Review: Ticket 1 -- Add `DedupIndex` module (`src/dedup.rs`)

**Ticket:** 1 -- Add `DedupIndex` module (`src/dedup.rs`)
**Impl Report:** prd/010-idempotent-appends-reports/ticket-01-impl.md
**Date:** 2026-02-26 15:00
**Verdict:** APPROVED

---

## AC Coverage

| AC # | Description | Status | Notes |
|------|-------------|--------|-------|
| 1 | `DedupIndex` struct has private `cache: LruCache<Uuid, Arc<Vec<RecordedEvent>>>`, derives nothing requiring `RecordedEvent: Clone` beyond existing | Met | Line 28-33: struct has single private field of correct type. No derive macros on `DedupIndex`. `RecordedEvent` already derives `Clone` (confirmed in `types.rs:64`), used only in `seed_from_log` to clone from borrowed slice. |
| 2 | `DedupIndex::new(capacity: NonZeroUsize) -> Self` | Met | Line 41-45: correct signature, delegates to `LruCache::new(capacity)`. |
| 3 | `check(&self, proposed: &[ProposedEvent]) -> Option<Arc<Vec<RecordedEvent>>>` checks first event's `event_id`; `None` for empty | Met | Line 61-65: uses `proposed.first()?` for early return on empty, `self.cache.peek(&first.event_id).cloned()` for lookup. `peek` is the correct choice for `&self` (no LRU promotion). |
| 4 | `record(&mut self, recorded: Vec<RecordedEvent>)` inserts one `Arc` shared across all per-event-ID entries | Met | Line 77-82: single `Arc::new(recorded)`, then `Arc::clone(&shared)` for each event ID. Correct shared-allocation pattern. |
| 5 | `seed_from_log(&mut self, events: &[RecordedEvent])` inserts ascending so highest-position events are LRU-hottest | Met | Line 97-105: iterates in order, `put` marks each as most-recently-used, so last inserted (highest position) is hottest. Each event becomes a single-element batch, which is correct since original batch boundaries are unknown at recovery time. |
| 6 | All public items have doc comments | Met | Module-level `//!` doc (lines 1-6), struct doc (lines 19-27), field doc (lines 30-32), `new` doc (lines 36-40), `check` doc (lines 47-60), `record` doc (lines 67-76), `seed_from_log` doc (lines 84-96). All comprehensive with `# Arguments` and `# Returns` sections. |
| 7 | Test: `new(4)` then `check(&[])` returns `None` | Met | `check_empty_slice_returns_none` (lines 138-141). |
| 8 | Test: record batch of 2, check returns `Some(arc)` with `len()==2`, same `Arc` pointer for both IDs | Met | `record_batch_then_check_returns_same_arc` (lines 144-166). Uses `Arc::ptr_eq` for pointer identity check. |
| 9 | Test: check for unrecorded ID returns `None` | Met | `check_unknown_event_id_returns_none` (lines 169-179). |
| 10 | Test: LRU eviction with capacity=2, three batches | Met | `lru_eviction_drops_oldest_entry` (lines 182-206). Correctly records X, Y, Z; asserts X evicted, Y and Z remain. `peek()` in intermediate checks does not disturb LRU order. |
| 11 | Test: seed_from_log with 5 events capacity=3, only 2-4 remain | Met | `seed_from_log_evicts_oldest_positions` (lines 209-232). Verifies positions 0,1 evicted and 2,3,4 present. |
| 12 | Test: seed_from_log then check returns correct event data | Met | `seed_from_log_check_returns_correct_event_data` (lines 235-251). Checks `event_id`, `stream_id`, `global_position`, `stream_version`. |
| 13 | Quality gates pass | Met | Verified independently: `cargo build` (0 warnings), `cargo clippy --all-targets --all-features --locked -- -D warnings` (clean), `cargo fmt --check` (clean), `cargo test` (216 tests, 0 failures). |

## Issues Found

### Critical (must fix before merge)

None.

### Major (should fix, risk of downstream problems)

None.

### Minor (nice to fix, not blocking)

1. **`#![allow(dead_code)]` is a module-level inner attribute** (`src/dedup.rs:9`). This correctly suppresses warnings for the entire module since no callers exist yet. The implementer documented the reason with a comment on line 8. This should be removed in Ticket 2 when the writer task integrates the module. No action needed now -- just a note for the Ticket 2 implementer.

## Suggestions (non-blocking)

1. **`check` uses `peek()` (no LRU promotion):** The implementer correctly documented this design decision in both the code comment (line 63) and the impl report's Concerns section. The PRD suggests promotion on hit, but the ticket's AC specifies `&self`, which is incompatible with `get()` (requires `&mut self`). This is the right call for now -- Ticket 2 can change the signature to `&mut self` and switch to `get()` if promotion semantics are needed. Well-handled.

2. **Test helpers `proposed()` and `recorded()` are clean and minimal.** If future tickets in this PRD need similar helpers in integration tests, these could be extracted to a shared test utility module, but that is out of scope for this ticket.

## Scope Check

- Files within scope: YES
  - Created: `src/dedup.rs` (in scope)
  - Modified: `src/lib.rs` (in scope -- single line addition)
  - Modified: `Cargo.toml` (in scope -- `lru = "0.12"` added)
  - Modified: `Cargo.lock` (auto-generated, expected)
- Scope creep detected: NO
- Unauthorized dependencies added: NO (`lru = "0.12"` is explicitly called for in the ticket)

## Risk Assessment

- Regression risk: LOW -- This is a new, self-contained module with no callers. The only changes to existing files are a single `pub(crate) mod dedup;` line in `lib.rs` and the `lru` dependency in `Cargo.toml`. All 210 pre-existing tests continue to pass (216 total - 6 new = 210 pre-existing).
- Security concerns: NONE
- Performance concerns: NONE -- LRU cache operations are O(1). `seed_from_log` is O(n) in the number of events, which is the minimum possible. `record` is O(k) where k is batch size. Memory is bounded by the capacity parameter.
