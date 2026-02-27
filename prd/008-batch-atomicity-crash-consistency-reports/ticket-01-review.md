# Code Review: Ticket 1 -- Batch Envelope Codec -- Types, Constants, Encode/Decode, VERSION Bump

**Ticket:** 1 -- Batch Envelope Codec -- Types, Constants, Encode/Decode, VERSION Bump
**Impl Report:** prd/008-batch-atomicity-crash-consistency-reports/ticket-01-impl.md
**Date:** 2026-02-26 20:45
**Verdict:** APPROVED

---

## AC Coverage

| AC # | Description | Status | Notes |
|------|-------------|--------|-------|
| 1 | `BATCH_HEADER_MAGIC` is `[0x45, 0x46, 0x42, 0x42]` and `BATCH_FOOTER_MAGIC` is `[0x45, 0x46, 0x42, 0x46]`, `pub(crate) const [u8; 4]` | Met | Lines 24 and 27 of `src/codec.rs`. Values and visibility correct. |
| 2 | `BatchHeader` struct with `record_count: u32`, `first_global_pos: u64`, derives `Debug`, `PartialEq` | Met | Lines 64-70 of `src/codec.rs`. Fields, types, and derives correct. Doc comments present. |
| 3 | `BatchFooter` struct with `batch_crc: u32`, derives `Debug`, `PartialEq` | Met | Lines 77-81 of `src/codec.rs`. Field, type, and derives correct. Doc comment present. |
| 4 | `encode_batch_header(record_count: u32, first_global_pos: u64) -> [u8; 16]` | Met | Line 349 of `src/codec.rs`. Signature matches AC. Function is `pub fn` (superset of `pub(crate)`), consistent with existing `encode_header`/`encode_record` pattern. |
| 5 | `decode_batch_header` returns `Incomplete` for <16, `CorruptRecord` for wrong magic, `Complete` on success | Met | Lines 373-394 of `src/codec.rs`. All three branches verified in code and covered by tests. |
| 6 | `encode_batch_footer(batch_crc: u32) -> [u8; 8]` | Met | Line 408 of `src/codec.rs`. Signature matches AC. |
| 7 | `decode_batch_footer` returns `Incomplete` for <8, `CorruptRecord` for wrong magic, `Complete` on success | Met | Lines 431-446 of `src/codec.rs`. All three branches correct. |
| 8 | `FORMAT_VERSION` is `2`; `decode_header` rejects version 1 with `Error::InvalidHeader` containing "version" | Met | Line 21: `const FORMAT_VERSION: u32 = 2`. Lines 123-127: version mismatch produces `InvalidHeader` with `"unsupported format version: {version}"` which contains "version". Test `decode_header_rejects_version_1` explicitly asserts `msg.contains("version")`. |
| Test AC 8 | `encode_batch_header(3, 42)` raw bytes test | Met | Test `encode_batch_header_raw_bytes` at line 859 verifies exact byte offsets. |
| Test AC 9 | `encode_batch_footer(0xDEAD_BEEF)` raw bytes test | Met | Test `encode_batch_footer_raw_bytes` at line 867 verifies magic and CRC bytes. |
| Test | `decode_batch_header` incomplete (buf < 16) | Met | Test `decode_batch_header_incomplete_short_buffer` at line 874 uses 15-byte buffer. |
| Test | `decode_batch_header` wrong magic | Met | Test `decode_batch_header_wrong_magic_returns_corrupt` at line 880. |
| Test | `encode_batch_header` -> `decode_batch_header` round-trip | Met | Test `decode_batch_header_round_trip` at line 888 verifies `record_count`, `first_global_pos`, and `consumed`. |
| Test | `decode_batch_footer` incomplete (buf < 8) | Met | Test `decode_batch_footer_incomplete_short_buffer` at line 902 uses 7-byte buffer. |
| Test | `decode_batch_footer` wrong magic | Met | Test `decode_batch_footer_wrong_magic_returns_corrupt` at line 908. |
| Test | `encode_batch_footer` -> `decode_batch_footer` round-trip | Met | Test `decode_batch_footer_round_trip` at line 916 verifies `batch_crc` and `consumed`. |
| Test | `decode_header` rejects version 1 with "version" in message | Met | Test `decode_header_rejects_version_1` at line 929 constructs a version-1 header and asserts `msg.contains("version")`. |
| Test | `decode_header` with version 2 returns `Ok(2)` | Met | Test `decode_header_accepts_version_2` at line 947. |
| QG | Quality gates pass | Met | Independently verified: `cargo build` (zero warnings), `cargo clippy --all-targets --all-features --locked -- -D warnings` (clean), `cargo fmt --check` (clean), `cargo test` (200 passed, 0 failed). |

## Issues Found

### Critical (must fix before merge)

None.

### Major (should fix, risk of downstream problems)

None.

### Minor (nice to fix, not blocking)

1. **`BATCH_HEADER_SIZE` / `BATCH_FOOTER_SIZE` not mentioned in AC but added** (lines 331, 334 of `src/codec.rs`). These are `pub(crate) const` convenience constants. They are not listed in the ticket's ACs but are reasonable forward-looking additions for Tickets 2 and 3. Acceptable as minimal scope creep with clear downstream value.

2. **`DecodeOutcome` generic field name `value` could include a note about the rename.** The field was renamed from `event` to `value` (line 51), which is a breaking change to the public API. The crate is internal-only so this is low-risk, but adding a `// renamed from `event` in FORMAT_VERSION 2` comment would help future readers. Very minor.

## Suggestions (non-blocking)

- The `decode_batch_header` and `decode_batch_footer` functions hardcode `position: 0` in the `CorruptRecord` error variants (lines 379-381, 436-438). In later tickets when these functions are called from the recovery loop, the caller will need to override the position. Consider accepting an `offset` parameter for the error position, or document that callers should map the position. This is a Ticket 3 concern, not blocking here.

- Existing tests `decode_outcome_complete_is_constructible`, `decode_outcome_incomplete_is_constructible`, and `decode_outcome_debug_is_non_empty` (lines 453-484) were correctly updated for the generic `DecodeOutcome<T>` (adding type annotations and `value:` field name). Good mechanical update.

## Scope Check

- Files within scope: YES
  - `src/codec.rs` -- primary target, correctly modified
  - `src/store.rs` -- single-line mechanical change (`event` -> `value: event`) required by the `DecodeOutcome` generification. Documented in impl report. Acceptable.
- Out-of-scope files in working tree: `docs/design.md` and `prd/008-batch-atomicity-crash-consistency.md` also have uncommitted changes, but these belong to Ticket 4 (documentation) and the orchestrator (PRD status update) respectively. They are not attributable to the Ticket 1 implementer. No scope violation.
- Scope creep detected: NO (the `BATCH_HEADER_SIZE`/`BATCH_FOOTER_SIZE` constants are minimal and directly useful for downstream tickets)
- Unauthorized dependencies added: NO

## Risk Assessment

- Regression risk: LOW -- The `DecodeOutcome` generification is a straightforward type-level change. All 200 existing tests pass. The `store.rs` field rename is mechanical (binding rename, not semantic). The `FORMAT_VERSION` bump from 1 to 2 means existing v1 data files will be rejected, which is the intended behavior per the PRD.
- Security concerns: NONE
- Performance concerns: NONE -- All encode/decode functions are zero-allocation fixed-size operations (array returns, no heap allocation). The `DecodeOutcome<T>` generic has no runtime cost.
