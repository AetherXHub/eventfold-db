# Implementation Report: Ticket 3 -- Store::open -- Batch-Aware Recovery Loop and Directory Fsync

**Ticket:** 3 - Store::open -- Batch-Aware Recovery Loop and Directory Fsync
**Date:** 2026-02-26 16:45
**Status:** COMPLETE

---

## Files Changed

### Modified
- `src/store.rs` - Rewrote recovery loop to decode batches (header + N records + footer); replaced `has_valid_record_after` with `has_valid_batch_after`; extracted `truncate_and_return` helper; added directory fsync on new-file creation; updated `seed_file` test helper to write batch-format data; updated 2 existing tests for batch semantics; added 6 new tests; removed 2 `#[ignore]` attributes.
- `src/writer.rs` - Removed `#[ignore]` from `ac6_durability_survives_restart` test (1 test).
- `tests/server_binary.rs` - Removed `#[ignore]` from `ac6_recovery_on_restart` and `ac7_graceful_shutdown_durability` tests (2 tests).

## Implementation Notes

- **Recovery loop rewrite**: The recovery loop in `Store::open` now reads one batch at a time: decode `BatchHeader` (16 bytes), then `record_count` records via `decode_record`, then `BatchFooter` (8 bytes). A batch is only committed to the in-memory index if all three steps succeed and the batch CRC matches.
- **`has_valid_batch_after`**: Replaced `has_valid_record_after` with `has_valid_batch_after`, which scans forward for `BATCH_HEADER_MAGIC` instead of individual records. This correctly detects mid-file corruption where a corrupt batch header is followed by a valid batch.
- **`truncate_and_return` helper**: Extracted the truncation logic (set_len + sync_all + build Store) into a helper function to eliminate code duplication across the 5 truncation return paths.
- **Directory fsync**: After `file.sync_all()` on the new-file creation branch, `std::fs::File::open(parent_dir)?.sync_all()?` is called to ensure the directory entry is durable.
- **Updated old tests**: Two existing recovery tests (`recovery_truncates_crc_corrupt_last_record` and `recovery_returns_error_on_mid_file_corruption`) were rewritten to use multi-batch scenarios, since with batch-aware recovery, corruption within a single batch discards the entire batch rather than individual records.
- **Updated `seed_file`**: The test helper now writes events as a single batch envelope (header + records + footer) instead of bare records, matching the v2 on-disk format.

## Acceptance Criteria

- [x] AC 1: Recovery loop reads batches (header -> N records -> footer), commits valid batches to index -- implemented in the main recovery loop.
- [x] AC 2: `decode_batch_header` Incomplete -> truncate to offset, warn, return -- handled in Step 1 of the loop.
- [x] AC 3: `decode_batch_header` CorruptRecord with no valid batch after -> truncate; with valid batch after -> Err(CorruptRecord) -- handled in Step 1 with `has_valid_batch_after`.
- [x] AC 4: Record Incomplete/CorruptRecord within batch -> truncate to batch_start_offset, warn -- handled in Step 2.
- [x] AC 5: `decode_batch_footer` Incomplete or wrong magic -> truncate to batch_start_offset, warn -- handled in Step 3.
- [x] AC 6: Footer CRC mismatch -> truncate to batch_start_offset, warn -- handled in Step 4.
- [x] AC 7: Directory fsync on new-file creation -- `File::open(parent)?.sync_all()?` after `file.sync_all()`.
- [x] AC 8 (Test): `recovery_truncates_batch_missing_footer` -- writes batch, removes footer, asserts 0 events and file truncated to HEADER_SIZE.
- [x] AC 9 (Test): `recovery_truncates_batch_mid_record_truncation` -- writes batch header + 1 full record + 4 bytes of 2nd record, asserts 0 events and file truncated.
- [x] AC 10 (Test): `recovery_two_complete_batches_plus_partial_third` -- 2 complete batches + incomplete 3rd, asserts exactly 3 events from first two batches.
- [x] AC 11 (Test): `open_new_file_dir_fsync_and_reopen` -- opens non-existent path, asserts 0 events, reopens same path, asserts 0 events.
- [x] AC 12 (Test): `recovery_two_complete_batches_all_events_correct` -- 2 manually-constructed batches, asserts read_all returns all events in order.
- [x] AC 13 (Test): `recovery_rejects_version_1_file` -- v1 header file, asserts `Err(InvalidHeader)` with "version" in message.
- [x] Ignored tests: All 5 `#[ignore]` attributes removed; tests pass: `recovery_via_append_5_events_across_2_streams`, `recovery_via_append_truncates_garbage_after_real_appends` (store.rs), `ac6_durability_survives_restart` (writer.rs), `ac6_recovery_on_restart`, `ac7_graceful_shutdown_durability` (server_binary.rs).

## Test Results
- Lint: PASS (`cargo clippy --all-targets --all-features --locked -- -D warnings`)
- Tests: PASS (210 total: 170 lib + 8 main + 2 broker + 23 grpc + 6 server_binary + 1 writer_integration; 0 ignored)
- Build: PASS (`cargo build` with zero warnings)
- Format: PASS (`cargo fmt --check`)
- New tests added:
  - `src/store.rs::tests::recovery_truncates_batch_missing_footer`
  - `src/store.rs::tests::recovery_truncates_batch_mid_record_truncation`
  - `src/store.rs::tests::recovery_two_complete_batches_plus_partial_third`
  - `src/store.rs::tests::recovery_two_complete_batches_all_events_correct`
  - `src/store.rs::tests::open_new_file_dir_fsync_and_reopen`
  - `src/store.rs::tests::recovery_rejects_version_1_file`

## Concerns / Blockers
- None
