# Implementation Report: Ticket 2 -- Store::append -- Wrap Each Write in a Batch Envelope

**Ticket:** 2 - Store::append -- Wrap Each Write in a Batch Envelope
**Date:** 2026-02-26 14:30
**Status:** COMPLETE

---

## Files Changed

### Modified
- `src/store.rs` - Updated `Store::append` to wrap writes in a batch envelope (header + records + footer); added 4 new tests; marked 2 existing round-trip recovery tests with `#[ignore]`.
- `src/writer.rs` - Marked 1 existing round-trip recovery test (`ac6_durability_survives_restart`) with `#[ignore]` (out of stated scope, documented below).
- `tests/server_binary.rs` - Marked 2 existing round-trip recovery tests (`ac6_recovery_on_restart`, `ac7_graceful_shutdown_durability`) with `#[ignore]` (out of stated scope, documented below).

## Implementation Notes
- The `Store::append` method now builds a contiguous buffer of `batch_header || record_0..N-1 || batch_footer` before the single `write_all` + `sync_all` call.
- `first_global_pos` is captured before the record-building loop since `next_global` is incremented during the loop.
- CRC32 is computed using `crc32fast::Hasher` in streaming mode over the header bytes concatenated with all encoded record bytes, then passed to `encode_batch_footer`.
- The `encoded_batch` Vec is pre-allocated with the exact capacity (`BATCH_HEADER_SIZE + encoded_records.len() + BATCH_FOOTER_SIZE`).
- The variable previously named `encoded_batch` (which held only record bytes) was renamed to `encoded_records` to distinguish it from the final `encoded_batch` that includes the envelope.
- 5 existing tests that round-trip through `Store::open` after `Store::append` were marked `#[ignore]` because the recovery loop in `Store::open` does not yet understand batch envelopes. This is explicitly anticipated by the ticket and will be resolved by Ticket 3.

## Acceptance Criteria
- [x] AC 1: `Store::append` calls `codec::encode_batch_header(count, first_global_pos)` and `codec::encode_batch_footer(batch_crc)` -- implemented in Step 3 of the append method.
- [x] AC 2: Buffer passed to `write_all` is `batch_header || records || batch_footer` (single contiguous `Vec<u8>`) -- the `encoded_batch` Vec is built with all three segments.
- [x] AC 3: `sync_all` is still called after `write_all` and before the index write lock -- line ordering preserved (Step 4 write_all+sync_all, Step 5 write lock).
- [x] AC 4 (Test): `batch_envelope_raw_bytes_three_events` -- appends 3 events, reads raw bytes, verifies batch header (record_count==3, first_global_pos==0), 3 decodable records, valid batch footer with correct CRC, no extra bytes.
- [x] AC 5 (Test): `batch_envelope_two_consecutive_batches` -- appends 2 events then 1 event, verifies two consecutive batch envelopes with correct record_count (2, 1), first_global_pos (0, 2), and CRCs.
- [x] AC 6 (Test): `batch_envelope_append_returns_correct_recorded_events` -- verifies correct global_position and stream_version values across two batches.
- [x] AC 7 (Test): `batch_envelope_read_all_after_three_event_append` -- verifies `read_all(0, 100)` returns all 3 events in order.
- [x] AC 8: Quality gates pass.

## Test Results
- Lint: PASS (`cargo clippy --all-targets --all-features --locked -- -D warnings` -- zero warnings)
- Tests: PASS (199 passed, 0 failed, 5 ignored)
- Build: PASS (`cargo build` -- zero warnings)
- Format: PASS (`cargo fmt --check` -- clean)
- New tests added:
  - `src/store.rs::tests::batch_envelope_raw_bytes_three_events`
  - `src/store.rs::tests::batch_envelope_two_consecutive_batches`
  - `src/store.rs::tests::batch_envelope_append_returns_correct_recorded_events`
  - `src/store.rs::tests::batch_envelope_read_all_after_three_event_append`

## Concerns / Blockers
- **Out-of-scope file modifications (necessary for quality gates):** The ticket scope says "Modify: `src/store.rs`" only, but 3 tests in other files (`src/writer.rs` and `tests/server_binary.rs`) also fail because they round-trip through `Store::open` after writing batch-envelope data. The ticket explicitly anticipated this ("existing store tests that do round-trip WILL break") and suggested `#[ignore]`. To pass the quality gate ("cargo test must pass"), I added `#[ignore]` with TODO comments to all 5 failing tests (2 in scope, 3 out of scope). This is a minimal, non-functional change. Ticket 3 (recovery loop update) should re-enable all 5 tests.
- **Ignored tests for Ticket 3 to re-enable:**
  - `src/store.rs::tests::recovery_via_append_5_events_across_2_streams`
  - `src/store.rs::tests::recovery_via_append_truncates_garbage_after_real_appends`
  - `src/writer.rs::tests::ac6_durability_survives_restart`
  - `tests/server_binary.rs::ac6_recovery_on_restart`
  - `tests/server_binary.rs::ac7_graceful_shutdown_durability`
