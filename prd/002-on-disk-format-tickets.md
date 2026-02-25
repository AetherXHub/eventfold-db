# Tickets for PRD 002: On-Disk Format (Record Codec)

**Source PRD:** prd/002-on-disk-format.md
**Created:** 2026-02-25
**Total Tickets:** 4
**Estimated Total Complexity:** 7 (S=1 + M=2 + L=3 + S=1)

---

### Ticket 1: Scaffold `codec.rs` — dependency, module registration, and `DecodeOutcome` type

**Description:**
Add `crc32fast = "1"` to `Cargo.toml`, create `src/codec.rs` with stub function signatures
and the `DecodeOutcome` return type that distinguishes "not enough bytes" from a fully decoded
record, and register `pub mod codec` in `src/lib.rs`. This is the foundational scaffold all
subsequent codec tests and implementations depend on.

**Scope:**
- Modify: `Cargo.toml`
- Create: `src/codec.rs` (stubs + `DecodeOutcome` type)
- Modify: `src/lib.rs` (add `pub mod codec;`)

**Acceptance Criteria:**
- [ ] `Cargo.toml` contains `crc32fast = "1"` under `[dependencies]`.
- [ ] `src/codec.rs` defines a public `DecodeOutcome` enum with at least two variants:
      `Complete { event: RecordedEvent, consumed: usize }` and `Incomplete`.
      The enum derives `Debug`.
- [ ] `src/codec.rs` declares stub `pub fn encode_header() -> [u8; 8]` that panics with
      `unimplemented!()`.
- [ ] `src/codec.rs` declares stub `pub fn decode_header(buf: &[u8; 8]) -> Result<u32, Error>`
      that panics with `unimplemented!()`.
- [ ] `src/codec.rs` declares stub `pub fn encode_record(event: &RecordedEvent) -> Vec<u8>`
      that panics with `unimplemented!()`.
- [ ] `src/codec.rs` declares stub `pub fn decode_record(buf: &[u8]) -> Result<DecodeOutcome, Error>`
      that panics with `unimplemented!()`.
- [ ] `src/lib.rs` contains `pub mod codec;` and re-exports `codec::DecodeOutcome`.
- [ ] All public items in `codec.rs` have doc comments.
- [ ] Test: `cargo build` compiles with zero warnings (stubs accepted by the compiler).
- [ ] Quality gates pass (build, clippy, fmt).

**Dependencies:** None (PRD 001 already complete)
**Complexity:** S
**Maps to PRD AC:** AC-6 (partial/complete distinction type), AC-10 (build and lint)

---

### Ticket 2: Implement and test file header encode/decode

**Description:**
Implement `encode_header()` and `decode_header()` in `src/codec.rs` with their full logic:
write the 4-byte magic `0x45464442` and 4-byte version `1u32` in little-endian; on decode,
reject wrong magic or unsupported version. Write all AC-1 and AC-2 tests first (red), then
make them green.

**Scope:**
- Modify: `src/codec.rs` (implement `encode_header`, `decode_header`, add `#[cfg(test)]` module)

**Acceptance Criteria:**
- [ ] `encode_header()` returns `[u8; 8]` with magic at bytes 0..4 and version at bytes 4..8.
- [ ] Magic constant `MAGIC: [u8; 4] = [0x45, 0x46, 0x44, 0x42]` (ASCII "EFDB") and
      `FORMAT_VERSION: u32 = 1` are defined as private constants.
- [ ] Test: `encode_header()` produces exactly 8 bytes — `assert_eq!(encode_header().len(), 8)`.
- [ ] Test: first 4 bytes of `encode_header()` equal `[0x45, 0x46, 0x44, 0x42]`.
- [ ] Test: bytes 4..8 of `encode_header()` equal `1u32.to_le_bytes()`.
- [ ] Test: `decode_header(&encode_header())` returns `Ok(1)`.
- [ ] Test: `decode_header` with wrong magic (e.g., `[0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00]`)
      returns `Err(Error::InvalidHeader(msg))` where `msg` contains "magic".
- [ ] Test: `decode_header` with correct magic but unsupported version (e.g., version field = 99)
      returns `Err(Error::InvalidHeader(msg))` where `msg` contains "version".
- [ ] Quality gates pass (build, clippy, fmt, tests).

**Dependencies:** Ticket 1
**Complexity:** M
**Maps to PRD AC:** AC-1, AC-2

---

### Ticket 3: Implement and test record encode/decode (CRC32, partial detection, round-trips)

**Description:**
Implement `encode_record()` and `decode_record()` in `src/codec.rs`. This is the core of the
codec: serialize all fields in the specified byte layout with a CRC32 checksum over the record
body; on decode, detect incomplete buffers (return `DecodeOutcome::Incomplete`), verify the
CRC32, parse all fields including UTF-8 validation of the event type, and return
`DecodeOutcome::Complete { event, consumed }`. Write all AC-3 through AC-9 tests first (red),
then implement.

**Scope:**
- Modify: `src/codec.rs` (implement `encode_record`, `decode_record`, expand `#[cfg(test)]`)

**Acceptance Criteria:**
- [ ] `encode_record` writes fields in this exact order: `record_length` (u32 LE, 4 bytes),
      `global_position` (u64 LE, 8 bytes), `stream_id` (UUID raw bytes, 16 bytes),
      `stream_version` (u64 LE, 8 bytes), `event_id` (UUID raw bytes, 16 bytes),
      `event_type_len` (u16 LE, 2 bytes), `event_type` (UTF-8 bytes),
      `metadata_len` (u32 LE, 4 bytes), `metadata` (raw bytes),
      `payload_len` (u32 LE, 4 bytes), `payload` (raw bytes), `checksum` (CRC32 LE, 4 bytes).
- [ ] `record_length` stores the byte count from `global_position` through `checksum` inclusive
      (i.e., total encoded length minus 4 for the `record_length` field itself).
- [ ] The CRC32 checksum is computed over all bytes from `global_position` through `payload`
      inclusive (everything except `record_length` and `checksum`).
- [ ] `decode_record` returns `Ok(DecodeOutcome::Incomplete)` when `buf.len() < 4`
      (cannot read length prefix).
- [ ] `decode_record` returns `Ok(DecodeOutcome::Incomplete)` when `buf.len() < 4 + record_length`
      (length prefix present but record body is truncated).
- [ ] `decode_record` returns `Err(Error::CorruptRecord { position: 0, detail: _ })` on CRC mismatch.
- [ ] Test (AC-3a): encode a `RecordedEvent` with non-empty metadata and payload, decode it,
      assert all 7 fields match the original.
- [ ] Test (AC-3b): round-trip with `metadata = Bytes::new()` and `payload = Bytes::new()` —
      all fields match.
- [ ] Test (AC-3c): round-trip with `event_type` of exactly 256 bytes — all fields match.
- [ ] Test (AC-3d): round-trip with metadata and payload containing `\x00\xff\x00\xff` binary
      data — all fields match (null bytes do not truncate).
- [ ] Test (AC-4): encode the same `RecordedEvent` twice, assert the two `Vec<u8>` are equal.
- [ ] Test (AC-5a): encode a record, flip one bit inside the payload region (byte at offset
      `buf.len() - 5` which is inside payload before checksum), call `decode_record` —
      returns `Err(Error::CorruptRecord { .. })`.
- [ ] Test (AC-5b): encode a record, flip one bit at byte offset 8 (inside the `stream_id`
      region), call `decode_record` — returns `Err(Error::CorruptRecord { .. })`.
- [ ] Test (AC-5c): encode a record, flip one bit in the checksum (last 4 bytes of buffer),
      call `decode_record` — returns `Err(Error::CorruptRecord { .. })`.
- [ ] Test (AC-6a): call `decode_record(&[0x00, 0x01])` (2-byte buffer) —
      returns `Ok(DecodeOutcome::Incomplete)`.
- [ ] Test (AC-6b): build a buffer where the first 4 bytes encode a large `record_length`
      (e.g., 1000) but the total buffer is only 10 bytes — returns `Ok(DecodeOutcome::Incomplete)`.
- [ ] Test (AC-6c): encode a valid record, append 3 extra bytes, call `decode_record` —
      returns `Ok(DecodeOutcome::Complete { consumed, .. })` where `consumed` equals the length of
      the encoded record alone (not `len() - 3`), confirming the caller can slice from `consumed`.
- [ ] Test (AC-7): encode 3 distinct `RecordedEvent`s into a single concatenated buffer;
      call `decode_record` three times advancing by `consumed` each time; assert all 3 events
      match originals and `consumed_1 + consumed_2 + consumed_3 == buf.len()`.
- [ ] Test (AC-8): encode a record with known `global_position = 0xABCD_EF01_2345_6789u64`.
      Assert `&buf[4..12] == 0xABCD_EF01_2345_6789u64.to_le_bytes()` (global_position starts
      at byte 4, immediately after the 4-byte length prefix). Assert `&buf[0..4]` decodes as
      a u32 LE equal to `(buf.len() - 4) as u32` (record_length covers everything after itself).
      Assert the last 4 bytes decode as a u32 LE matching the expected CRC32 of the record body.
- [ ] Test (AC-9): manually construct a byte buffer with a valid record structure but inject
      invalid UTF-8 bytes (e.g., `[0xFF, 0xFE]`) into the event type region; call `decode_record`
      and assert it returns `Err(Error::CorruptRecord { .. })`.
- [ ] Quality gates pass (build, clippy, fmt, tests).

**Dependencies:** Ticket 2
**Complexity:** L
**Maps to PRD AC:** AC-3, AC-4, AC-5, AC-6, AC-7, AC-8, AC-9

---

### Ticket 4: Verification and integration check

**Description:**
Run the full PRD 002 acceptance criteria checklist end-to-end. Verify the codec module
integrates correctly with the existing crate: all tests green, no warnings, clippy clean,
formatted. This ticket produces no new code; it is the quality gate before PRD 003 begins.

**Scope:**
- No new files. Run quality checks only.

**Acceptance Criteria:**
- [ ] `cargo build 2>&1 | tail -1` exits zero with no warnings.
- [ ] `cargo clippy --all-targets --all-features --locked -- -D warnings` exits zero.
- [ ] `cargo fmt --check` exits zero.
- [ ] `cargo test` exits zero with all tests green (including all codec tests from Tickets 1-3
      and all pre-existing PRD 001 tests).
- [ ] `grep -r "unwrap()" src/codec.rs` returns no matches (no `.unwrap()` in codec).
- [ ] All public items in `src/codec.rs` have doc comments (spot-check with `cargo doc --no-deps`).
- [ ] All PRD AC-1 through AC-9 test cases are present and passing.
- [ ] No regressions in `src/types.rs`, `src/error.rs`, or `src/lib.rs` tests.

**Dependencies:** Tickets 1, 2, 3
**Complexity:** S
**Maps to PRD AC:** AC-10

---

## AC Coverage Matrix

| PRD AC # | Description                              | Covered By Ticket(s) | Status  |
|----------|------------------------------------------|----------------------|---------|
| AC-1     | Header encoding (magic + version bytes)  | Ticket 2             | Covered |
| AC-2     | Header decoding (valid + bad magic + bad version) | Ticket 2    | Covered |
| AC-3     | Record round-trip (full, empty, max type, binary) | Ticket 3    | Covered |
| AC-4     | Record encoding determinism              | Ticket 3             | Covered |
| AC-5     | CRC32 integrity (payload, stream_id, checksum region) | Ticket 3 | Covered |
| AC-6     | Partial record detection (too short, truncated body, trailing bytes) | Tickets 1, 3 | Covered |
| AC-7     | Multiple records in sequence             | Ticket 3             | Covered |
| AC-8     | Field boundary correctness               | Ticket 3             | Covered |
| AC-9     | Event type UTF-8 validation              | Ticket 3             | Covered |
| AC-10    | Build and lint                           | Tickets 1, 2, 3, 4  | Covered |
