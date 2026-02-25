# Implementation Report: Ticket 2 -- Implement and test file header encode/decode

**Ticket:** 2 - Implement and test file header encode/decode
**Date:** 2026-02-25 12:00
**Status:** COMPLETE

---

## Files Changed

### Created
- None

### Modified
- `src/codec.rs` - Added private constants `MAGIC` and `FORMAT_VERSION`; implemented `encode_header()` and `decode_header()`; added 6 unit tests covering AC-1 and AC-2.

## Implementation Notes
- `MAGIC` is `[u8; 4] = [0x45, 0x46, 0x44, 0x42]` (ASCII "EFDB"), defined as a private module-level constant.
- `FORMAT_VERSION` is `u32 = 1`, defined as a private module-level constant.
- `encode_header()` writes magic into bytes 0..4 and `FORMAT_VERSION.to_le_bytes()` into bytes 4..8 of a stack-allocated `[u8; 8]`.
- `decode_header()` validates magic first (returning `InvalidHeader` with "magic" in the message), then reads the version as `u32` little-endian and rejects anything other than `FORMAT_VERSION` (returning `InvalidHeader` with "version" in the message).
- Both `encode_record` and `decode_record` remain as `unimplemented!()` stubs from Ticket 1 -- they are out of scope for this ticket.
- The existing 3 `DecodeOutcome` tests from Ticket 1 were preserved unchanged.

## Acceptance Criteria
- [x] AC 1: `encode_header()` returns `[u8; 8]` with magic at bytes 0..4 and version at bytes 4..8 - Implemented directly.
- [x] AC 2: `MAGIC: [u8; 4] = [0x45, 0x46, 0x44, 0x42]` and `FORMAT_VERSION: u32 = 1` defined as private constants - Defined at module level without `pub`.
- [x] AC 3: Test `encode_header()` produces exactly 8 bytes - `encode_header_returns_8_bytes`
- [x] AC 4: Test first 4 bytes equal `[0x45, 0x46, 0x44, 0x42]` - `encode_header_first_4_bytes_are_magic`
- [x] AC 5: Test bytes 4..8 equal `1u32.to_le_bytes()` - `encode_header_bytes_4_to_8_are_version_1_le`
- [x] AC 6: Test `decode_header(&encode_header())` returns `Ok(1)` - `decode_header_round_trip_returns_version_1`
- [x] AC 7: Test wrong magic returns `Err(Error::InvalidHeader(msg))` where msg contains "magic" - `decode_header_wrong_magic_returns_error_mentioning_magic`
- [x] AC 8: Test unsupported version returns `Err(Error::InvalidHeader(msg))` where msg contains "version" - `decode_header_unsupported_version_returns_error_mentioning_version`
- [x] AC 9: Quality gates pass (build, clippy, fmt, tests) - All four pass clean.

## Test Results
- Lint: PASS (`cargo clippy --all-targets --all-features --locked -- -D warnings` -- zero warnings)
- Tests: PASS (36 total: 30 existing + 6 new, all green)
- Build: PASS (`cargo build` -- zero warnings)
- Fmt: PASS (`cargo fmt --check` -- clean)
- New tests added:
  - `src/codec.rs::tests::encode_header_returns_8_bytes`
  - `src/codec.rs::tests::encode_header_first_4_bytes_are_magic`
  - `src/codec.rs::tests::encode_header_bytes_4_to_8_are_version_1_le`
  - `src/codec.rs::tests::decode_header_round_trip_returns_version_1`
  - `src/codec.rs::tests::decode_header_wrong_magic_returns_error_mentioning_magic`
  - `src/codec.rs::tests::decode_header_unsupported_version_returns_error_mentioning_version`

## Concerns / Blockers
- None
