# Code Review: Ticket 3 -- Implement and test record encode/decode (CRC32, partial detection, round-trips)

**Ticket:** 3 -- Implement and test record encode/decode
**Impl Report:** prd/002-on-disk-format-reports/ticket-03-impl.md
**Date:** 2026-02-25 15:45
**Verdict:** APPROVED

---

## AC Coverage

| AC # | Description | Status | Notes |
|------|-------------|--------|-------|
| encode field order | Fields written in exact specified order | Met | Verified line-by-line in `encode_record` (lines 126-143): record_length, global_position, stream_id, stream_version, event_id, event_type_len, event_type, metadata_len, metadata, payload_len, payload, checksum. All LE encodings and UUID raw bytes correct. |
| record_length semantics | Stores byte count from global_position through checksum (total - 4) | Met | `body_len` = `FIXED_BODY_SIZE + variable fields` where FIXED_BODY_SIZE=62 includes the 4-byte checksum. Line 126 writes `body_len as u32`. AC-8 test independently verifies `record_length == (buf.len() - 4) as u32`. |
| CRC32 scope | CRC covers global_position through payload | Met | Line 142: `crc32fast::hash(&buf[LENGTH_PREFIX_SIZE..])` at the point where buf contains record_length through payload (checksum not yet appended). Decode line 204: `crc32fast::hash(&body[..crc_offset])` where crc_offset = body.len()-4. Both correctly exclude record_length and checksum. AC-8 test independently verifies. |
| incomplete < 4 bytes | Returns `Ok(Incomplete)` when `buf.len() < 4` | Met | Line 174: `if buf.len() < LENGTH_PREFIX_SIZE` where LENGTH_PREFIX_SIZE=4. AC-6a test passes 2-byte buffer. |
| incomplete truncated body | Returns `Ok(Incomplete)` when `buf.len() < 4 + record_length` | Met | Lines 178-183: reads record_length, computes `total = 4 + record_length`, checks `buf.len() < total`. AC-6b test passes 10-byte buffer with record_length=1000. |
| CRC mismatch error | Returns `Err(CorruptRecord { position: 0, .. })` on mismatch | Met | Lines 206-213: compares stored_crc vs computed_crc, returns `CorruptRecord { position: 0, .. }` with hex-formatted detail. Tested by AC-5a, AC-5b, AC-5c. |
| AC-3a | Round-trip with non-empty metadata and payload | Met | Test `ac3a_round_trip_non_empty_metadata_and_payload` uses `make_event(0, 0, "OrderPlaced", b"meta-data", b"{\"qty\":1}")`, encodes, decodes, asserts `decoded == event` and `consumed == buf.len()`. |
| AC-3b | Round-trip with empty metadata and payload | Met | Test `ac3b_round_trip_empty_metadata_and_payload` uses `b""` for both, asserts equality. |
| AC-3c | Round-trip with 256-byte event type | Met | Test `ac3c_round_trip_max_length_event_type` uses `"A".repeat(256)`, asserts equality. |
| AC-3d | Round-trip with `\x00\xff\x00\xff` binary data | Met | Test `ac3d_round_trip_binary_data_with_null_bytes` passes `b"\x00\xff\x00\xff"` as both metadata and payload. |
| AC-4 | Encoding determinism | Met | Test `ac4_encode_determinism` encodes same event twice, asserts `buf1 == buf2`. |
| AC-5a | Flipped bit in payload region | Met | Test `ac5a_crc_mismatch_flipped_payload_bit` flips at `buf.len() - 5`, asserts `CorruptRecord`. |
| AC-5b | Flipped bit at byte offset 8 | Met | Test `ac5b_crc_mismatch_flipped_stream_id_bit` flips `buf[8]`. Note: offset 8 is actually within global_position (bytes 4..12), not stream_id (starts at 12). The ticket AC text says "byte offset 8" and the test does exactly that. The parenthetical label is imprecise but the test fulfills the requirement. |
| AC-5c | Flipped bit in checksum | Met | Test `ac5c_crc_mismatch_flipped_checksum_bit` flips last byte of buffer. |
| AC-6a | 2-byte buffer incomplete | Met | Test `ac6a_incomplete_2_byte_buffer` passes `[0x00, 0x01]`, asserts Incomplete. |
| AC-6b | Length prefix present, truncated body | Met | Test `ac6b_incomplete_large_length_small_buffer` uses 10-byte buffer with record_length=1000. |
| AC-6c | Valid record + 3 extra bytes | Met | Test `ac6c_extra_trailing_bytes_consumed_correctly` appends 3 bytes, asserts consumed equals original encoded length. |
| AC-7 | 3 concatenated records decoded sequentially | Met | Test `ac7_three_records_sequential_decode` encodes 3 events, concatenates, decodes sequentially, asserts all match and consumed sums to total. |
| AC-8 | Field boundary correctness | Met | Test `ac8_field_boundary_correctness` verifies global_position at bytes 4..12, record_length = total - 4, and CRC matches independently computed hash. |
| AC-9 | Invalid UTF-8 returns CorruptRecord | Met | Test `ac9_invalid_utf8_event_type` injects `[0xFF, 0xFE]` at offset 54 (event_type region), recomputes CRC, asserts CorruptRecord. Correctly isolates UTF-8 path from CRC path. |
| Quality gates | Build, clippy, fmt, tests all pass | Met | Independently verified: `cargo test` (50 passed), `cargo clippy --all-targets --all-features --locked -- -D warnings` (clean), `cargo fmt --check` (clean), `cargo build` (zero warnings). |

## Issues Found

### Critical (must fix before merge)
- None

### Major (should fix, risk of downstream problems)
- None

### Minor (nice to fix, not blocking)
- **AC-5b test comment accuracy** (`src/codec.rs` line 463): The comment says "Byte offset 8 is inside global_position/stream_id region" -- this is slightly misleading. Byte offset 8 is `buf[8]`, which is at index 4 within the global_position field (bytes 4..12 of the full buffer). Stream_id starts at byte 12. The slash notation is a reasonable hedge, but strictly speaking it is only in the global_position field. Not a correctness issue -- the CRC integrity check works the same regardless of which field the bit flip targets.

## Suggestions (non-blocking)
- The `read_bytes!` macro (lines 222-234) is a clean solution for cursor-based parsing. If future modules need similar parsing, consider extracting it into a shared utility. For now, keeping it scoped to `decode_record` is appropriate.
- The `.expect()` messages on `try_into()` calls (e.g., line 238: `"8 bytes for u64"`) are correct invariant-violation expectations since `read_bytes!` guarantees the exact size. No change needed.
- The `body.len() < 4` guard at line 191 handles the edge case of record_length being 0-3. This is good defensive coding for malformed data.

## Scope Check
- Files within scope: YES -- only `src/codec.rs` was modified (new code added to the existing file from Tickets 1 and 2).
- Scope creep detected: NO -- the changes to `Cargo.toml`, `Cargo.lock`, `src/lib.rs` are from Tickets 1 and 2, not this ticket. The working tree contains uncommitted code from all prior tickets in this PRD, which is the established pattern for this project.
- Unauthorized dependencies added: NO

## Risk Assessment
- Regression risk: LOW -- All 50 tests pass (36 pre-existing + 14 new). The encode/decode functions are pure data transformations with no side effects.
- Security concerns: NONE -- The codec is a pure serialization layer with no I/O, no network access, and no secrets.
- Performance concerns: NONE -- `Vec::with_capacity` used in encode to avoid reallocation. `crc32fast::hash` is a single-pass computation. Decode uses zero-copy slicing from the input buffer with a single allocation for the `RecordedEvent` fields.
