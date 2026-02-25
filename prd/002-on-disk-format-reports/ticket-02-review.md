# Code Review: Ticket 2 -- Implement and test file header encode/decode

**Ticket:** 2 -- Implement and test file header encode/decode
**Impl Report:** prd/002-on-disk-format-reports/ticket-02-impl.md
**Date:** 2026-02-25 14:30
**Verdict:** APPROVED

---

## AC Coverage

| AC # | Description | Status | Notes |
|------|-------------|--------|-------|
| 1 | `encode_header()` returns `[u8; 8]` with magic at 0..4 and version at 4..8 | Met | Function at line 55 builds a stack-allocated `[u8; 8]`, copies MAGIC into 0..4 and `FORMAT_VERSION.to_le_bytes()` into 4..8. Return type is `[u8; 8]` (compile-time enforced). |
| 2 | `MAGIC` and `FORMAT_VERSION` are private constants with correct values | Met | Lines 15-18: `const MAGIC: [u8; 4] = [0x45, 0x46, 0x44, 0x42]` and `const FORMAT_VERSION: u32 = 1`. No `pub` keyword on either. |
| 3 | Test: `encode_header()` produces exactly 8 bytes | Met | `encode_header_returns_8_bytes` at line 180. Note: since the return type is `[u8; 8]`, this is enforced at compile time, but the test is explicitly required by the AC. |
| 4 | Test: first 4 bytes equal `[0x45, 0x46, 0x44, 0x42]` | Met | `encode_header_first_4_bytes_are_magic` at line 185. Asserts `&header[0..4]` against the expected magic bytes. |
| 5 | Test: bytes 4..8 equal `1u32.to_le_bytes()` | Met | `encode_header_bytes_4_to_8_are_version_1_le` at line 191. Asserts `&header[4..8]` against `&1u32.to_le_bytes()`. |
| 6 | Test: `decode_header(&encode_header())` returns `Ok(1)` | Met | `decode_header_round_trip_returns_version_1` at line 199. Uses `.expect()` then `assert_eq!(version, 1)`. |
| 7 | Test: wrong magic returns `Err(Error::InvalidHeader(msg))` containing "magic" | Met | `decode_header_wrong_magic_returns_error_mentioning_magic` at line 206. Uses the exact input bytes specified in the AC. Pattern matches on `Error::InvalidHeader(msg)` and asserts `msg.contains("magic")`. |
| 8 | Test: unsupported version returns `Err(Error::InvalidHeader(msg))` containing "version" | Met | `decode_header_unsupported_version_returns_error_mentioning_version` at line 221. Constructs buffer with correct magic + version 99. Pattern matches on `Error::InvalidHeader(msg)` and asserts `msg.contains("version")`. |
| 9 | Quality gates pass (build, clippy, fmt, tests) | Met | Verified independently: `cargo test` = 36 passed, `cargo clippy --all-targets --all-features --locked -- -D warnings` = clean, `cargo fmt --check` = clean. |

## Issues Found

### Critical (must fix before merge)
- None

### Major (should fix, risk of downstream problems)
- None

### Minor (nice to fix, not blocking)
- None

## Suggestions (non-blocking)

- The `decode_header` version extraction at line 85 uses individual byte indexing (`[buf[4], buf[5], buf[6], buf[7]]`) instead of `buf[4..8].try_into().expect("...")`. Both are correct; the current form is arguably more explicit. No change needed.

## Scope Check
- Files within scope: YES -- Only `src/codec.rs` was modified for this ticket. Other working tree changes (`Cargo.toml`, `Cargo.lock`, `src/lib.rs`) are from Ticket 1 and were already present before Ticket 2 work began.
- Scope creep detected: NO
- Unauthorized dependencies added: NO

## Risk Assessment
- Regression risk: LOW -- Changes are additive (replacing `unimplemented!()` stubs with logic for two functions). The 3 pre-existing `DecodeOutcome` tests from Ticket 1 are preserved and still pass. The `encode_record`/`decode_record` stubs remain as `unimplemented!()` for Ticket 3.
- Security concerns: NONE
- Performance concerns: NONE -- `encode_header` is stack-only (no heap allocation), `decode_header` only allocates on the error path (format string).
