//! Binary codec for the EventfoldDB append-only log file.
//!
//! This module handles serialization and deserialization of the file header and
//! individual event records. It is pure data transformation -- no file I/O, no
//! async, no index management.
//!
//! The file header is a fixed 8-byte sequence (magic number + format version).
//! Each record is a length-prefixed, CRC32-checksummed binary frame containing
//! a single [`RecordedEvent`].

use bytes::Bytes;
use uuid::Uuid;

use crate::error::Error;
use crate::types::RecordedEvent;

/// Magic bytes identifying an EventfoldDB log file (ASCII "EFDB").
const MAGIC: [u8; 4] = [0x45, 0x46, 0x44, 0x42];

/// Current on-disk format version.
const FORMAT_VERSION: u32 = 1;

/// Result of attempting to decode a single record from a byte buffer.
///
/// Distinguishes between a successfully decoded record and a buffer that does
/// not contain enough bytes to form a complete record. This distinction is
/// critical for crash recovery: a truncated trailing record is expected after
/// an unclean shutdown, whereas a checksum mismatch in the middle of the log
/// indicates corruption.
///
/// # Variants
///
/// * `Complete` - A full record was decoded successfully.
/// * `Incomplete` - The buffer does not contain enough bytes for a complete record.
#[derive(Debug)]
pub enum DecodeOutcome {
    /// A full record was successfully decoded from the buffer.
    Complete {
        /// The decoded event.
        event: RecordedEvent,
        /// Total number of bytes consumed from the buffer (including the
        /// 4-byte length prefix).
        consumed: usize,
    },
    /// The buffer does not contain enough bytes to form a complete record.
    Incomplete,
}

/// Encode the file header as a fixed 8-byte array.
///
/// The header consists of a 4-byte magic number (`EFDB` in ASCII) followed by
/// a 4-byte format version in little-endian encoding. The current format
/// version is `1`.
///
/// # Returns
///
/// An 8-byte array containing the encoded file header.
pub fn encode_header() -> [u8; 8] {
    let mut buf = [0u8; 8];
    buf[0..4].copy_from_slice(&MAGIC);
    buf[4..8].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
    buf
}

/// Decode and validate the file header.
///
/// Checks that the magic number matches `EFDB` and that the format version is
/// supported (currently only version `1`).
///
/// # Arguments
///
/// * `buf` - Exactly 8 bytes containing the file header.
///
/// # Returns
///
/// The format version on success.
///
/// # Errors
///
/// Returns [`Error::InvalidHeader`] if the magic number is wrong or the
/// format version is unsupported.
pub fn decode_header(buf: &[u8; 8]) -> Result<u32, Error> {
    if buf[0..4] != MAGIC {
        return Err(Error::InvalidHeader(
            "wrong magic bytes: expected EFDB".to_string(),
        ));
    }
    let version = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
    if version != FORMAT_VERSION {
        return Err(Error::InvalidHeader(format!(
            "unsupported format version: {version}"
        )));
    }
    Ok(version)
}

/// Fixed-size portion of a record body (everything except variable-length fields):
/// global_position(8) + stream_id(16) + stream_version(8) + event_id(16) +
/// event_type_len(2) + metadata_len(4) + payload_len(4) + checksum(4) = 62.
const FIXED_BODY_SIZE: usize = 8 + 16 + 8 + 16 + 2 + 4 + 4 + 4;

/// Size of the length prefix field in bytes.
const LENGTH_PREFIX_SIZE: usize = 4;

/// Encode a [`RecordedEvent`] into the binary on-disk format.
///
/// The returned buffer contains the length prefix, all record fields, and a
/// trailing CRC32 checksum. The caller can append this directly to the log
/// file.
///
/// # Arguments
///
/// * `event` - The recorded event to serialize.
///
/// # Returns
///
/// A `Vec<u8>` containing the complete binary record.
pub fn encode_record(event: &RecordedEvent) -> Vec<u8> {
    let et_bytes = event.event_type.as_bytes();
    let body_len = FIXED_BODY_SIZE + et_bytes.len() + event.metadata.len() + event.payload.len();
    let total_len = LENGTH_PREFIX_SIZE + body_len;

    let mut buf = Vec::with_capacity(total_len);

    // record_length: byte count from global_position through checksum (inclusive).
    buf.extend_from_slice(&(body_len as u32).to_le_bytes());

    // -- Begin body (CRC32 covers from here through payload) --
    buf.extend_from_slice(&event.global_position.to_le_bytes());
    buf.extend_from_slice(event.stream_id.as_bytes());
    buf.extend_from_slice(&event.stream_version.to_le_bytes());
    buf.extend_from_slice(event.event_id.as_bytes());
    buf.extend_from_slice(&(et_bytes.len() as u16).to_le_bytes());
    buf.extend_from_slice(et_bytes);
    buf.extend_from_slice(&(event.metadata.len() as u32).to_le_bytes());
    buf.extend_from_slice(&event.metadata);
    buf.extend_from_slice(&(event.payload.len() as u32).to_le_bytes());
    buf.extend_from_slice(&event.payload);
    // -- End body --

    // CRC32 over the body (everything after record_length, before checksum).
    let crc = crc32fast::hash(&buf[LENGTH_PREFIX_SIZE..]);
    buf.extend_from_slice(&crc.to_le_bytes());

    buf
}

/// Decode a single record from the start of a byte buffer.
///
/// Handles three cases:
///
/// 1. **Complete record** -- returns [`DecodeOutcome::Complete`] with the
///    decoded event and the total number of bytes consumed.
/// 2. **Incomplete data** -- the buffer is too short to contain a full record
///    (either fewer than 4 bytes for the length prefix, or fewer bytes than
///    the length prefix indicates). Returns [`DecodeOutcome::Incomplete`].
/// 3. **Corrupt data** -- the checksum does not match or a field is malformed.
///    Returns [`Error::CorruptRecord`].
///
/// # Arguments
///
/// * `buf` - A byte slice starting at the beginning of a record.
///
/// # Returns
///
/// A [`DecodeOutcome`] on success, or an [`Error`] if the record is corrupt.
///
/// # Errors
///
/// Returns [`Error::CorruptRecord`] if the CRC32 checksum does not match or
/// if field data is malformed (e.g., invalid UTF-8 in the event type).
pub fn decode_record(buf: &[u8]) -> Result<DecodeOutcome, Error> {
    // Need at least 4 bytes for the length prefix.
    if buf.len() < LENGTH_PREFIX_SIZE {
        return Ok(DecodeOutcome::Incomplete);
    }

    let record_length = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    let total = LENGTH_PREFIX_SIZE + record_length;

    // Need the full record body + length prefix.
    if buf.len() < total {
        return Ok(DecodeOutcome::Incomplete);
    }

    // Slice the body (global_position through checksum).
    let body = &buf[LENGTH_PREFIX_SIZE..total];

    // The last 4 bytes of the body are the checksum; everything before is the
    // CRC32-protected region.
    if body.len() < 4 {
        return Err(Error::CorruptRecord {
            position: 0,
            detail: "record body too short for checksum".to_string(),
        });
    }
    let crc_offset = body.len() - 4;
    let stored_crc = u32::from_le_bytes([
        body[crc_offset],
        body[crc_offset + 1],
        body[crc_offset + 2],
        body[crc_offset + 3],
    ]);
    let computed_crc = crc32fast::hash(&body[..crc_offset]);

    if stored_crc != computed_crc {
        return Err(Error::CorruptRecord {
            position: 0,
            detail: format!(
                "CRC32 mismatch: stored {stored_crc:#010X}, computed {computed_crc:#010X}"
            ),
        });
    }

    // Parse fixed fields from the CRC-protected region.
    // We read from `body` which starts at global_position.
    let protected = &body[..crc_offset];
    let mut cursor = 0;

    // Helper macro: read N bytes from `protected` at `cursor`, advance cursor,
    // or return CorruptRecord if the remaining data is too short.
    macro_rules! read_bytes {
        ($n:expr) => {{
            if cursor + $n > protected.len() {
                return Err(Error::CorruptRecord {
                    position: 0,
                    detail: "unexpected end of record body".to_string(),
                });
            }
            let start = cursor;
            cursor += $n;
            &protected[start..cursor]
        }};
    }

    // global_position (u64 LE, 8 bytes)
    let gp_bytes = read_bytes!(8);
    let global_position = u64::from_le_bytes(gp_bytes.try_into().expect("8 bytes for u64"));

    // stream_id (UUID raw bytes, 16 bytes)
    let sid_bytes = read_bytes!(16);
    let stream_id = Uuid::from_bytes(sid_bytes.try_into().expect("16 bytes for UUID"));

    // stream_version (u64 LE, 8 bytes)
    let sv_bytes = read_bytes!(8);
    let stream_version = u64::from_le_bytes(sv_bytes.try_into().expect("8 bytes for u64"));

    // event_id (UUID raw bytes, 16 bytes)
    let eid_bytes = read_bytes!(16);
    let event_id = Uuid::from_bytes(eid_bytes.try_into().expect("16 bytes for UUID"));

    // event_type_len (u16 LE, 2 bytes)
    let etl_bytes = read_bytes!(2);
    let event_type_len =
        u16::from_le_bytes(etl_bytes.try_into().expect("2 bytes for u16")) as usize;

    // event_type (UTF-8 bytes)
    let et_bytes = read_bytes!(event_type_len);
    let event_type = std::str::from_utf8(et_bytes).map_err(|e| Error::CorruptRecord {
        position: 0,
        detail: format!("invalid UTF-8 in event type: {e}"),
    })?;

    // metadata_len (u32 LE, 4 bytes)
    let ml_bytes = read_bytes!(4);
    let metadata_len = u32::from_le_bytes(ml_bytes.try_into().expect("4 bytes for u32")) as usize;

    // metadata (raw bytes)
    let meta_bytes = read_bytes!(metadata_len);

    // payload_len (u32 LE, 4 bytes)
    let pl_bytes = read_bytes!(4);
    let payload_len = u32::from_le_bytes(pl_bytes.try_into().expect("4 bytes for u32")) as usize;

    // payload (raw bytes)
    let pay_bytes = read_bytes!(payload_len);
    // Cursor is intentionally not read after the last field; suppress the warning.
    let _ = cursor;

    let event = RecordedEvent {
        event_id,
        stream_id,
        stream_version,
        global_position,
        event_type: event_type.to_string(),
        metadata: Bytes::copy_from_slice(meta_bytes),
        payload: Bytes::copy_from_slice(pay_bytes),
    };

    Ok(DecodeOutcome::Complete {
        event,
        consumed: total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_outcome_complete_is_constructible() {
        let event = RecordedEvent {
            event_id: uuid::Uuid::new_v4(),
            stream_id: uuid::Uuid::new_v4(),
            stream_version: 0,
            global_position: 0,
            event_type: "TestEvent".to_string(),
            metadata: bytes::Bytes::new(),
            payload: bytes::Bytes::from_static(b"{}"),
        };
        let outcome = DecodeOutcome::Complete {
            event,
            consumed: 100,
        };
        assert!(matches!(
            outcome,
            DecodeOutcome::Complete { consumed: 100, .. }
        ));
    }

    #[test]
    fn decode_outcome_incomplete_is_constructible() {
        let outcome = DecodeOutcome::Incomplete;
        assert!(matches!(outcome, DecodeOutcome::Incomplete));
    }

    #[test]
    fn decode_outcome_debug_is_non_empty() {
        let outcome = DecodeOutcome::Incomplete;
        let debug_str = format!("{outcome:?}");
        assert!(!debug_str.is_empty());
    }

    /// Helper: build a `RecordedEvent` with the given fields for test convenience.
    fn make_event(
        global_position: u64,
        stream_version: u64,
        event_type: &str,
        metadata: &[u8],
        payload: &[u8],
    ) -> RecordedEvent {
        RecordedEvent {
            event_id: uuid::Uuid::new_v4(),
            stream_id: uuid::Uuid::new_v4(),
            stream_version,
            global_position,
            event_type: event_type.to_string(),
            metadata: bytes::Bytes::copy_from_slice(metadata),
            payload: bytes::Bytes::copy_from_slice(payload),
        }
    }

    // AC-3a: Round-trip with non-empty metadata and payload -- all 7 fields match.

    #[test]
    fn ac3a_round_trip_non_empty_metadata_and_payload() {
        let event = make_event(0, 0, "OrderPlaced", b"meta-data", b"{\"qty\":1}");
        let buf = encode_record(&event);
        let result = decode_record(&buf).expect("decode should succeed");
        match result {
            DecodeOutcome::Complete {
                event: decoded,
                consumed,
            } => {
                assert_eq!(decoded, event);
                assert_eq!(consumed, buf.len());
            }
            DecodeOutcome::Incomplete => panic!("expected Complete, got Incomplete"),
        }
    }

    // AC-3b: Round-trip with empty metadata and empty payload.

    #[test]
    fn ac3b_round_trip_empty_metadata_and_payload() {
        let event = make_event(5, 2, "ItemRemoved", b"", b"");
        let buf = encode_record(&event);
        let result = decode_record(&buf).expect("decode should succeed");
        match result {
            DecodeOutcome::Complete {
                event: decoded,
                consumed,
            } => {
                assert_eq!(decoded, event);
                assert_eq!(consumed, buf.len());
            }
            DecodeOutcome::Incomplete => panic!("expected Complete, got Incomplete"),
        }
    }

    // AC-3c: Round-trip with event_type of exactly 256 bytes.

    #[test]
    fn ac3c_round_trip_max_length_event_type() {
        let event_type: String = "A".repeat(256);
        let event = make_event(10, 0, &event_type, b"m", b"p");
        let buf = encode_record(&event);
        let result = decode_record(&buf).expect("decode should succeed");
        match result {
            DecodeOutcome::Complete {
                event: decoded,
                consumed,
            } => {
                assert_eq!(decoded, event);
                assert_eq!(consumed, buf.len());
            }
            DecodeOutcome::Incomplete => panic!("expected Complete, got Incomplete"),
        }
    }

    // AC-3d: Round-trip with binary data containing null bytes and 0xFF.

    #[test]
    fn ac3d_round_trip_binary_data_with_null_bytes() {
        let binary_data = b"\x00\xff\x00\xff";
        let event = make_event(7, 3, "BinaryEvent", binary_data, binary_data);
        let buf = encode_record(&event);
        let result = decode_record(&buf).expect("decode should succeed");
        match result {
            DecodeOutcome::Complete {
                event: decoded,
                consumed,
            } => {
                assert_eq!(decoded, event);
                assert_eq!(consumed, buf.len());
            }
            DecodeOutcome::Incomplete => panic!("expected Complete, got Incomplete"),
        }
    }

    // AC-4: Encoding the same event twice produces identical byte sequences.

    #[test]
    fn ac4_encode_determinism() {
        let event = make_event(0, 0, "Deterministic", b"meta", b"payload");
        let buf1 = encode_record(&event);
        let buf2 = encode_record(&event);
        assert_eq!(buf1, buf2);
    }

    // AC-5a: Flip one bit inside the payload region (byte at buf.len() - 5).

    #[test]
    fn ac5a_crc_mismatch_flipped_payload_bit() {
        let event = make_event(0, 0, "TestEvent", b"meta", b"payload-data");
        let mut buf = encode_record(&event);
        // Flip one bit at offset buf.len() - 5 (inside payload, before checksum)
        let idx = buf.len() - 5;
        buf[idx] ^= 0x01;
        let result = decode_record(&buf);
        assert!(
            matches!(result, Err(Error::CorruptRecord { .. })),
            "expected CorruptRecord, got: {result:?}"
        );
    }

    // AC-5b: Flip one bit at byte offset 8 (inside the stream_id region).

    #[test]
    fn ac5b_crc_mismatch_flipped_stream_id_bit() {
        let event = make_event(0, 0, "TestEvent", b"meta", b"payload");
        let mut buf = encode_record(&event);
        // Byte offset 8 is inside global_position/stream_id region
        buf[8] ^= 0x01;
        let result = decode_record(&buf);
        assert!(
            matches!(result, Err(Error::CorruptRecord { .. })),
            "expected CorruptRecord, got: {result:?}"
        );
    }

    // AC-5c: Flip one bit in the checksum (last 4 bytes).

    #[test]
    fn ac5c_crc_mismatch_flipped_checksum_bit() {
        let event = make_event(0, 0, "TestEvent", b"meta", b"payload");
        let mut buf = encode_record(&event);
        // Flip one bit in the last byte (checksum region)
        let last = buf.len() - 1;
        buf[last] ^= 0x01;
        let result = decode_record(&buf);
        assert!(
            matches!(result, Err(Error::CorruptRecord { .. })),
            "expected CorruptRecord, got: {result:?}"
        );
    }

    // AC-6a: 2-byte buffer returns Incomplete.

    #[test]
    fn ac6a_incomplete_2_byte_buffer() {
        let result = decode_record(&[0x00, 0x01]).expect("should not error");
        assert!(
            matches!(result, DecodeOutcome::Incomplete),
            "expected Incomplete, got: {result:?}"
        );
    }

    // AC-6b: Length prefix present but buffer too short.

    #[test]
    fn ac6b_incomplete_large_length_small_buffer() {
        // First 4 bytes encode record_length = 1000, but total buffer is only 10 bytes.
        let mut buf = [0u8; 10];
        buf[0..4].copy_from_slice(&1000u32.to_le_bytes());
        let result = decode_record(&buf).expect("should not error");
        assert!(
            matches!(result, DecodeOutcome::Incomplete),
            "expected Incomplete, got: {result:?}"
        );
    }

    // AC-6c: Valid record followed by 3 extra bytes.

    #[test]
    fn ac6c_extra_trailing_bytes_consumed_correctly() {
        let event = make_event(0, 0, "TestEvent", b"meta", b"payload");
        let mut buf = encode_record(&event);
        let expected_consumed = buf.len();
        // Append 3 extra bytes (start of another record).
        buf.extend_from_slice(&[0xAA, 0xBB, 0xCC]);
        let result = decode_record(&buf).expect("decode should succeed");
        match result {
            DecodeOutcome::Complete { consumed, .. } => {
                assert_eq!(
                    consumed, expected_consumed,
                    "consumed should equal encoded record length, not total buffer"
                );
            }
            DecodeOutcome::Incomplete => panic!("expected Complete, got Incomplete"),
        }
    }

    // AC-7: Three records concatenated, decoded sequentially.

    #[test]
    fn ac7_three_records_sequential_decode() {
        let events: Vec<RecordedEvent> = (0..3)
            .map(|i| {
                make_event(
                    i,
                    i,
                    &format!("Event{i}"),
                    format!("meta{i}").as_bytes(),
                    format!("payload{i}").as_bytes(),
                )
            })
            .collect();

        let mut combined = Vec::new();
        for event in &events {
            combined.extend_from_slice(&encode_record(event));
        }

        let mut offset = 0;
        let mut total_consumed = 0;
        for (i, expected) in events.iter().enumerate() {
            let result = decode_record(&combined[offset..])
                .unwrap_or_else(|e| panic!("decode {i} should succeed: {e}"));
            match result {
                DecodeOutcome::Complete {
                    event: decoded,
                    consumed,
                } => {
                    assert_eq!(&decoded, expected, "event {i} fields mismatch");
                    offset += consumed;
                    total_consumed += consumed;
                }
                DecodeOutcome::Incomplete => panic!("expected Complete for event {i}"),
            }
        }
        assert_eq!(total_consumed, combined.len());
    }

    // AC-8: Field boundary correctness with known global_position.

    #[test]
    fn ac8_field_boundary_correctness() {
        let known_pos: u64 = 0xABCD_EF01_2345_6789;
        let event = make_event(known_pos, 0, "BoundaryTest", b"m", b"p");
        let buf = encode_record(&event);

        // global_position starts at byte 4 (immediately after the 4-byte length prefix).
        assert_eq!(
            &buf[4..12],
            &known_pos.to_le_bytes(),
            "global_position at bytes 4..12"
        );

        // record_length covers everything after itself.
        let record_length = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        assert_eq!(
            record_length,
            (buf.len() - 4) as u32,
            "record_length should be total len minus 4"
        );

        // Last 4 bytes are the CRC32 checksum. Verify it matches computing CRC32
        // over the body (bytes from global_position through payload, i.e., buf[4..buf.len()-4]).
        let stored_crc = u32::from_le_bytes([
            buf[buf.len() - 4],
            buf[buf.len() - 3],
            buf[buf.len() - 2],
            buf[buf.len() - 1],
        ]);
        let expected_crc = crc32fast::hash(&buf[4..buf.len() - 4]);
        assert_eq!(
            stored_crc, expected_crc,
            "CRC32 checksum at end should match body hash"
        );
    }

    // AC-9: Invalid UTF-8 in event type region produces CorruptRecord.

    #[test]
    fn ac9_invalid_utf8_event_type() {
        // Encode a valid record first, then manually inject invalid UTF-8 into
        // the event type region.
        let event = make_event(0, 0, "AB", b"", b"");
        let mut buf = encode_record(&event);

        // The event_type region: after record_length (4) + global_position (8) +
        // stream_id (16) + stream_version (8) + event_id (16) + event_type_len (2) = offset 54.
        // The event_type is 2 bytes ("AB") at offsets 54..56.
        let et_offset = 4 + 8 + 16 + 8 + 16 + 2; // = 54
        // Replace with invalid UTF-8
        buf[et_offset] = 0xFF;
        buf[et_offset + 1] = 0xFE;

        // Recompute the CRC32 so the checksum is valid but the UTF-8 is not.
        let body = &buf[4..buf.len() - 4];
        let new_crc = crc32fast::hash(body);
        let crc_offset = buf.len() - 4;
        buf[crc_offset..].copy_from_slice(&new_crc.to_le_bytes());

        let result = decode_record(&buf);
        assert!(
            matches!(result, Err(Error::CorruptRecord { .. })),
            "expected CorruptRecord for invalid UTF-8, got: {result:?}"
        );
    }

    // AC-1: Header encoding

    #[test]
    fn encode_header_returns_8_bytes() {
        assert_eq!(encode_header().len(), 8);
    }

    #[test]
    fn encode_header_first_4_bytes_are_magic() {
        let header = encode_header();
        assert_eq!(&header[0..4], &[0x45, 0x46, 0x44, 0x42]);
    }

    #[test]
    fn encode_header_bytes_4_to_8_are_version_1_le() {
        let header = encode_header();
        assert_eq!(&header[4..8], &1u32.to_le_bytes());
    }

    // AC-2: Header decoding

    #[test]
    fn decode_header_round_trip_returns_version_1() {
        let header = encode_header();
        let version = decode_header(&header).expect("valid header should decode");
        assert_eq!(version, 1);
    }

    #[test]
    fn decode_header_wrong_magic_returns_error_mentioning_magic() {
        let buf: [u8; 8] = [0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
        let err = decode_header(&buf).expect_err("wrong magic should fail");
        match err {
            Error::InvalidHeader(msg) => {
                assert!(
                    msg.contains("magic"),
                    "error message should mention 'magic', got: {msg}"
                );
            }
            other => panic!("expected InvalidHeader, got: {other:?}"),
        }
    }

    #[test]
    fn decode_header_unsupported_version_returns_error_mentioning_version() {
        // Correct magic, but version = 99
        let mut buf = [0u8; 8];
        buf[0..4].copy_from_slice(&[0x45, 0x46, 0x44, 0x42]);
        buf[4..8].copy_from_slice(&99u32.to_le_bytes());
        let err = decode_header(&buf).expect_err("unsupported version should fail");
        match err {
            Error::InvalidHeader(msg) => {
                assert!(
                    msg.contains("version"),
                    "error message should mention 'version', got: {msg}"
                );
            }
            other => panic!("expected InvalidHeader, got: {other:?}"),
        }
    }
}
