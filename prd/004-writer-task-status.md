# Build Status: PRD 004 -- Writer Task

**Source PRD:** prd/004-writer-task.md
**Tickets:** prd/004-writer-task-tickets.md
**Started:** 2026-02-25
**Last Updated:** 2026-02-25
**Overall Status:** IN PROGRESS

---

## Ticket Tracker

| Ticket | Title | Status | Impl Report | Review Report | Notes |
|--------|-------|--------|-------------|---------------|-------|
| 1 | Add `tokio` dependency to `Cargo.toml` | DONE | ticket-01-impl.md | (skipped: trivial) | |
| 2 | `EventLog` shared state + `Store` refactor | DONE | ticket-02-impl.md | ticket-02-review.md | APPROVED |
| 3 | `ReadIndex` shared read-only view | DONE | ticket-03-impl.md | ticket-03-review.md | APPROVED |
| 4 | `AppendRequest` + `WriterHandle` types | DONE | ticket-04-impl.md | ticket-04-review.md | APPROVED |
| 5 | `run_writer` + `spawn_writer` | IN PROGRESS | -- | -- | |
| 6 | Verification and integration | TODO | -- | -- | |

## Prior Work Summary

- PRDs 001-003 implemented: `types.rs`, `error.rs`, `codec.rs`, `store.rs`, `lib.rs`
- Ticket 1: `tokio = { version = "1", features = ["full"] }` added to Cargo.toml
- Ticket 2: `EventLog` struct added to `store.rs` (pub, Debug, fields: `events: Vec<RecordedEvent>`, `streams: HashMap<Uuid, Vec<u64>>`)
- `Store` now holds `file: File` and `log: Arc<std::sync::RwLock<EventLog>>`
- `Store::log()` returns `Arc::clone` of the inner lock for sharing with ReadIndex
- `Store::append` acquires write lock only AFTER fsync (critical correctness)
- Read methods (`read_stream`, `read_all`, `stream_version`, `global_position`) use read lock
- `EventLog` re-exported from crate root via `lib.rs`
- Ticket 3: `src/reader.rs` created with `ReadIndex` struct (pub, Clone, Debug)
- `ReadIndex::new(log)`, `read_all`, `read_stream`, `stream_version`, `global_position`
- `ReadIndex` re-exported from crate root
- Ticket 4: `src/writer.rs` created with `AppendRequest` and `WriterHandle` (pub, Clone)
- `WriterHandle::append` is async, creates oneshot, sends over mpsc, maps errors to `InvalidArgument`
- `WriterHandle` re-exported from crate root
- 100 tests passing, all quality gates clean
- Rust edition 2024

## Follow-Up Tickets

(none yet)

## Completion Report

(pending)
