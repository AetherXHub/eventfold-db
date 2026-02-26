# Code Review: Ticket 4 -- `AppendRequest` and `WriterHandle` Types

**Ticket:** 4 -- `AppendRequest` and `WriterHandle` Types in `src/writer.rs`
**Impl Report:** prd/004-writer-task-reports/ticket-04-impl.md
**Date:** 2026-02-25 14:00
**Verdict:** APPROVED

---

## AC Coverage

| AC # | Description | Status | Notes |
|------|-------------|--------|-------|
| 1 | `AppendRequest` struct has required fields | Met | All four fields present at `src/writer.rs` lines 23-32: `stream_id: Uuid`, `expected_version: ExpectedVersion`, `events: Vec<ProposedEvent>`, `response_tx: oneshot::Sender<Result<Vec<RecordedEvent>, Error>>` |
| 2 | `WriterHandle` is `pub`, derives `Clone`, holds `tx: mpsc::Sender<AppendRequest>` | Met | `pub struct WriterHandle` at line 43, `#[derive(Clone)]` at line 42, `tx` field at line 45 |
| 3 | `WriterHandle::append` is async, creates oneshot, sends, awaits, maps closed-channel to `InvalidArgument("writer task closed")` | Met | Implementation at lines 79-106. Both mpsc send error and oneshot recv error map to `Error::InvalidArgument("writer task closed".into())`. Error message checked via `msg.contains("writer task closed")` in AC-5 test. |
| 4 | Test: loopback with test receiver that replies via oneshot | Met | `writer_handle_append_loopback` at line 143. Spawns a task that receives the request, asserts `stream_id`, `expected_version`, `events.len()`, `events[0].event_id`, then replies `Ok(vec![])`. Caller receives `Ok(vec![])`. |
| 5 | Test: dropped receiver returns `Err(Error::InvalidArgument(..))` | Met | `append_returns_error_when_receiver_dropped` at line 179. Drops `rx`, calls `append`, asserts `InvalidArgument` with message containing "writer task closed". |
| 6 | Test: `WriterHandle::clone()` produces second handle, both send to same channel | Met | `cloned_handles_send_to_same_channel` at line 209. Both `handle_a` and `handle_b` independently send; responder task handles exactly 2 requests; both assert `is_ok()`. |
| 7 | Quality gates pass | Met | Independently verified: `cargo test` (100 pass, 0 fail), `cargo clippy -- -D warnings` (clean), `cargo build` (0 warnings), `cargo fmt --check` (clean). |

## Issues Found

### Critical (must fix before merge)
None.

### Major (should fix, risk of downstream problems)
None.

### Minor (nice to fix, not blocking)

1. **Test module does not use `use super::*`** (`src/writer.rs`, line 110): The `#[cfg(test)] mod tests` block imports via `use crate::...` inside each individual test function, rather than at the module level with `use super::*`. Every other test module in this codebase (`src/types.rs`, `src/error.rs`, `src/store.rs`, `src/reader.rs`) opens its `mod tests` block with `use super::*;`. While this compiles and passes all checks, it deviates from the established codebase convention and creates repetitive inline imports across four tests. Adding `use super::*;` (and collapsing the per-function imports) would align with convention.

2. **`AppendRequest` and `WriterHandle` lack `Debug`** (`src/writer.rs`): The impl report acknowledges this deliberately (oneshot::Sender does not implement `Debug` usefully). This is a reasonable call for now. However, a manual `Debug` implementation for `WriterHandle` (which only wraps `mpsc::Sender<AppendRequest>`, and `Sender` does implement `Debug`) would be feasible and useful for tracing. This is a suggestion for a future ticket, not a blocker.

3. **`unwrap_err()` in test at line 202**: The call `result.unwrap_err()` in `append_returns_error_when_receiver_dropped` uses the pattern the codebase discourages in library code, but this is test code so it is acceptable. However, the test also already does `assert!(result.is_err())` on line 201, making the subsequent `unwrap_err()` safe. The style is internally consistent.

## Suggestions (non-blocking)

- The loopback test (`writer_handle_append_loopback`) verifies `result.is_ok()` and then immediately calls `.expect("should be Ok")` on the same `result` (line 175-176). Since `.is_ok()` is asserted first, the `.expect()` will never fire its message in a failing test -- the `assert!` panics first. A single `assert_eq!(result.expect("should be Ok"), vec![])` would be cleaner and still correct.

- The error mapping for a dropped oneshot sender (line 103-105) maps `RecvError` (which fires when the writer task panics or is cancelled after accepting the request but before responding) identically to the mpsc `SendError` (writer not yet started or already shut down). These are operationally distinct failure modes. A future ticket might distinguish them with a `WriterCrashed` variant, but for the scope of this ticket, treating both as `InvalidArgument("writer task closed")` is correct per the AC.

## Scope Check

- **Files within scope: YES** -- The ticket scope is `src/writer.rs` (create) and `src/lib.rs` (modify). The working tree has additional uncommitted changes in `src/store.rs`, `src/reader.rs`, and `Cargo.toml`, but these originate from Tickets 1, 2, and 3 (verified by reading the tickets file and the impl report). The Ticket 4 implementer explicitly noted this in the Concerns section and confirmed they did not add `pub mod reader;` themselves -- it was already present from Ticket 3. The lib.rs additions for this ticket are limited to `pub mod writer;` and `pub use writer::WriterHandle;`, which is exactly what the ticket specifies.
- **Scope creep detected: NO**
- **Unauthorized dependencies added: NO** -- `tokio` (used for `oneshot` and `mpsc`) was added by Ticket 1. No new Cargo.toml entries were introduced by this ticket.

## Risk Assessment

- **Regression risk: LOW** -- `src/writer.rs` is a new file with no side effects on existing modules. The `lib.rs` additions are purely additive (two new lines). All 100 existing tests continue to pass.
- **Security concerns: NONE**
- **Performance concerns: NONE** -- The implementation correctly avoids allocations in the hot path (the oneshot channel and `AppendRequest` struct are stack-allocated; the `Vec<ProposedEvent>` is moved, not cloned). The mpsc send path is the expected tokio async channel mechanism.
