# PRD 004: Writer Task

## Summary

Implement the async single-writer task that serializes all write operations through a `tokio::mpsc` channel. gRPC handlers never touch the log file or index directly -- they send append requests to the writer task, which processes them sequentially, ensuring durability (fsync) and correctness (no concurrent mutations). The writer responds to each caller via a `tokio::oneshot` channel.

## Motivation

The single-writer architecture is the foundation of EventfoldDB's concurrency model. It eliminates write-side locking, enables batching of fsyncs for throughput under load, and provides natural backpressure via the bounded channel. This PRD also introduces the shared state wrapper (`Arc<RwLock<...>>`) that allows concurrent read access to the in-memory index while the writer holds exclusive write access.

## Scope

### In scope

- `writer.rs`: The writer task, `AppendRequest` type, `WriterHandle` for sending requests.
- Shared state: wrap the store's read-side data in `Arc<RwLock<...>>` so reads can proceed concurrently.
- Batching: drain multiple pending requests per loop iteration, coalescing fsyncs.
- Graceful shutdown: the writer exits when the channel is closed.
- `Cargo.toml`: add `tokio` dependency with required features.

### Out of scope

- Broadcast notifications to subscribers (PRD 005 -- the writer will gain a broker handle later).
- gRPC handlers (PRD 006).
- Server startup orchestration (PRD 007).

## Detailed Design

### Shared State

The store's read-side data must be accessible from gRPC read handlers without going through the writer task. The approach:

```rust
/// Thread-safe, read-optimized view of the event log.
pub struct EventLog {
    /// Global event log. Append-only -- new events are pushed to the end.
    events: Vec<RecordedEvent>,
    /// Stream index. Maps stream ID to list of global positions.
    streams: HashMap<Uuid, Vec<u64>>,
}
```

The `EventLog` is wrapped in `Arc<RwLock<EventLog>>` (using `tokio::sync::RwLock` or `parking_lot::RwLock`). The writer holds a write lock only during index updates (after fsync, not during I/O). Read handlers acquire a read lock for the duration of a read operation.

The `Store` from PRD 003 needs to be refactored so that:
- The `Store` owns the `File` handle and the `Arc<RwLock<EventLog>>`.
- `Store::append` writes to disk, fsyncs, then acquires the write lock to update the in-memory index.
- Read methods (`read_stream`, `read_all`, `stream_version`, `global_position`) operate on a read-locked `EventLog`.

Alternatively, the writer task can own the `Store` directly, and a separate `ReadIndex` handle (backed by `Arc<RwLock<EventLog>>`) is shared with read handlers. The PRD does not prescribe the exact internal factoring -- the acceptance criteria define the observable behavior.

### `AppendRequest`

```rust
pub struct AppendRequest {
    pub stream_id: Uuid,
    pub expected_version: ExpectedVersion,
    pub events: Vec<ProposedEvent>,
    pub response_tx: oneshot::Sender<Result<Vec<RecordedEvent>, Error>>,
}
```

### `WriterHandle`

A cloneable handle that gRPC handlers use to submit append requests:

```rust
#[derive(Clone)]
pub struct WriterHandle {
    tx: mpsc::Sender<AppendRequest>,
}

impl WriterHandle {
    /// Submit an append request and wait for the result.
    pub async fn append(
        &self,
        stream_id: Uuid,
        expected_version: ExpectedVersion,
        events: Vec<ProposedEvent>,
    ) -> Result<Vec<RecordedEvent>, Error> { ... }
}
```

The `append` method creates a oneshot channel, sends the request, and awaits the response.

### Writer Task

```rust
pub async fn run_writer(
    mut store: Store,
    mut rx: mpsc::Receiver<AppendRequest>,
) {
    // Drain and process requests in a loop.
    // On each iteration:
    //   1. recv() the first request (blocks until available or channel closed).
    //   2. try_recv() additional requests (non-blocking drain for batching).
    //   3. Process each request: call store.append() for each.
    //   4. A single fsync covers the entire batch (refactoring store.append
    //      to support deferred fsync, or batching at the writer level).
    //   5. Send results back via oneshot channels.
    // When the channel is closed (all senders dropped), exit the loop.
}
```

**Batching detail**: The simplest correct approach for v1 is to process each request individually (each `store.append` call fsyncs). A batching optimization (deferred fsync across multiple appends) can be added later. The acceptance criteria require sequential correctness; batching is a performance optimization that must not change observable behavior.

### `spawn_writer`

```rust
pub fn spawn_writer(
    store: Store,
    channel_capacity: usize,
) -> (WriterHandle, ReadIndex, JoinHandle<()>)
```

Creates the mpsc channel, spawns the writer task on the tokio runtime, and returns:
- `WriterHandle` for submitting appends.
- `ReadIndex` (or equivalent) for read access to the shared in-memory index.
- `JoinHandle` for awaiting graceful shutdown.

## Acceptance Criteria

### AC-1: Basic append through writer

- **Test**: Spawn a writer. Send an append request via `WriterHandle::append`. Await the result. Verify the returned `RecordedEvent` has correct `global_position` and `stream_version`.

### AC-2: Sequential consistency

- **Test**: Send 3 append requests sequentially (await each before sending the next). Global positions are 0, 1, 2 (or spanning the batch sizes). Stream versions are contiguous.

### AC-3: Concurrent appends are serialized

- **Test**: Spawn 10 concurrent `WriterHandle::append` calls (via `tokio::spawn`). Collect all results. Every global position is unique. The set of global positions is contiguous starting from 0. No errors (using `ExpectedVersion::Any`).

### AC-4: ExpectedVersion enforcement through writer

- **Test**: Append one event to stream A with `NoStream`. Then append another to stream A with `NoStream`. The second returns `Err(Error::WrongExpectedVersion)`.
- **Test**: Append one event with `NoStream`, then append with `Exact(0)` -- succeeds.
- **Test**: Append one event with `NoStream`, then append with `Exact(5)` -- fails.

### AC-5: Read index reflects writes

- **Test**: Spawn a writer, get the `ReadIndex`. Append 3 events. Use `ReadIndex::read_all` to read them back. All 3 are present.
- **Test**: Append events to 2 streams. Use `ReadIndex::read_stream` for each. Correct events are returned.

### AC-6: Durability (survives restart)

- **Test**: Spawn a writer with a temp directory path. Append 5 events. Drop the writer handle and await the join handle (graceful shutdown). Open a new store at the same path. All 5 events are recovered.

### AC-7: Graceful shutdown

- **Test**: Spawn a writer. Drop all `WriterHandle` clones (closes the channel). The writer task exits. `JoinHandle` resolves without panic.

### AC-8: Backpressure (bounded channel)

- **Test**: Create a writer with a channel capacity of 1. Fill the channel without awaiting responses. The next send blocks (does not complete immediately). This verifies the bounded channel provides backpressure. (Use `tokio::time::timeout` to detect blocking.)

### AC-9: Error propagation

- **Test**: Send an append with an event exceeding `MAX_EVENT_SIZE`. The oneshot response contains `Err(Error::EventTooLarge)`.
- **Test**: After an error, subsequent valid appends still succeed (the writer is not poisoned).

### AC-10: Build and lint

- `cargo build` completes with zero warnings.
- `cargo clippy --all-targets --all-features --locked -- -D warnings` passes.
- `cargo fmt --check` passes.
- `cargo test` passes with all tests green.

## Dependencies

- **Depends on**: PRD 001 (types), PRD 002 (codec), PRD 003 (store).
- **Depended on by**: PRD 005, 006, 007.

## Cargo.toml Additions

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
```
