# PRD 002: On-Disk Format (Record Codec)

**Status:** TICKETS READY

## Summary

Implement the binary serialization and deserialization layer for the append-only log file. This includes the file header format, individual record encoding/decoding, and CRC32 integrity checking. This module is pure data transformation -- no file I/O, no async, no index management.

## Motivation

The on-disk format is the durability contract. Every event persisted by EventfoldDB passes through this codec. Getting the binary layout right -- length prefixes, field ordering, checksums -- is critical for crash recovery and data integrity. By isolating the codec from file I/O, we can test serialization/deserialization exhaustively with in-memory buffers.

## Scope

### In scope

- `codec.rs`: Functions to serialize/deserialize the file header and individual event records to/from byte buffers.
- File header: magic number, format version.
- Record format: length prefix, global position, stream ID, stream version, event type, metadata, payload, CRC32 checksum.
- CRC32 integrity verification on read.
- Detection of truncated/partial records.

### Out of scope

- File I/O (PRD 003).
- In-memory index (PRD 003).
- Startup recovery orchestration (PRD 003).

## Detailed Design

### File Header

Fixed 8-byte header at the start of the log file:

| Offset | Size | Field          | Value                          |
|--------|------|----------------|--------------------------------|
| 0      | 4    | Magic number   | `0x45464442` ("EFDB" in ASCII) |
| 4      | 4    | Format version | `1` (u32, little-endian)       |

Functions:
- `encode_header() -> [u8; 8]`
- `decode_header(buf: &[u8; 8]) -> Result<u32, Error>` -- returns format version or `Error::InvalidHeader`.

### Record Format

Each record is a contiguous byte sequence:

| Field             | Size                    | Encoding             |
|-------------------|-------------------------|----------------------|
| record_length     | 4 bytes                 | u32 LE, total bytes after this field through checksum |
| global_position   | 8 bytes                 | u64 LE               |
| stream_id         | 16 bytes                | UUID as raw bytes    |
| stream_version    | 8 bytes                 | u64 LE               |
| event_id          | 16 bytes                | UUID as raw bytes    |
| event_type_len    | 2 bytes                 | u16 LE               |
| event_type        | event_type_len bytes    | UTF-8                |
| metadata_len      | 4 bytes                 | u32 LE               |
| metadata          | metadata_len bytes      | raw bytes            |
| payload_len       | 4 bytes                 | u32 LE               |
| payload           | payload_len bytes       | raw bytes            |
| checksum          | 4 bytes                 | CRC32 LE over all bytes from global_position through payload (inclusive) |

The `record_length` field stores the total number of bytes from `global_position` through `checksum` (inclusive). This lets the reader know exactly how many bytes to consume for the record.

Functions:
- `encode_record(event: &RecordedEvent) -> Vec<u8>` -- serializes a `RecordedEvent` into the binary format.
- `decode_record(buf: &[u8]) -> Result<(RecordedEvent, usize), Error>` -- deserializes one record from the start of `buf`. Returns the event and the total number of bytes consumed (including the 4-byte length prefix). Returns `Error::CorruptRecord` if the checksum fails or the data is malformed.

### CRC32 Calculation

The checksum covers the record body: all bytes from `global_position` through the end of `payload` (i.e., everything inside the record except the `record_length` prefix and the `checksum` itself). Use `crc32fast` for the computation.

### Partial Record Detection

`decode_record` must handle these cases:
- `buf` is shorter than 4 bytes (cannot read length prefix): return a specific error or signal indicating "incomplete."
- `buf` has a length prefix but fewer bytes than the prefix indicates: partial/truncated record.
- The checksum does not match: corrupt record.

To distinguish "incomplete trailing record" (expected during crash recovery) from "corrupt record in the middle of the log," the caller (PRD 003) uses the position context. The codec simply reports what it finds.

A dedicated return type or error variant should distinguish "not enough bytes" from "checksum mismatch."

## Acceptance Criteria

### AC-1: Header encoding

- **Test**: `encode_header()` produces exactly 8 bytes.
- **Test**: First 4 bytes of encoded header equal `[0x45, 0x46, 0x44, 0x42]` (ASCII "EFDB").
- **Test**: Bytes 4..8 of encoded header equal `1u32` in little-endian.

### AC-2: Header decoding

- **Test**: `decode_header` on a correctly encoded header returns `Ok(1)`.
- **Test**: `decode_header` on bytes with wrong magic returns `Err(Error::InvalidHeader)` with a message mentioning "magic".
- **Test**: `decode_header` on bytes with unsupported version (e.g., 99) returns `Err(Error::InvalidHeader)` with a message mentioning "version".

### AC-3: Record round-trip

- **Test**: Encode a `RecordedEvent`, then decode it. All fields match the original.
- **Test**: Round-trip with empty metadata and empty payload.
- **Test**: Round-trip with maximum-length event type (256 bytes).
- **Test**: Round-trip with metadata and payload containing arbitrary binary data (including null bytes).

### AC-4: Record encoding determinism

- **Test**: Encoding the same `RecordedEvent` twice produces identical byte sequences.

### AC-5: CRC32 integrity

- **Test**: Flip one bit in the payload region of an encoded record. `decode_record` returns `Err(Error::CorruptRecord)`.
- **Test**: Flip one bit in the stream_id region. `decode_record` returns `Err(Error::CorruptRecord)`.
- **Test**: Flip one bit in the checksum itself. `decode_record` returns `Err(Error::CorruptRecord)`.

### AC-6: Partial record detection

- **Test**: Pass a buffer with only 2 bytes (less than length prefix). Decode signals "not enough data."
- **Test**: Pass a buffer with a valid length prefix but fewer bytes than indicated. Decode signals "not enough data."
- **Test**: Pass a buffer containing a valid record followed by 3 extra bytes (start of another record). Decode returns the first record and consumed byte count; the caller can continue from the remaining bytes.

### AC-7: Multiple records in sequence

- **Test**: Encode 3 different `RecordedEvent`s, concatenate the byte buffers. Decode them sequentially. All 3 match their originals and consumed byte counts sum to the total buffer length.

### AC-8: Field boundary correctness

- **Test**: Encode a record with known field values. Manually inspect byte offsets to verify the length prefix, global_position, stream_id, and checksum are at the expected positions. (This can be a single detailed test that verifies the wire format.)

### AC-9: Event type UTF-8 validation

- **Test**: `decode_record` with an event type region containing invalid UTF-8 returns `Err(Error::CorruptRecord)`.

### AC-10: Build and lint

- `cargo build` completes with zero warnings.
- `cargo clippy --all-targets --all-features --locked -- -D warnings` passes.
- `cargo fmt --check` passes.
- `cargo test` passes with all tests green.

## Dependencies

- **Depends on**: PRD 001 (types, error enum).
- **Depended on by**: PRD 003.

## Cargo.toml Additions

```toml
[dependencies]
crc32fast = "1"
```
