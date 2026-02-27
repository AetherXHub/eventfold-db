# Implementation Report: Ticket 1 -- Batch Envelope Codec -- Types, Constants, Encode/Decode, VERSION Bump

**Ticket:** 1 - Batch Envelope Codec -- Types, Constants, Encode/Decode, VERSION Bump
**Date:** 2026-02-26 12:00
**Status:** COMPLETE

---

## Files Changed

### Created
- None

### Modified
- `src/codec.rs` - Added `BatchHeader`, `BatchFooter` structs; `BATCH_HEADER_MAGIC`, `BATCH_FOOTER_MAGIC` constants; `encode_batch_header`, `decode_batch_header`, `encode_batch_footer`, `decode_batch_footer` functions; made `DecodeOutcome` generic over `T`; bumped `FORMAT_VERSION` from 1 to 2; updated `decode_header` to accept only version 2; added 10 new unit tests; updated existing tests for version 2 and the `value` field name.
- `src/store.rs` - Updated `DecodeOutcome::Complete { event, consumed }` destructuring to `DecodeOutcome::Complete { value: event, consumed }` to match the generic `DecodeOutcome<T>` field rename from `event` to `value`. This was a necessary mechanical change (one line).

## Implementation Notes
- **Generic `DecodeOutcome<T>`**: The ticket's ACs explicitly required `Result<DecodeOutcome<BatchHeader>, Error>` and `Result<DecodeOutcome<BatchFooter>, Error>` return types, which mandated making `DecodeOutcome` generic. The field was renamed from `event` to `value` as specified in the AC (`Complete { value: BatchHeader, consumed: 16 }`). This required a one-line change in `store.rs` to update the destructuring pattern.
- **`store.rs` scope expansion**: The ticket scope listed only `src/codec.rs`, but making `DecodeOutcome` generic required updating `store.rs` (field rename `event` -> `value: event`). This is a purely mechanical change with no behavioral impact. The `lib.rs` re-export `pub use codec::DecodeOutcome` required no change since it re-exports the type by name, and the generic parameter is inferred at each use site.
- **Version bump effect on existing tests**: All existing store tests create fresh temp files via `Store::open`, which calls `encode_header()` (now writes version 2) and `decode_header()` (now accepts version 2). So all store tests continued to pass without any modifications to their logic.
- **Added `BATCH_HEADER_SIZE` and `BATCH_FOOTER_SIZE` constants**: These are `pub(crate)` convenience constants (16 and 8 respectively) to avoid magic numbers in the encode/decode functions and for downstream ticket use.

## Acceptance Criteria
- [x] AC 1: `BATCH_HEADER_MAGIC` is `[0x45, 0x46, 0x42, 0x42]` and `BATCH_FOOTER_MAGIC` is `[0x45, 0x46, 0x42, 0x46]`, declared as `pub(crate) const [u8; 4]` -- Lines 24, 27 of `src/codec.rs`.
- [x] AC 2: `pub struct BatchHeader { pub record_count: u32, pub first_global_pos: u64 }` exists, derives `Debug` and `PartialEq` -- Lines 64-70 of `src/codec.rs`.
- [x] AC 3: `pub struct BatchFooter { pub batch_crc: u32 }` exists, derives `Debug` and `PartialEq` -- Lines 77-81 of `src/codec.rs`.
- [x] AC 4: `pub fn encode_batch_header(record_count: u32, first_global_pos: u64) -> [u8; 16]` -- Line 349 of `src/codec.rs`.
- [x] AC 5: `pub fn decode_batch_header(buf: &[u8]) -> Result<DecodeOutcome<BatchHeader>, Error>` with Incomplete for <16, CorruptRecord for wrong magic, Complete on success -- Lines 373-394 of `src/codec.rs`.
- [x] AC 6: `pub fn encode_batch_footer(batch_crc: u32) -> [u8; 8]` -- Line 408 of `src/codec.rs`.
- [x] AC 7: `pub fn decode_batch_footer(buf: &[u8]) -> Result<DecodeOutcome<BatchFooter>, Error>` with Incomplete for <8, CorruptRecord for wrong magic, Complete on success -- Lines 431-446 of `src/codec.rs`.
- [x] AC 8: `FORMAT_VERSION` constant is `2`. `decode_header` rejects version 1 with `Error::InvalidHeader` containing "version" -- Line 21 (constant), lines 116-129 (decode logic).
- [x] Test AC 8: `encode_batch_header(3, 42)` raw bytes test -- `encode_batch_header_raw_bytes` test.
- [x] Test AC 9: `encode_batch_footer(0xDEAD_BEEF)` raw bytes test -- `encode_batch_footer_raw_bytes` test.
- [x] Test: `decode_batch_header` incomplete (buf < 16) -- `decode_batch_header_incomplete_short_buffer` test.
- [x] Test: `decode_batch_header` wrong magic -- `decode_batch_header_wrong_magic_returns_corrupt` test.
- [x] Test: `encode_batch_header` -> `decode_batch_header` round-trip -- `decode_batch_header_round_trip` test.
- [x] Test: `decode_batch_footer` incomplete (buf < 8) -- `decode_batch_footer_incomplete_short_buffer` test.
- [x] Test: `decode_batch_footer` wrong magic -- `decode_batch_footer_wrong_magic_returns_corrupt` test.
- [x] Test: `encode_batch_footer` -> `decode_batch_footer` round-trip -- `decode_batch_footer_round_trip` test.
- [x] Test: `decode_header` rejects version 1 with "version" in message -- `decode_header_rejects_version_1` test.
- [x] Test: `decode_header` accepts version 2 -- `decode_header_accepts_version_2` test.
- [x] Quality gates: all four pass (build, clippy, fmt, test).

## Test Results
- Lint: PASS (`cargo clippy --all-targets --all-features --locked -- -D warnings` -- zero warnings)
- Tests: PASS (200 tests total: 160 unit + 8 store + 2 reader + 23 integration + 6 server_binary + 1 writer_integration)
- Build: PASS (zero warnings)
- Format: PASS (`cargo fmt --check` clean)
- New tests added: 10 tests in `src/codec.rs::tests` module:
  - `encode_batch_header_raw_bytes`
  - `encode_batch_footer_raw_bytes`
  - `decode_batch_header_incomplete_short_buffer`
  - `decode_batch_header_wrong_magic_returns_corrupt`
  - `decode_batch_header_round_trip`
  - `decode_batch_footer_incomplete_short_buffer`
  - `decode_batch_footer_wrong_magic_returns_corrupt`
  - `decode_batch_footer_round_trip`
  - `decode_header_rejects_version_1`
  - `decode_header_accepts_version_2`

## Concerns / Blockers
- **Out-of-scope file touched**: `src/store.rs` was modified (one line) to update the `DecodeOutcome::Complete` destructuring from `{ event, consumed }` to `{ value: event, consumed }`. This was mechanically required by the generic `DecodeOutcome<T>` change and has zero behavioral impact. The alternative (a separate non-generic enum for batch decode) would have violated the explicit AC signatures.
- The `lib.rs` re-export `pub use codec::DecodeOutcome` now re-exports the generic `DecodeOutcome<T>`. Downstream consumers that previously matched on `DecodeOutcome::Complete { event, .. }` will need to update to `value`. Since this is a library crate used internally, the only consumer is `store.rs` (already updated).
- None of the new types (`BatchHeader`, `BatchFooter`) or functions are re-exported from `lib.rs`. Downstream tickets (Ticket 2, 3) that modify `store.rs` will access them via `crate::codec::` paths, which is the existing pattern.
