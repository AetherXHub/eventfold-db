# Implementation Report: Ticket 5 -- `run_writer` Task Loop and `spawn_writer`

**Ticket:** 5 - `run_writer` Task Loop and `spawn_writer`
**Date:** 2026-02-25 14:30
**Status:** COMPLETE

---

## Files Changed

### Created
- None

### Modified
- `src/writer.rs` - Added `run_writer` (pub(crate) async fn) and `spawn_writer` (pub fn), plus 12 integration tests (AC-1 through AC-9b)
- `src/lib.rs` - Added `spawn_writer` to the `pub use writer::{...}` re-export line

## Implementation Notes
- `run_writer` uses `while let Some(first) = rx.recv().await` for the main loop, then `rx.try_recv()` to drain additional pending requests into a `Vec` for batching. Each request is processed individually via `store.append()`, and the result is sent back through the oneshot channel.
- If `response_tx.send()` fails (receiver dropped), a `tracing::warn!` is emitted -- this follows the ticket spec exactly.
- `spawn_writer` calls `store.log()` to clone the `Arc<RwLock<EventLog>>` BEFORE moving `store` into `tokio::spawn`. This is the critical sequencing required by the ticket and the architecture (the ReadIndex needs to share the same Arc as the Store's internal state).
- `WriterHandle::new(tx)` constructor from Ticket 4 is reused as directed.
- For AC-8 (backpressure test), `try_send` was used instead of `tokio::time::timeout` because the async timeout approach yields to the runtime, allowing the spawned writer task to drain the channel before the second send. Using the synchronous `try_send` avoids this race entirely and directly proves the bounded channel property.
- Two test helpers (`proposed()` and `temp_store()`) were added to reduce boilerplate across the 12 tests.

## Acceptance Criteria
- [x] AC: `run_writer` is `pub(crate) async fn` with correct loop semantics (recv, try_recv drain, store.append, response_tx.send, tracing::warn on dropped receiver, clean exit on None)
- [x] AC: `spawn_writer` is `pub` fn, calls `store.log()` before move, constructs ReadIndex, returns `(WriterHandle, ReadIndex, JoinHandle<()>)`
- [x] AC-1: Basic append through writer - `global_position == 0`, `stream_version == 0`
- [x] AC-2: 3 sequential appends - positions 0, 1, 2; stream versions 0, 1, 2
- [x] AC-3: 10 concurrent appends with `Any` - all positions unique, set equals {0..9}
- [x] AC-4a: `NoStream` twice returns `WrongExpectedVersion`
- [x] AC-4b: `NoStream` then `Exact(0)` succeeds
- [x] AC-4c: `NoStream` then `Exact(5)` returns `WrongExpectedVersion`
- [x] AC-5: `read_index.read_all` and `read_index.read_stream` reflect writes
- [x] AC-6: Durability - 5 events survive restart via Store::open at same path
- [x] AC-7: Graceful shutdown - drop handle, join resolves within 1 second
- [x] AC-8: Backpressure - `try_send` returns `Full` on capacity=1 channel
- [x] AC-9a: Oversized event returns `EventTooLarge`
- [x] AC-9b: Valid append succeeds after `EventTooLarge` error (writer not poisoned)

## Test Results
- Lint: PASS (`cargo clippy --all-targets --all-features --locked -- -D warnings` -- zero warnings)
- Tests: PASS (`cargo test` -- 112 passed, 0 failed, 12 new tests)
- Build: PASS (`cargo build` -- zero warnings)
- Format: PASS (`cargo fmt --check` -- no diffs)
- New tests added:
  - `src/writer.rs::tests::ac1_basic_append_through_writer`
  - `src/writer.rs::tests::ac2_sequential_appends_have_contiguous_positions`
  - `src/writer.rs::tests::ac3_concurrent_appends_serialized`
  - `src/writer.rs::tests::ac4a_nostream_twice_returns_wrong_expected_version`
  - `src/writer.rs::tests::ac4b_exact_0_after_nostream_succeeds`
  - `src/writer.rs::tests::ac4c_exact_5_after_nostream_returns_wrong_expected_version`
  - `src/writer.rs::tests::ac5_read_index_reflects_writes`
  - `src/writer.rs::tests::ac6_durability_survives_restart`
  - `src/writer.rs::tests::ac7_graceful_shutdown_on_handle_drop`
  - `src/writer.rs::tests::ac8_backpressure_bounded_channel`
  - `src/writer.rs::tests::ac9a_event_too_large_returns_error`
  - `src/writer.rs::tests::ac9b_writer_not_poisoned_after_error`

## Concerns / Blockers
- None
