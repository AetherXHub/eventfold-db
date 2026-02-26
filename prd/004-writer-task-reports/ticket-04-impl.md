# Implementation Report: Ticket 4 -- `AppendRequest` and `WriterHandle` Types in `src/writer.rs`

**Ticket:** 4 - `AppendRequest` and `WriterHandle` Types in `src/writer.rs`
**Date:** 2026-02-25 12:00
**Status:** COMPLETE

---

## Files Changed

### Created
- `src/writer.rs` - New module with `AppendRequest` struct, `WriterHandle` struct with `append` method, and 4 unit tests.

### Modified
- `src/lib.rs` - Added `pub mod writer;` and `pub use writer::WriterHandle;` re-export.

## Implementation Notes
- `AppendRequest` is a plain struct with `pub` fields matching the ticket specification exactly: `stream_id: Uuid`, `expected_version: ExpectedVersion`, `events: Vec<ProposedEvent>`, `response_tx: oneshot::Sender<Result<Vec<RecordedEvent>, Error>>`.
- `WriterHandle` derives `Clone` and holds `tx: mpsc::Sender<AppendRequest>` (private field). A `pub fn new(tx)` constructor is provided for downstream code to create handles.
- `WriterHandle::append` creates a oneshot channel, sends the `AppendRequest` over the mpsc channel, and awaits the oneshot response. Both the mpsc send error and the oneshot recv error are mapped to `Error::InvalidArgument("writer task closed".into())`.
- No `Debug` derive on `AppendRequest` because `oneshot::Sender` does not implement `Debug` in a useful way and the ticket did not require it.
- No `Debug` derive on `WriterHandle` because the ticket only required `Clone`. Adding `Debug` could be done in a future ticket if needed.
- Followed existing codebase patterns: doc comments on all public items, `thiserror` error propagation, `#[cfg(test)] mod tests` for unit tests.

## Acceptance Criteria
- [x] AC 1: `AppendRequest` struct has fields `stream_id: Uuid`, `expected_version: ExpectedVersion`, `events: Vec<ProposedEvent>`, `response_tx: oneshot::Sender<Result<Vec<RecordedEvent>, Error>>` - All four fields present and verified by `append_request_has_required_fields` test.
- [x] AC 2: `WriterHandle` is `pub`, derives `Clone`, holds `tx: mpsc::Sender<AppendRequest>` - Struct definition at line 42-46 with `#[derive(Clone)]`.
- [x] AC 3: `WriterHandle::append` is async, creates oneshot, sends `AppendRequest`, awaits receiver, maps closed-channel to `Error::InvalidArgument("writer task closed")` - Implementation at lines 79-106.
- [x] AC 4: Loopback test - `writer_handle_append_loopback` test creates mpsc channel, constructs handle, spawns receiver task that asserts field values and replies `Ok(vec![])`, caller receives `Ok(vec![])`.
- [x] AC 5: Dropped receiver test - `append_returns_error_when_receiver_dropped` test drops receiver, calls append, asserts `Err(Error::InvalidArgument("writer task closed"))`.
- [x] AC 6: Clone test - `cloned_handles_send_to_same_channel` test clones the handle, both independently send requests to the same channel successfully.
- [x] AC 7: Quality gates pass.

## Test Results
- Lint: PASS (`cargo clippy --all-targets --all-features --locked -- -D warnings` -- zero warnings)
- Tests: PASS (91 tests, 4 new in `writer::tests`)
- Build: PASS (`cargo build` -- zero warnings)
- Format: PASS (`cargo fmt --check` -- clean)
- New tests added:
  - `src/writer.rs::tests::append_request_has_required_fields`
  - `src/writer.rs::tests::writer_handle_append_loopback`
  - `src/writer.rs::tests::append_returns_error_when_receiver_dropped`
  - `src/writer.rs::tests::cloned_handles_send_to_same_channel`

## Concerns / Blockers
- An untracked `src/reader.rs` file exists in the working directory from a parallel ticket (Ticket 3 of PRD 004). The rust-analyzer linter periodically auto-wires `pub mod reader;` into `lib.rs`. This file has clippy warnings (`dead_code`, `unused_imports`) that would cause clippy to fail if the `pub mod reader;` declaration is present. My changes do NOT include `pub mod reader;` -- I ensured `lib.rs` only contains the committed baseline plus my additions (`pub mod writer;` and `pub use writer::WriterHandle;`). The reader module's issues should be resolved by its own ticket.
- The prior work summary mentioned `EventLog` being re-exported from `lib.rs`, but the committed `lib.rs` on main does not include that re-export. My changes preserve the committed baseline. The `EventLog` re-export may need to be added by another ticket.
