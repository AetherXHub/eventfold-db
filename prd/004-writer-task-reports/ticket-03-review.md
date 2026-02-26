# Code Review: Ticket 3 -- ReadIndex -- Shared Read-Only View

**Ticket:** 3 -- ReadIndex -- Shared Read-Only View of the In-Memory Index
**Impl Report:** prd/004-writer-task-reports/ticket-03-impl.md
**Date:** 2026-02-25 18:00
**Verdict:** APPROVED

---

## AC Coverage

| AC # | Description | Status | Notes |
|------|-------------|--------|-------|
| 1 | `ReadIndex` is `pub`, `Clone`, `Debug`, holds `log: Arc<std::sync::RwLock<EventLog>>` | Met | `#[derive(Clone, Debug)]` on line 23; field `log: Arc<RwLock<EventLog>>` on line 26 |
| 2 | `ReadIndex::new(log: Arc<std::sync::RwLock<EventLog>>) -> ReadIndex` | Met | Implemented at `reader.rs:39` |
| 3 | `read_all(&self, from_position: u64, max_count: u64) -> Vec<RecordedEvent>` | Met | Implemented at `reader.rs:129`; slice logic is identical to `Store::read_all` |
| 4 | `read_stream(&self, stream_id: Uuid, from_version: u64, max_count: u64) -> Result<Vec<RecordedEvent>, Error>` with `StreamNotFound` | Met | Implemented at `reader.rs:93`; `StreamNotFound` returned on missing stream_id |
| 5 | `stream_version(&self, stream_id: &Uuid) -> Option<u64>` | Met | Implemented at `reader.rs:55`; returns `None` for missing streams |
| 6 | `global_position(&self) -> u64` | Met | Implemented at `reader.rs:70`; returns `events.len() as u64` |
| 7 | `ReadIndex` is `Clone` (cloning produces new handle backed by same `Arc`) | Met | `Clone` is derived; `two_clones_observe_same_data_after_append` test validates shared backing |
| 8 | Test: `read_all` after appending 3 events returns 3 events | Met | `read_all_returns_all_events_from_store` at `reader.rs:194` |
| 9 | Test: `read_stream` returns correct events in version order | Met | `read_stream_returns_correct_events_in_version_order` at `reader.rs:284`; asserts `stream_version` and `stream_id` per event |
| 10 | Test: `read_stream` on non-existent UUID returns `Err(StreamNotFound)` | Met | `read_stream_nonexistent_returns_stream_not_found` at `reader.rs:268`; uses exhaustive match to confirm the exact UUID in the error |
| 11 | Test: two `ReadIndex` clones both observe same data after append | Met | `two_clones_observe_same_data_after_append` at `reader.rs:238`; confirms initial-empty + post-append visibility for both clones |
| 12 | Quality gates pass | Met | Verified independently: `cargo test` (100 tests, 0 failures), `cargo clippy -- -D warnings` (clean), `cargo fmt --check` (clean), `cargo build` (zero warnings) |

---

## Issues Found

### Critical (must fix before merge)
None.

### Major (should fix, risk of downstream problems)
None.

### Minor (nice to fix, not blocking)

- **`reader.rs:39` -- return type can use `Self`**
  The constructor `pub fn new(log: Arc<RwLock<EventLog>>) -> ReadIndex` repeats the type name. The idiomatic Rust convention (and what the global CLAUDE.md calls out as preferred style) is `-> Self` for constructors inside `impl` blocks. This is a style point only; the compiler accepts either form.

- **`reader.rs:129` -- `to_vec()` vs `.iter().cloned().collect()`**
  `read_all` uses `log.events[...].to_vec()` which copies the slice via `Clone`. This is semantically correct and efficient (single allocation, `RecordedEvent` is `Clone`). However, `read_stream` uses `.iter().map(|&pos| log.events[pos].clone()).collect()`, which is a slightly different idiom. Neither is wrong, but a comment noting that `RecordedEvent` contains `Bytes` (which uses reference-counted allocation) would clarify that the clone is cheap for the `Bytes` fields.

- **`reader.rs:26` -- private field is not accessible to external callers of `ReadIndex::new`**
  The `log` field is private (no `pub` qualifier). That is correct -- callers use `new()`. No issue here, but it is worth noting the field privacy is consistent with the pattern in `Store`, where `log` is also private.

---

## Suggestions (non-blocking)

- Consider adding a `read_stream` test that exercises `from_version > 0` (e.g., reading the second event of a three-event stream). The current test only reads `from_version = 0`. The slice logic `start = from_version.min(stream_len)` / `end = from_version.saturating_add(max_count).min(stream_len)` is correctly cribbed from `Store`, but a targeted mid-stream test would give independent confidence.

- The impl report mentions that `pub use store::EventLog;` was removed from `lib.rs` by a parallel ticket. If downstream tickets (e.g., the writer or service tickets) need to construct `ReadIndex::new(...)` externally, they will need to use `store::EventLog` via `use eventfold_db::store::EventLog`. Since `store` is `pub mod`, this is accessible; the loss of the crate-root re-export only affects ergonomics, not correctness. The orchestrator should confirm whether any future ticket calls `ReadIndex::new` with a freshly constructed `EventLog` (rather than obtaining the `Arc` from `Store::log()`), and if so, add `pub use store::EventLog;` back to `lib.rs`.

- `read_stream` accepts `stream_id: Uuid` by value even though the implementation only needs a reference (it calls `log.streams.get(&stream_id)`). This matches `Store::read_stream`'s signature exactly, so consistency is preserved. The by-value signature is a minor allocation cost (UUID is 16 bytes = `Copy`, so effectively free), but it's worth noting the pattern deviates from CLAUDE.md's "prefer borrowing over ownership" principle. Again, this is established by the existing `Store` API, so changing it here in isolation would create inconsistency.

---

## Scope Check

- Files within scope: YES
  - `src/reader.rs` -- created (in scope)
  - `src/lib.rs` -- modified with `pub mod reader;` and `pub use reader::ReadIndex;` (in scope)
- Scope creep detected: NO
- Unauthorized dependencies added: NO
  - No new entries in `Cargo.toml`; `uuid` and `std::sync` are pre-existing dependencies

---

## Risk Assessment

- **Regression risk: LOW**
  The implementation is purely additive. `ReadIndex` is a new type that wraps the existing `Arc<RwLock<EventLog>>` already owned by `Store`. No existing code paths are mutated. The `lib.rs` changes are additive re-exports. The 9 new tests all pass alongside the existing 91. No risk of regressions to the codec, store, or writer modules.

- **Security concerns: NONE**
  Read-only access pattern. No I/O, no file handles, no user-controlled deserialization in this module.

- **Performance concerns: NONE**
  `read_all` acquires a single read lock and copies the slice in one allocation. `read_stream` acquires a single read lock and does a single pass through the stream positions. Lock hold times are short (no I/O inside the lock). `Bytes` fields clone cheaply (ref-counted). The design correctly matches the architecture's intent: reads bypass the writer task entirely and go directly to the in-memory index.

- **Correctness notes:**
  - Poisoned lock handling via `.expect("EventLog RwLock poisoned")` correctly panics on a violated invariant (a prior write-lock holder panicked, leaving the log in an unknown state). This matches the CLAUDE.md convention of using `.expect()` for invariant violations and is consistent with the pattern established in `Store`.
  - The `read_stream` indexing into `log.events[global_pos as usize]` is safe by the contiguous-positions invariant documented in CLAUDE.md ("Landmines" section). If that invariant were violated, this would panic -- but violating it is a programmer error, not an operational failure.
  - `stream_version` computes `positions.len() as u64 - 1`. This is safe because the branch is only reached when `positions` is `Some` (meaning at least one event was appended), so `positions.len() >= 1` is guaranteed.
