# Tickets for PRD 008: Batch Atomicity and Crash Consistency Hardening

**Source PRD:** prd/008-batch-atomicity-crash-consistency.md
**Created:** 2026-02-26
**Total Tickets:** 5
**Estimated Total Complexity:** 13 (S=1, M=2, L=3 → 3+3+3+2+2)

---

### Ticket 1: Batch Envelope Codec — Types, Constants, Encode/Decode, VERSION Bump

**Description:**
Add `BatchHeader` and `BatchFooter` structs, the `BATCH_HEADER_MAGIC` and `BATCH_FOOTER_MAGIC`
constants, `encode_batch_header`, `decode_batch_header`, `encode_batch_footer`, and
`decode_batch_footer` functions to `src/codec.rs`. Bump `FORMAT_VERSION` from `1` to `2` and
update `decode_header` to accept only version `2`, returning `Error::InvalidHeader` (mentioning
"version") for anything else.

**Scope:**
- Modify: `src/codec.rs`

**Acceptance Criteria:**
- [ ] `BATCH_HEADER_MAGIC` is `[0x45, 0x46, 0x42, 0x42]` and `BATCH_FOOTER_MAGIC` is `[0x45, 0x46, 0x42, 0x46]`, declared as `pub(crate) const [u8; 4]`.
- [ ] `pub struct BatchHeader { pub record_count: u32, pub first_global_pos: u64 }` exists, derives `Debug` and `PartialEq`.
- [ ] `pub struct BatchFooter { pub batch_crc: u32 }` exists, derives `Debug` and `PartialEq`.
- [ ] `pub fn encode_batch_header(record_count: u32, first_global_pos: u64) -> [u8; 16]` is public within the crate.
- [ ] `pub fn decode_batch_header(buf: &[u8]) -> Result<DecodeOutcome<BatchHeader>, Error>` returns `DecodeOutcome::Incomplete` for `buf.len() < 16`, `Err(Error::CorruptRecord)` for wrong magic, `Ok(DecodeOutcome::Complete { event: BatchHeader, consumed: 16 })` on success. (Re-use the existing `DecodeOutcome` type, parameterizing it over `T` or introduce a separate enum — see note below.)
- [ ] `pub fn encode_batch_footer(batch_crc: u32) -> [u8; 8]` is public within the crate.
- [ ] `pub fn decode_batch_footer(buf: &[u8]) -> Result<DecodeOutcome<BatchFooter>, Error>` returns `DecodeOutcome::Incomplete` for `buf.len() < 8`, `Err(Error::CorruptRecord)` for wrong magic, `Ok(...)` on success.
- [ ] `FORMAT_VERSION` constant is `2`. `decode_header` rejects version `1` files with `Error::InvalidHeader` containing "version" in the message.
- [ ] Test (AC 8): Call `encode_batch_header(3, 42)` and inspect raw bytes — `bytes[0..4] == [0x45, 0x46, 0x42, 0x42]`, `bytes[4..8] == 3u32.to_le_bytes()`, `bytes[8..16] == 42u64.to_le_bytes()`.
- [ ] Test (AC 9): Call `encode_batch_footer(0xDEAD_BEEF)` and inspect raw bytes — `bytes[0..4] == [0x45, 0x46, 0x42, 0x46]`, `bytes[4..8] == 0xDEAD_BEEFu32.to_le_bytes()`.
- [ ] Test: `decode_batch_header` with a buffer shorter than 16 bytes returns `Ok(DecodeOutcome::Incomplete)`.
- [ ] Test: `decode_batch_header` with correct length but wrong magic at byte 0 returns `Err(Error::CorruptRecord { .. })`.
- [ ] Test: `encode_batch_header(count, pos)` -> `decode_batch_header` round-trip: decoded `record_count == count`, `first_global_pos == pos`, `consumed == 16`.
- [ ] Test: `decode_batch_footer` with a buffer shorter than 8 bytes returns `Ok(DecodeOutcome::Incomplete)`.
- [ ] Test: `decode_batch_footer` with wrong magic returns `Err(Error::CorruptRecord { .. })`.
- [ ] Test: `encode_batch_footer(crc)` -> `decode_batch_footer` round-trip: decoded `batch_crc == crc`, `consumed == 8`.
- [ ] Test (AC 5 partial): `decode_header` called with a valid 8-byte buffer at `FORMAT_VERSION = 1` (old magic + `[1, 0, 0, 0]`) returns `Err(Error::InvalidHeader(msg))` where `msg.contains("version")`.
- [ ] Test: `decode_header` with the new format version 2 header (produced by the updated `encode_header()`) returns `Ok(2)`.
- [ ] Quality gates pass: `cargo build`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo fmt --check`, `cargo test`.

> **Implementer note:** `DecodeOutcome` is currently `enum DecodeOutcome` with a `RecordedEvent` in `Complete`. You have two implementation choices: (a) make it generic `DecodeOutcome<T>` with `Complete { value: T, consumed: usize }` and update all existing call sites, or (b) introduce a new private enum `BatchDecodeOutcome` used only in the new batch decode functions. Choose whichever keeps the diff smaller. Option (b) avoids touching `store.rs` prematurely.

**Dependencies:** None
**Complexity:** L
**Maps to PRD AC:** AC 5 (partial — version rejection), AC 8, AC 9

---

### Ticket 2: Store::append — Wrap Each Write in a Batch Envelope

**Description:**
Update `Store::append` in `src/store.rs` to prepend a batch header and append a batch footer
(with a computed CRC32 over header bytes + all record bytes) to the `encoded_batch` buffer before
the single `write_all` + `sync_all` call. The rest of the append logic (validation, event
construction, index update) is unchanged. After this ticket, every successfully fsynced write
produces a file containing a valid batch header, N records, and a valid batch footer — matching
PRD AC 1.

**Scope:**
- Modify: `src/store.rs`

**Acceptance Criteria:**
- [ ] `Store::append` calls `codec::encode_batch_header(count, first_global_pos)` before encoding records and `codec::encode_batch_footer(batch_crc)` after, where `batch_crc = crc32fast::hash(&header_bytes || &all_record_bytes)`.
- [ ] The buffer passed to `write_all` is: `batch_header_bytes || record_0_bytes || ... || record_N-1_bytes || batch_footer_bytes` (a single contiguous `Vec<u8>`).
- [ ] `sync_all` is still called after `write_all` and before the index write lock is acquired.
- [ ] Test (AC 1): Append a batch of 3 events to a fresh store. Read the raw bytes of the log file. After byte offset 8 (file header): bytes 0..16 decode as a valid `BatchHeader` with `record_count == 3` and `first_global_pos == 0`. The next bytes are 3 individually-decodable records (using `decode_record`). The next 8 bytes decode as a valid `BatchFooter` with the correct CRC32 (recomputed over the 16 header bytes + the concatenated 3 record bytes). No extra bytes remain.
- [ ] Test: Append two separate batches (first batch 2 events, second batch 1 event). Read raw file bytes. Verify two consecutive batch envelopes are present: first envelope has `record_count == 2`, second has `record_count == 1`, `first_global_pos == 2`.
- [ ] Test: `Store::append` still returns the correct `Vec<RecordedEvent>` with correct `global_position` and `stream_version` values after the batch envelope change.
- [ ] Test: `store.read_all(0, 100)` after a 3-event append returns all 3 events in order.
- [ ] Quality gates pass.

> **Implementer note:** `Store::open` (recovery loop) is updated in Ticket 3, not here. After this ticket, `Store::open` cannot open a file written by the new `append` (the format version has changed and the recovery loop expects old individual-record format). This is acceptable — the store tests that call `Store::open` followed by `append` will fail until Ticket 3 is complete. Write the Ticket 2 tests to not call `Store::open` on a file that was written by the new `append`. Alternatively, seed the test file manually using `encode_batch_header` + `encode_record` + `encode_batch_footer` and write only unit tests of `append`'s output bytes, not round-trip tests through `open`.

**Dependencies:** Ticket 1
**Complexity:** L
**Maps to PRD AC:** AC 1

---

### Ticket 3: Store::open — Batch-Aware Recovery Loop and Directory Fsync

**Description:**
Rewrite the recovery loop in `Store::open` to decode batches (header → N records → footer) rather
than individual records. A partial batch at the tail (missing or corrupt footer, or truncated
records) is truncated with a `tracing::warn!` and the store opens successfully with all prior
complete batches. Additionally, add a directory fsync immediately after `file.sync_all()` on the
new-file creation branch. This ticket makes `Store::open` compatible with the new format produced
by Ticket 2.

**Scope:**
- Modify: `src/store.rs`

**Acceptance Criteria:**
- [ ] The recovery loop reads one batch at a time: decode `BatchHeader`, then `record_count` records using `decode_record`, then `BatchFooter`. A complete, valid batch is committed to the in-memory index.
- [ ] If `decode_batch_header` returns `Incomplete` at the current offset (trailing partial header): truncate file to `offset`, `tracing::warn!`, return store with events from prior batches.
- [ ] If `decode_batch_header` returns `CorruptRecord` and no valid batch follows (checked via a `has_valid_batch_after` helper or equivalent): truncate and return. If valid data follows, escalate to `Err(Error::CorruptRecord)`.
- [ ] If any record within the batch is `Incomplete` or `CorruptRecord`: truncate to `batch_start_offset`, `tracing::warn!`, return store with events from prior batches.
- [ ] If `decode_batch_footer` is `Incomplete` or its magic is wrong: truncate to `batch_start_offset`, `tracing::warn!`, return.
- [ ] If `decode_batch_footer` CRC does not match the recomputed CRC over the header + record bytes: truncate to `batch_start_offset`, `tracing::warn!`, return.
- [ ] The new-file creation branch calls `std::fs::File::open(parent_dir)?.sync_all()?` after `file.sync_all()`.
- [ ] Test (AC 2): Write a complete batch (header + 2 records + footer) to a file, then remove the last 8 bytes (footer). Call `Store::open`. Assert the store has 0 events and the file is truncated to the byte offset of the batch header (i.e., `HEADER_SIZE = 8`).
- [ ] Test (AC 3): Write a complete batch header, encode 2 records, truncate after the first record's first 4 bytes (mid-record), omit footer. Call `Store::open`. Assert 0 events, file truncated to `HEADER_SIZE`.
- [ ] Test (AC 4): Write two complete valid batches, then append an incomplete third batch (header + 1 of 2 records, no footer). Call `Store::open`. Assert exactly the events from the first two batches are present. Assert `read_all(0, 100).len() == total_events_from_first_two_batches`.
- [ ] Test (AC 6): Call `Store::open` on a non-existent path. Assert the returned store has 0 events. Assert a second `Store::open` on the same path (after simulating process exit by dropping the first store) returns an empty but valid store with 0 events.
- [ ] Test (AC 7): Write two complete batches to a file (manually using `encode_batch_header` + `encode_record` + `encode_batch_footer`). Call `Store::open`. Assert `read_all(0, 100)` returns all events in global-position order with no gaps.
- [ ] Test (AC 5): Manually write a file with the old `FORMAT_VERSION = 1` file header (magic `EFDB` + `[1, 0, 0, 0]`). Call `Store::open`. Assert `Err(Error::InvalidHeader(msg))` where `msg.contains("version")`.
- [ ] Quality gates pass.

> **Implementer note:** The `has_valid_record_after` helper (currently used by the old recovery loop) needs to be replaced or updated to operate at batch granularity. A new `has_valid_batch_after(data: &[u8], start: usize) -> bool` that probes for a valid `BatchHeader` magic at every offset after `start` is appropriate. Keep `has_valid_record_after` if it is still used elsewhere, or remove it if it is no longer needed.

**Dependencies:** Ticket 1, Ticket 2
**Complexity:** L
**Maps to PRD AC:** AC 2, AC 3, AC 4, AC 5, AC 6, AC 7

---

### Ticket 4: Documentation — Filesystem Assumptions in docs/design.md

**Description:**
Add a "Filesystem Assumptions" subsection to `docs/design.md` under the "On-Disk Format" section.
The subsection must document: the supported filesystem (ext4 `data=ordered`), the `data=ordered`
guarantee and its implications for fsync semantics, the requirement for directory fsync on new file
creation, the use of `File::sync_all()` (not `fdatasync`), and the explicit non-support for
network-attached filesystems.

**Scope:**
- Modify: `docs/design.md`

**Acceptance Criteria:**
- [ ] A "Filesystem Assumptions" subsection exists under the "On-Disk Format" section (or a top-level section if "On-Disk Format" does not exist — confirm before starting).
- [ ] The subsection states that ext4 with `data=ordered` is the tested and supported filesystem.
- [ ] The subsection explains that `data=ordered` guarantees file data is written before inode metadata is committed, making a `File::sync_all()` sufficient for data durability.
- [ ] The subsection states that a directory fsync is required after creating a new file to make the directory entry durable.
- [ ] The subsection states that all fsync calls use `File::sync_all()` (maps to `fsync(2)`, not `fdatasync(2)`).
- [ ] The subsection explicitly states that NFS, CIFS, and FUSE-based filesystems are not supported.
- [ ] The subsection notes that other Linux filesystems (XFS, btrfs, ZFS, tmpfs) may work but are not validated.
- [ ] Test: `cargo build` passes (this is a docs-only change; verify no broken doc links or `//!` changes are needed).
- [ ] Quality gates pass (`cargo fmt --check`, `cargo clippy` — confirm no Rust code is touched).

**Dependencies:** None (documentation is independent of code changes)
**Complexity:** M
**Maps to PRD AC:** (Derived AC from "Filesystem assumptions documentation" section of PRD)

---

### Ticket 5: Verification and Integration Testing

**Description:**
Run the full PRD 008 acceptance criteria checklist end-to-end. Verify all four prior tickets
integrate correctly as a cohesive feature. Confirm that the existing integration tests (in
`tests/`) still pass, that the format version bump is consistently enforced, and that the batch
envelope is observable in a real gRPC round-trip scenario.

**Scope:**
- Modify: `tests/` (add or extend integration tests as needed; no more than 1-2 new test files)

**Acceptance Criteria:**
- [ ] All PRD AC 1–9 pass (confirmed by running `cargo test` and checking test names map to each AC).
- [ ] AC 1: A unit test in `src/store.rs` confirms the raw file bytes after `append` contain a valid 16-byte batch header, N records, and a valid 8-byte batch footer with correct CRC.
- [ ] AC 2: A unit test in `src/store.rs` confirms that truncating the footer triggers recovery-by-truncation with 0 events returned.
- [ ] AC 3: A unit test in `src/store.rs` confirms mid-record truncation is handled as a partial batch, not mid-file corruption.
- [ ] AC 4: A unit test in `src/store.rs` confirms two full batches + one partial third batch recovers exactly the events from batches 1 and 2.
- [ ] AC 5: A unit test in `src/codec.rs` or `src/store.rs` confirms `Store::open` on a version-1 file returns `Err(Error::InvalidHeader)` mentioning "version".
- [ ] AC 6: A unit test in `src/store.rs` confirms directory fsync occurs on new file creation (observed indirectly: open succeeds, second open recovers an empty valid store).
- [ ] AC 7: A unit test in `src/store.rs` confirms two-batch recovery produces gap-free global positions.
- [ ] AC 8: A unit test in `src/codec.rs` confirms `encode_batch_header` byte layout.
- [ ] AC 9: A unit test in `src/codec.rs` confirms `encode_batch_footer` byte layout.
- [ ] All existing tests in `tests/` pass without modification (gRPC integration tests are unaffected because `Store::open` creates a fresh file per test using `tempfile`).
- [ ] `cargo build` produces zero warnings.
- [ ] `cargo clippy --all-targets --all-features --locked -- -D warnings` produces zero warnings.
- [ ] `cargo fmt --check` passes.
- [ ] `cargo test` is fully green.

**Dependencies:** Tickets 1, 2, 3, 4
**Complexity:** M
**Maps to PRD AC:** AC 1–9, AC 10

---

## AC Coverage Matrix

| PRD AC # | Description | Covered By Ticket(s) | Status |
|----------|-------------|----------------------|--------|
| 1 | After `append` + `sync_all`, log file contains valid batch header, N records, valid batch footer with correct CRC | Ticket 2, Ticket 5 | Covered |
| 2 | Truncating batch footer → `Store::open` truncates to batch header offset, warns, returns 0 events | Ticket 3, Ticket 5 | Covered |
| 3 | Mid-record truncation → `Store::open` truncates to batch header offset, warns, returns prior batches | Ticket 3, Ticket 5 | Covered |
| 4 | Two complete batches + partial third → `Store::open` recovers exactly the two complete batches | Ticket 3, Ticket 5 | Covered |
| 5 | `FORMAT_VERSION = 1` file → `Store::open` returns `Err(Error::InvalidHeader)` mentioning "version", file not modified | Ticket 1 (version bump), Ticket 3 (open behavior), Ticket 5 | Covered |
| 6 | New file creation → parent directory receives `fsync`; second open recovers empty-but-valid store | Ticket 3, Ticket 5 | Covered |
| 7 | Two complete batches recovered → `read_all` returns all events in global-position order, no gaps | Ticket 3, Ticket 5 | Covered |
| 8 | `encode_batch_header(count, pos)` produces exactly 16 bytes with correct magic, count LE, pos LE at correct offsets | Ticket 1, Ticket 5 | Covered |
| 9 | `encode_batch_footer(crc)` produces exactly 8 bytes with correct magic and crc LE at correct offsets | Ticket 1, Ticket 5 | Covered |
| 10 | `cargo build`, `cargo clippy`, `cargo fmt --check`, `cargo test` all pass | Ticket 5 | Covered |
| Derived | Filesystem assumptions documented in `docs/design.md` (ext4 `data=ordered`, directory fsync rationale, `sync_all` semantics, NFS not supported) | Ticket 4 | Covered |
