# Implementation Report: Ticket 3 -- Implement and test record encode/decode (CRC32, partial detection, round-trips)

**Ticket:** 3 - Implement and test record encode/decode
**Date:** 2026-02-25 14:30
**Status:** COMPLETE

---

## Files Changed

### Created
- None

### Modified
- `src/codec.rs` - Implemented `encode_record()` and `decode_record()`, added 14 new tests covering AC-3 through AC-9, added two private constants (`FIXED_BODY_SIZE`, `LENGTH_PREFIX_SIZE`), added imports for `bytes::Bytes` and `uuid::Uuid`.

## Implementation Notes
- **Encode layout**: `record_length` (u32 LE) followed by body fields in exact PRD order, terminated by CRC32 (u32 LE). `record_length` stores byte count from `global_position` through `checksum` inclusive.
- **CRC32 scope**: Computed over bytes from `global_position` through end of `payload` (everything after `record_length` and before `checksum`). Uses `crc32fast::hash()`.
- **Decode uses a cursor-based macro** (`read_bytes!`) for sequential field extraction from the CRC-protected region. The macro bounds-checks each read and returns `CorruptRecord` on overrun.
- **UTF-8 validation** happens after CRC verification, so a corrupt event type that also has a bad checksum will report as CRC mismatch (not UTF-8 error). This is the correct priority since CRC failure means the data is untrustworthy.
- **`Vec::with_capacity()`** used in encode to avoid reallocation -- total size is known upfront from fixed overhead + variable field lengths.
- **`Bytes::copy_from_slice()`** used in decode to construct owned `Bytes` from borrowed slices of the input buffer.
- **AC-9 test technique**: Encodes a valid record, injects invalid UTF-8 bytes into the event type region at the known offset (54), then recomputes the CRC32 so the checksum is valid but the UTF-8 is not. This isolates the UTF-8 validation path from the CRC validation path.

## Acceptance Criteria
- [x] AC (encode field order): `encode_record` writes all fields in the specified exact order (record_length, global_position, stream_id, stream_version, event_id, event_type_len, event_type, metadata_len, metadata, payload_len, payload, checksum). Verified by AC-8 test which inspects byte offsets.
- [x] AC (record_length semantics): `record_length` stores byte count from `global_position` through `checksum` inclusive (total - 4). Verified by AC-8 assertion.
- [x] AC (CRC32 scope): CRC32 computed over bytes from `global_position` through `payload`. Verified by AC-8 test that independently computes CRC32 and compares.
- [x] AC (incomplete < 4 bytes): `decode_record` returns `Ok(Incomplete)` when `buf.len() < 4`. Verified by AC-6a test.
- [x] AC (incomplete truncated body): `decode_record` returns `Ok(Incomplete)` when `buf.len() < 4 + record_length`. Verified by AC-6b test.
- [x] AC (CRC mismatch error): Returns `Err(CorruptRecord { position: 0, .. })` on CRC mismatch. Verified by AC-5a, AC-5b, AC-5c tests.
- [x] AC-3a: Round-trip with non-empty metadata and payload -- all 7 fields match.
- [x] AC-3b: Round-trip with empty metadata and payload -- all fields match.
- [x] AC-3c: Round-trip with 256-byte event type -- all fields match.
- [x] AC-3d: Round-trip with `\x00\xff\x00\xff` binary data -- all fields match.
- [x] AC-4: Encoding same event twice produces identical byte sequences.
- [x] AC-5a: Flipped bit in payload region (offset `buf.len() - 5`) returns `CorruptRecord`.
- [x] AC-5b: Flipped bit at byte offset 8 (stream_id region) returns `CorruptRecord`.
- [x] AC-5c: Flipped bit in checksum (last 4 bytes) returns `CorruptRecord`.
- [x] AC-6a: 2-byte buffer returns `Incomplete`.
- [x] AC-6b: 10-byte buffer with record_length=1000 returns `Incomplete`.
- [x] AC-6c: Valid record + 3 extra bytes returns `Complete` with `consumed` = encoded record length (not total buffer).
- [x] AC-7: 3 concatenated records decoded sequentially, all match, consumed sums to buffer length.
- [x] AC-8: Known global_position at bytes 4..12, record_length = total - 4, CRC32 at last 4 bytes matches body hash.
- [x] AC-9: Invalid UTF-8 in event type region (with valid CRC32) returns `CorruptRecord`.
- [x] Quality gates pass (build, clippy, fmt, tests).

## Test Results
- Lint: PASS (`cargo clippy --all-targets --all-features --locked -- -D warnings`)
- Tests: PASS (50 total: 36 pre-existing + 14 new codec tests)
- Build: PASS (zero warnings)
- Format: PASS (`cargo fmt --check`)
- New tests added:
  - `src/codec.rs::tests::ac3a_round_trip_non_empty_metadata_and_payload`
  - `src/codec.rs::tests::ac3b_round_trip_empty_metadata_and_payload`
  - `src/codec.rs::tests::ac3c_round_trip_max_length_event_type`
  - `src/codec.rs::tests::ac3d_round_trip_binary_data_with_null_bytes`
  - `src/codec.rs::tests::ac4_encode_determinism`
  - `src/codec.rs::tests::ac5a_crc_mismatch_flipped_payload_bit`
  - `src/codec.rs::tests::ac5b_crc_mismatch_flipped_stream_id_bit`
  - `src/codec.rs::tests::ac5c_crc_mismatch_flipped_checksum_bit`
  - `src/codec.rs::tests::ac6a_incomplete_2_byte_buffer`
  - `src/codec.rs::tests::ac6b_incomplete_large_length_small_buffer`
  - `src/codec.rs::tests::ac6c_extra_trailing_bytes_consumed_correctly`
  - `src/codec.rs::tests::ac7_three_records_sequential_decode`
  - `src/codec.rs::tests::ac8_field_boundary_correctness`
  - `src/codec.rs::tests::ac9_invalid_utf8_event_type`

## Concerns / Blockers
- None
