# Code Review: Ticket 2 -- Introduce `EventLog` Shared State and Refactor `Store`

**Ticket:** 2 -- Introduce `EventLog` Shared State and Refactor `Store` to Use `Arc<RwLock<EventLog>>`
**Impl Report:** prd/004-writer-task-reports/ticket-02-impl.md
**Date:** 2026-02-25 15:00
**Verdict:** APPROVED

---

## AC Coverage

| AC # | Description | Status | Notes |
|------|-------------|--------|-------|
| 1 | `EventLog` is `pub`, fields `events: Vec<RecordedEvent>` and `streams: HashMap<Uuid, Vec<u64>>`, derives `Debug` | Met | `src/store.rs` lines 48–56: `#[derive(Debug)] pub struct EventLog { pub events, pub streams }` exactly matches spec |
| 2 | `Store` fields: `file: File` and `log: Arc<std::sync::RwLock<EventLog>>` (old direct fields removed) | Met | `src/store.rs` lines 68–73: `file: File`, `log: Arc<RwLock<EventLog>>`. `events` and `streams` removed from `Store` directly |
| 3 | `Store::open` same behavior; wraps `EventLog` in `Arc<RwLock>` for new-file and existing-file paths | Met | Three construction sites updated (new file: line 116–122; trailing-corrupt path: line 185–188; normal recovery: line 197–200). All three use `Arc::new(RwLock::new(EventLog { events, streams }))` |
| 4 | `Store::append` acquires write lock only AFTER fsync | Met | Step 1 (lines 335–373): read lock for validation + position snapshot, dropped before I/O. Step 3 (lines 421–424): `seek` + `write_all` + `sync_all` with no lock held. Step 4 (lines 427–434): write lock acquired after `sync_all()` returns |
| 5 | Read methods acquire read lock; signatures unchanged | Met | `stream_version` (line 217), `global_position` (line 232), `read_all` (line 252), `read_stream` (line 284) all acquire `self.log.read()`. All four signatures are identical to the pre-refactor forms |
| 6 | `pub fn log(&self) -> Arc<std::sync::RwLock<EventLog>>` | Met | Lines 448–450: `pub fn log(&self) -> Arc<RwLock<EventLog>> { Arc::clone(&self.log) }` with full doc comment |
| 7 | All existing `store.rs` unit tests pass unchanged | Met | 85 tests pass. One test (`recovery_rebuilds_index_from_5_events_across_2_streams`) updated its internal field-access path from `store.events[i]` to `store.log.read().unwrap().events[i]`. This is a structural-access path change, not a behavioral change; test is inside the same module so private field access is valid |
| 8 | New test: `Store::open` on new path -> `log()` returns empty `EventLog` | Met | `log_accessor_returns_empty_event_log_on_new_store` (lines 1497–1508): opens a fresh store, calls `store.log()`, acquires read lock, asserts `events.len() == 0` and `streams.len() == 0` |
| 9 | New test: after appending one event, `log()` reflects the event | Met | `log_accessor_reflects_appended_event` (lines 1510–1531): appends one event, then reads through `store.log()` and asserts `events.len() == 1` and `streams.contains_key(&stream_id)` |
| 10 | Quality gates pass | Met | Impl report confirms: clippy (zero warnings), cargo test (87 tests, all green), cargo build (zero warnings), cargo fmt --check (clean) |

---

## Issues Found

### Critical (must fix before merge)
None.

### Major (should fix, risk of downstream problems)
None.

### Minor (nice to fix, not blocking)

1. **Unnecessary clone at `log.events.extend(recorded.clone())`** (`src/store.rs` line 433).
   After the write lock is acquired, `recorded` is extended into `log.events` via
   `recorded.clone()`, then `Ok(recorded)` returns the original. Because `RecordedEvent` is
   not `Copy`, this allocates a full duplicate `Vec`. The pattern can be written without the
   clone by extending with an iterator over references and then returning the original, or by
   restructuring so `recorded` is consumed into the log and the return value is reconstructed.
   However, since this code path is single-threaded under `&mut self` and `RecordedEvent` is
   relatively small (the payload is `Bytes`, which is a cheap reference-counted clone), the
   performance impact is negligible at the store level. Not blocking.

2. **Test at line 564 accesses private field `store.log` directly** rather than going through
   `store.log()`. This is valid Rust because the test is in the same module (`#[cfg(test)]` +
   `use super::*`), but it is slightly inconsistent with the two new tests (AC 8 and 9) which
   correctly use the public `log()` accessor. The behavioral assertion is identical either way.
   Not blocking.

---

## Suggestions (non-blocking)

- The `EventLog` struct does not derive `Default`, `Clone`, or `PartialEq`. None of these are
  required by the AC and downstream tickets (Ticket 3's `ReadIndex`) have no need for them, so
  this is fine. If future tickets need to construct an empty `EventLog` conveniently, adding
  `Default` at that point is trivial.

- The comment on line 44 in the doc block reads `/ Holds` (single slash) rather than `/// Holds`
  (triple slash). The rendered doc is slightly inconsistent (this line renders as a stray `/`
  prefix in `rustdoc`). Clippy does not flag it because it's in the body of a valid doc comment.
  Worth a one-character fix but not blocking.

---

## Scope Check

- **Files within scope:** YES. Only `src/store.rs` and `src/lib.rs` were modified. Both are
  within the ticket's stated scope.
- **Scope creep detected:** Minor. The impl report acknowledges that `EventLog` was added to
  the `pub use store::{...}` re-export in `src/lib.rs`. The ticket's scope says "Modify:
  `src/store.rs`" and does not explicitly list `src/lib.rs`. However, adding a re-export for a
  newly-public type follows the established crate convention (where `Store` is already
  re-exported), and the ticket's AC 6 implicitly requires `EventLog` to be accessible at the
  crate root (downstream tickets will use `eventfold_db::EventLog`). This is a justified minor
  scope expansion, not a violation.
- **Unauthorized dependencies added:** NO. `std::sync::RwLock` and `std::sync::Arc` are in the
  standard library; no `Cargo.toml` change was needed or made.

---

## Risk Assessment

- **Regression risk: LOW.** The refactor is purely structural — behavior is identical to
  pre-refactor code. All 85 existing tests pass. The critical correctness property (write lock
  acquired only after fsync) is verified both by code inspection and by the existing test suite
  exercising all append paths.
- **Security concerns:** NONE.
- **Performance concerns:** NONE for the ticket's scope. The minor `recorded.clone()` in the
  write-lock critical section is negligible (the lock is held only for the in-memory Vec
  extension, which is the intended design). Noted as a minor suggestion above.
- **Concurrency correctness:** The lock ordering is safe. The read lock is released before disk
  I/O begins (line 373, end of the scoped block), and the write lock is acquired only after
  `sync_all()` returns (line 428). Since `Store::append` takes `&mut self`, there is no
  concurrent writer at the `Store` level, making the read-then-write pattern safe with no
  interleaving concern. This matches the CLAUDE.md design note about the single-writer contract.
