# PRD 010: Idempotent Appends

**Status:** TICKETS READY
**Created:** 2026-02-26
**Author:** PRD Writer Agent

---

## Problem Statement

Retrying clients can produce duplicate events when a network timeout or transient error causes a second `Append` RPC for events the server already wrote. The design doc designates `event_id` as an idempotency key but v1 does not enforce uniqueness, so duplicate events silently enter the log. This violates the expected-once delivery semantics that event-sourced systems depend on.

## Goals

- Detect duplicate event IDs on append and return the original recorded positions rather than an error, making retries transparent to callers.
- Keep the dedup window configurable and bounded so memory growth is predictable.
- Rebuild the dedup index from the on-disk log on startup so the guarantee survives process restart.

## Non-Goals

- Cross-batch partial dedup: if a single append batch contains two events with the same ID, this is still a caller error and returns `InvalidArgument`, not a silent dedup.
- Persistent dedup state beyond what can be cheaply rebuilt from the existing log on startup (no separate side-car file for the dedup index).
- Token-bucket or rate-limit-style protection against replay attacks; dedup is purely about exact event ID matching.
- Dedup across distributed nodes or multiple store instances (EventfoldDB is single-node by design).
- Changing the gRPC proto definition, `AppendResponse` shape, or any subscription behavior.

## User Stories

- As a command service author, I want a retried `Append` for already-written events to return the original positions rather than an error, so my retry logic does not need to distinguish "already written" from "successfully written."
- As an operator, I want dedup memory usage to be bounded by a configurable window size, so I can tune it for my workload without risking unbounded heap growth.
- As a platform engineer, I want the dedup index to be correct after a server restart, so a crash-then-retry sequence does not re-insert duplicate events.

## Technical Approach

### Data Structure: Bounded LRU Event ID Index

Add a new module `src/dedup.rs` that owns a bounded LRU cache mapping `Uuid` (event ID) to `Vec<RecordedEvent>` (the events from the original batch that contained this ID). The capacity is expressed as a maximum number of event IDs tracked, not a time window.

Use the `lru` crate (MIT/Apache-2.0, minimal transitive dependencies) rather than rolling a custom linked-hashmap:

```toml
lru = "0.12"
```

The `DedupIndex` struct exposes:

```rust
pub struct DedupIndex {
    cache: lru::LruCache<Uuid, Arc<Vec<RecordedEvent>>>,
}

impl DedupIndex {
    pub fn new(capacity: NonZeroUsize) -> Self { ... }

    /// Returns the recorded events for the first event in `proposed` whose
    /// event_id is already known, or `None` if no duplicates are detected.
    /// Only the first event_id in the batch is checked -- a batch is atomic,
    /// so if the first ID is a known duplicate the whole batch is one.
    pub fn check(&self, proposed: &[ProposedEvent]) -> Option<Arc<Vec<RecordedEvent>>> { ... }

    /// Record a successfully written batch in the cache. Inserts one entry per
    /// event_id in `recorded`, all pointing to the same `Arc<Vec<RecordedEvent>>`.
    pub fn record(&mut self, recorded: Vec<RecordedEvent>) { ... }

    /// Seed the index from recovered events during startup. Called once by
    /// `spawn_writer` after `Store::open` returns, walking the in-memory log.
    pub fn seed_from_log(&mut self, events: &[RecordedEvent]) { ... }
}
```

**Dedup semantics for a batch**: the check key is the `event_id` of the first event in the proposed batch. If that ID is in the cache, the entire stored `Vec<RecordedEvent>` for that key is returned as the dedup hit. A single `Arc<Vec<RecordedEvent>>` is stored once and shared across all per-event-ID entries for the same batch (N event IDs per batch, one shared allocation).

**Eviction**: the LRU cache evicts the least-recently-used entry when capacity is reached. A duplicate check on a hit also counts as a use (LRU promotion), so actively retried batches stay warm.

**Capacity default**: `EVENTFOLD_DEDUP_CAPACITY` environment variable, parsed as `usize`. Default: 65536 event IDs (covers ~64 K distinct events before eviction; at 16 bytes/UUID key this is ~1 MB of keys, plus the `Arc` pointer overhead per entry).

### Integration into the Writer Task

`run_writer` in `src/writer.rs` gains a `DedupIndex` parameter. The check-then-write sequence within the per-request loop becomes:

```
1. dedup_index.check(&req.events)
   -> Some(recorded) => send Ok(recorded) immediately, skip store.append()
   -> None           => call store.append(...)
                         -> Ok(recorded) => dedup_index.record(recorded.clone()); broker.publish; respond Ok
                         -> Err(e)       => respond Err(e)
```

The dedup check and the `record()` call both happen exclusively inside the single writer task, so no additional locking is required beyond what already exists.

`spawn_writer` builds the `DedupIndex` and seeds it from `store.log()` before entering the writer loop:

```rust
let mut dedup = DedupIndex::new(capacity);
dedup.seed_from_log(&store.log().read()...events);
```

`seed_from_log` walks the global event log in position order and calls `record()` for each event, reconstructing the LRU order as insertion order (oldest to newest, so newest events are hottest). If `capacity < total events recovered`, the oldest events are evicted and only the most recent `capacity` events remain in the index — this is acceptable because duplicates of old events are unlikely.

### Configuration

Add `dedup_capacity: NonZeroUsize` to the server configuration in `src/main.rs`, read from `EVENTFOLD_DEDUP_CAPACITY`. Default: `NonZeroUsize::new(65536).unwrap()`. Pass it through `spawn_writer`.

### File Change Table

| File | Change |
|------|--------|
| `src/dedup.rs` | New module: `DedupIndex`, unit tests |
| `src/lib.rs` | Add `pub(crate) mod dedup;` |
| `src/writer.rs` | `run_writer` / `spawn_writer` accept `DedupIndex`; dedup check in write loop |
| `src/main.rs` | Read `EVENTFOLD_DEDUP_CAPACITY`, construct `DedupIndex`, pass to `spawn_writer` |
| `Cargo.toml` | Add `lru = "0.12"` dependency |
| `tests/` | Integration test: duplicate append returns original positions |

### gRPC Layer

No changes. The `service.rs` `Append` handler calls `writer_handle.append(...)` and returns whatever `Vec<RecordedEvent>` comes back. A dedup hit returns the same `Vec<RecordedEvent>` type as a fresh write, so the `AppendResponse` construction is unchanged. `service.rs` and the `.proto` file are untouched.

## Acceptance Criteria

1. Appending a batch where all event IDs are new succeeds and returns recorded events with freshly assigned global positions, exactly as before this change.
2. Appending the same batch a second time (identical event IDs) returns `Ok` with the same `global_position` and `stream_version` values as the first append, without writing any new records to the log file.
3. After the dedup hit in AC-2, `ReadAll` and `ReadStream` return exactly the same event count as after the first append (no duplicate events in the log).
4. Appending two separate batches that each contain a different event ID succeeds independently with no interference; neither is treated as a duplicate of the other.
5. A single append batch containing two events with the same `event_id` is rejected with `Error::InvalidArgument` before any write occurs.
6. After a simulated restart (drop writer, reopen `Store`, rebuild `DedupIndex` via `seed_from_log`, spawn new writer), re-sending a batch whose event IDs were written before the restart returns `Ok` with the original positions, not a new write.
7. When the dedup index is seeded from a log containing more events than `capacity`, the index holds exactly `capacity` entries covering the most recently written `capacity` events; events outside the window are not deduped (a batch using only evicted event IDs is appended as new).
8. A dedup hit does not publish any events to the broadcast broker (no spurious subscription messages for retried appends).
9. `cargo build` produces zero warnings. `cargo clippy --all-targets --all-features --locked -- -D warnings` passes. `cargo fmt --check` passes. `cargo test` is fully green including all pre-existing tests.

## Open Questions

- **Batch-level vs. event-level dedup key**: the approach above uses only the first event ID in a batch as the dedup key. An alternative is to key every event ID in the batch independently. The first-event-ID approach is simpler and covers the primary retry pattern (retry the whole batch). If callers ever submit overlapping partial batches (batch A = [e1, e2], then batch B = [e2, e3]) the current approach will miss the e2 overlap. This is out of scope for v1 because the design doc treats each append as atomic at the batch level, but should be documented as a known limitation.
- **LRU eviction and `seed_from_log` ordering**: `seed_from_log` inserts events in global-position order (oldest first), which means the most recently written events will be the LRU hottest. This is correct but should be explicitly verified in tests.

## Dependencies

- **PRDs 001-008**: complete EventfoldDB with storage engine, writer task, broker, and gRPC server (all complete).
- **PRD 009**: console TUI (parallel; no dependency in either direction).
- **External**: `lru` crate version 0.12 (MIT/Apache-2.0).
