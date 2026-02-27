# Code Review: Ticket 2 -- Store::append -- Wrap Each Write in a Batch Envelope

**Ticket:** 2 -- Store::append -- Wrap Each Write in a Batch Envelope
**Impl Report:** prd/008-batch-atomicity-crash-consistency-reports/ticket-02-impl.md
**Date:** 2026-02-26 15:30
**Verdict:** APPROVED

---

## AC Coverage

| AC # | Description | Status | Notes |
|------|-------------|--------|-------|
| 1 | `Store::append` calls `encode_batch_header(count, first_global_pos)` and `encode_batch_footer(batch_crc)` | Met | Lines 426 and 435 of store.rs. `batch_crc` computed via streaming `crc32fast::Hasher` over header + record bytes (lines 429-434). |
| 2 | Buffer is `batch_header \|\| records \|\| batch_footer` in a single contiguous Vec | Met | Lines 438-443: `Vec::with_capacity(BATCH_HEADER_SIZE + encoded_records.len() + BATCH_FOOTER_SIZE)` followed by three `extend_from_slice` calls in correct order. |
| 3 | `sync_all` still called after `write_all` and before index lock | Met | Lines 447-449: `seek` + `write_all` + `sync_all` in Step 4, followed by Step 5's `self.log.write()` at line 453. |
| 4 (Test) | 3-event append raw bytes verify valid BatchHeader, 3 records, valid BatchFooter with correct CRC | Met | Test `batch_envelope_raw_bytes_three_events` (line 1591). Decodes all 16+N+8 bytes, verifies `record_count==3`, `first_global_pos==0`, each record's `global_position`, CRC match, and zero trailing bytes. |
| 5 (Test) | Two consecutive batches have correct record counts and first_global_pos | Met | Test `batch_envelope_two_consecutive_batches` (line 1668). First batch: `record_count==2`, `first_global_pos==0`; second batch: `record_count==1`, `first_global_pos==2`. Both CRCs verified. |
| 6 (Test) | Append returns correct RecordedEvent values | Met | Test `batch_envelope_append_returns_correct_recorded_events` (line 1762). Verifies `global_position`, `stream_version`, `stream_id`, and `event_type` across two batches. |
| 7 (Test) | read_all after append returns events in order | Met | Test `batch_envelope_read_all_after_three_event_append` (line 1808). Verifies 3 events returned with contiguous positions and versions. |
| 8 | Quality gates pass (5 ignored tests expected) | Met | Independently verified: 199 passed, 0 failed, 5 ignored. Clippy clean, fmt clean, build clean. |

## Issues Found

### Critical (must fix before merge)

None.

### Major (should fix, risk of downstream problems)

None.

### Minor (nice to fix, not blocking)

1. **DecodeOutcome field rename handled in store.rs line 153** -- The recovery loop's `Ok(DecodeOutcome::Complete { value: event, consumed })` destructuring at line 153 is technically a Ticket 1 change (the `DecodeOutcome` generic refactor), not a Ticket 2 change. The diff shows this as part of the same working tree. This is an artifact of the co-mingled uncommitted working tree across tickets, not a Ticket 2 scope violation.

## Suggestions (non-blocking)

- The `encoded_records` Vec is allocated with default capacity (`Vec::new()` at line 382). Since the number of proposed events is known and each record has a minimum fixed size, `Vec::with_capacity(proposed_events.len() * estimated_record_size)` could reduce allocations. However, this is a minor optimization and the current approach is idiomatic.

## Scope Check

- Files within scope: YES -- `src/store.rs` is the primary modified file, matching the ticket scope.
- Scope creep detected: MINOR -- `src/writer.rs` and `tests/server_binary.rs` received `#[ignore]` annotations (3 tests total) to make quality gates pass. The impl report explicitly documents and justifies this: these tests round-trip through `Store::open` which cannot parse the new batch envelope format until Ticket 3 updates the recovery loop. The changes are minimal (2 lines each: a TODO comment + `#[ignore]` attribute) and are necessary for the quality gate to pass. This is acceptable given the ticket's own note: "existing store tests that do round-trip WILL break."
- Unauthorized dependencies added: NO

## Risk Assessment

- Regression risk: LOW -- The batch envelope wraps the existing record encoding without modifying it. The in-memory index update logic is unchanged. Five tests that round-trip through `Store::open` are correctly `#[ignore]`d until Ticket 3 updates recovery. All non-ignored tests pass.
- Security concerns: NONE
- Performance concerns: NONE -- The `encoded_batch` Vec is pre-allocated with exact capacity. The streaming CRC32 hasher avoids an extra copy. The single `write_all` call is preserved.
