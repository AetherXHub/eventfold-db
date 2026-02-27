# Code Review: Ticket 3 -- Store::open -- Batch-Aware Recovery Loop and Directory Fsync

**Ticket:** 3 -- Store::open -- Batch-Aware Recovery Loop and Directory Fsync
**Impl Report:** prd/008-batch-atomicity-crash-consistency-reports/ticket-03-impl.md
**Date:** 2026-02-26 18:30
**Verdict:** APPROVED

---

## AC Coverage

| AC # | Description | Status | Notes |
|------|-------------|--------|-------|
| 1 | Recovery loop reads batches (header -> N records -> footer), commits valid batches to index | Met | Lines 189-307 of `src/store.rs`: loop decodes BatchHeader (Step 1), N records (Step 2), BatchFooter (Step 3), verifies CRC (Step 4), commits to index (Step 5). Only fully validated batches are committed. |
| 2 | Incomplete batch header at tail -> truncate to offset, warn, return prior batches | Met | Lines 203-212: `DecodeOutcome::Incomplete` from `decode_batch_header` triggers `tracing::warn!` and `truncate_and_return(path, batch_start_offset, ...)`. |
| 3 | Corrupt batch header + no valid batch follows -> truncate; if valid data follows -> Error::CorruptRecord | Met | Lines 213-231: `Err(CorruptRecord)` from `decode_batch_header` calls `has_valid_batch_after`. If true, returns `Err(CorruptRecord)` with detail. If false, truncates. |
| 4 | Incomplete/corrupt record within batch -> truncate to batch_start_offset, warn | Met | Lines 243-253: Both `Incomplete` and `CorruptRecord` from `decode_record` trigger truncation to `batch_start_offset`. |
| 5 | Incomplete/corrupt batch footer -> truncate to batch_start_offset, warn | Met | Lines 264-283: `Incomplete` and `CorruptRecord` from `decode_batch_footer` both truncate to `batch_start_offset`. |
| 6 | Footer CRC mismatch -> truncate to batch_start_offset, warn | Met | Lines 288-298: CRC recomputed over `&data[batch_start_offset..offset - BATCH_FOOTER_SIZE]` (header + records), compared to `footer.batch_crc`. Mismatch triggers truncation. |
| 7 | New-file creation branch calls directory fsync | Met | Lines 149-156: `File::open(parent)?.sync_all()?` called after `file.sync_all()`. Uses `.expect()` for invariant (path must have parent). |
| 8 (Test AC-2) | Remove footer -> 0 events, truncated to file header offset | Met | Test `recovery_truncates_batch_missing_footer` (lines 1979-2014): writes 2-event batch, removes last 8 bytes (footer), asserts 0 events and file truncated to `HEADER_SIZE`. |
| 9 (Test AC-3) | Mid-record truncation -> 0 events, truncated | Met | Test `recovery_truncates_batch_mid_record_truncation` (lines 2020-2058): writes batch header + 1 full record + 4 bytes of 2nd record, asserts 0 events and file truncated to `HEADER_SIZE`. |
| 10 (Test AC-4) | Two complete + one partial batch -> only first two batches recovered | Met | Test `recovery_two_complete_batches_plus_partial_third` (lines 2064-2110): 2 complete batches + incomplete 3rd (1 of 2 records, no footer), asserts 3 events from first two batches. |
| 11 (Test AC-6) | Store::open on non-existent path -> empty store, second open also empty | Met | Test `open_new_file_dir_fsync_and_reopen` (lines 1957-1973): creates store at new path, verifies 0 events, drops, reopens, verifies 0 events again. |
| 12 (Test AC-7) | Two complete batches -> all events recovered in order, no gaps | Met | Test `recovery_two_complete_batches_all_events_correct` (lines 2115-2156): 2 batches (2 events stream_a, 2 events stream_b), verifies 4 events in global-position order with correct event types and stream versions. |
| 13 (Test AC-5) | Version 1 file -> Err(InvalidHeader) mentioning "version" | Met | Test `recovery_rejects_version_1_file` (lines 2160-2179): writes old FORMAT_VERSION=1 header, asserts `Err(InvalidHeader)` with `msg.contains("version")`. |
| 14 | All 5 previously-ignored tests re-enabled and passing | Met | Verified: `grep -r "#[ignore]"` returns zero results across `src/` and `tests/`. All 210 tests pass with 0 ignored. The 5 tests (`recovery_via_append_5_events_across_2_streams`, `recovery_via_append_truncates_garbage_after_real_appends` in store.rs; `ac6_durability_survives_restart` in writer.rs; `ac6_recovery_on_restart`, `ac7_graceful_shutdown_durability` in server_binary.rs) are present and passing. |
| 15 | Quality gates pass (210 tests, 0 ignored) | Met | Confirmed: `cargo test` = 210 passed, 0 failed, 0 ignored. `cargo clippy` = clean. `cargo fmt --check` = clean. `cargo build` = zero warnings. |

## Issues Found

### Critical (must fix before merge)

None.

### Major (should fix, risk of downstream problems)

None.

### Minor (nice to fix, not blocking)

None.

## Suggestions (non-blocking)

1. **`has_valid_batch_after` is O(n) per byte from start to end of buffer.** For files with many valid batches followed by a single corrupt batch header at the tail, this scans the entire remaining buffer byte-by-byte. In practice, CRM-scale event stores are small enough that this is negligible, but for very large log files (hundreds of MB) this could be slow. An optimization would be to scan for the 4-byte magic pattern using `memchr` or `windows(4)`, but this is not needed now.

2. **`truncate_and_return` re-opens the file** (line 65: `OpenOptions::new().read(true).write(true).open(path)?`) even though `Store::open` already read the file into `data` via `std::fs::read(path)`. The original file was opened for reading only. This design is correct -- the truncation path needs write access. No change needed.

3. **Two batch helpers in test code (`seed_file` and `seed_batch_file`):** `seed_file` wraps all events in a single batch; `seed_batch_file` takes `&[&[RecordedEvent]]` for multi-batch files. Both are useful. The naming is clear. No action needed.

## Scope Check

- Files within scope: YES -- Only `src/store.rs` has actual changes. `src/writer.rs` and `tests/server_binary.rs` were touched (removing `#[ignore]` added by Ticket 2), but since all tickets are in the same uncommitted working tree, the net diff is zero for those files. This is correct behavior.
- Scope creep detected: NO
- Unauthorized dependencies added: NO

## Risk Assessment

- Regression risk: LOW -- The recovery loop is a complete rewrite from record-level to batch-level decoding, but it is thoroughly tested (6 new tests, 2 updated tests, 5 re-enabled tests, plus the existing gRPC integration tests that exercise the full append-recover-read cycle). All 210 tests pass.
- Security concerns: NONE
- Performance concerns: NONE -- The `has_valid_batch_after` scan is O(n) but only executes on the error path (corrupt batch header), which should never happen during normal operation. The recovery loop itself is linear in file size with no extra allocations beyond the event vectors.
