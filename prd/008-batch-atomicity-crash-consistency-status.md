# Build Status: PRD 008 -- Batch Atomicity and Crash Consistency Hardening

**Source PRD:** prd/008-batch-atomicity-crash-consistency.md
**Tickets:** prd/008-batch-atomicity-crash-consistency-tickets.md
**Started:** 2026-02-27
**Last Updated:** 2026-02-27
**Overall Status:** QA READY

---

## Ticket Tracker

| Ticket | Title | Status | Impl Report | Review Report | Notes |
|--------|-------|--------|-------------|---------------|-------|
| 1 | Batch Envelope Codec — Types, Constants, Encode/Decode, VERSION Bump | DONE | ticket-01-impl.md | ticket-01-review.md | APPROVED |
| 2 | Store::append — Wrap Each Write in a Batch Envelope | DONE | ticket-02-impl.md | ticket-02-review.md | APPROVED |
| 3 | Store::open — Batch-Aware Recovery Loop and Directory Fsync | DONE | ticket-03-impl.md | ticket-03-review.md | APPROVED |
| 4 | Documentation — Filesystem Assumptions in docs/design.md | DONE | ticket-04-impl.md | ticket-04-review.md | APPROVED |
| 5 | Verification and Integration Testing | DONE | ticket-05-impl.md | ticket-05-review.md | APPROVED |

## Prior Work Summary

- PRDs 001-008 complete
- `src/codec.rs`: `FORMAT_VERSION = 2`, generic `DecodeOutcome<T>`, `BatchHeader`/`BatchFooter` structs, encode/decode functions, magic constants, `BATCH_HEADER_SIZE`/`BATCH_FOOTER_SIZE`
- `src/store.rs`: `Store::append` writes batch envelopes (header + records + footer with CRC32); `Store::open` batch-aware recovery loop with truncation for partial batches; directory fsync on new file creation; `has_valid_batch_after` helper; `truncate_and_return` helper
- `docs/design.md`: "Filesystem Assumptions" subsection (ext4 `data=ordered`, directory fsync, `sync_all` semantics, NFS not supported)
- 210 tests passing (0 ignored), all quality gates clean

## Follow-Up Tickets

(none)

## Completion Report

**Completed:** 2026-02-27
**Tickets Completed:** 5/5

### Summary of Changes
- `src/codec.rs`: `FORMAT_VERSION` bumped 1->2, `DecodeOutcome` made generic `<T>`, added `BatchHeader`/`BatchFooter` structs, `BATCH_HEADER_MAGIC`/`BATCH_FOOTER_MAGIC` constants, `encode_batch_header`/`decode_batch_header`/`encode_batch_footer`/`decode_batch_footer` functions, `BATCH_HEADER_SIZE = 16`/`BATCH_FOOTER_SIZE = 8`
- `src/store.rs`: `Store::append` wraps writes in batch envelopes with CRC32; `Store::open` rewritten with batch-aware recovery loop (truncates partial batches, verifies footer CRC); directory fsync on new-file creation; `has_valid_batch_after` replaces `has_valid_record_after`; `truncate_and_return` helper
- `src/writer.rs`: Minor field rename (`event` -> `value` in DecodeOutcome pattern), `#[ignore]` removed
- `tests/server_binary.rs`: `#[ignore]` removed from 2 tests
- `docs/design.md`: "Filesystem Assumptions" subsection added
- 210 tests total (170 lib + 8 main + 2 broker + 23 grpc + 6 server_binary + 1 writer)
- All quality gates pass: build, clippy, fmt, test

### Known Issues / Follow-Up
- FORMAT_VERSION 1 files are rejected; no migration tool provided (documented as out-of-scope in PRD)

### Ready for QA: YES
