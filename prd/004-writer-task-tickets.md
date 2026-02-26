# Tickets for PRD 004: Writer Task

**Source PRD:** prd/004-writer-task.md
**Created:** 2026-02-25
**Total Tickets:** 6
**Estimated Total Complexity:** 13 (S=1 + L=3 + M=2 + M=2 + M=2 + M=2)

---

### Ticket 1: Add `tokio` Dependency to `Cargo.toml`

**Description:**
Add `tokio` with `features = ["full"]` to `[dependencies]` in `Cargo.toml`. This is the only
change — no source files are modified. All subsequent tickets depend on tokio being present so
the crate compiles with async/await, `mpsc`, `oneshot`, `RwLock`, `spawn`, and `JoinHandle`.

**Scope:**
- Modify: `Cargo.toml` (add `tokio = { version = "1", features = ["full"] }` under `[dependencies]`)

**Acceptance Criteria:**
- [ ] `Cargo.toml` `[dependencies]` section contains `tokio = { version = "1", features = ["full"] }`
- [ ] Test: `cargo build` completes with zero errors and zero warnings after this change alone
- [ ] Test: `use tokio::sync::mpsc;` is valid in a `#[cfg(test)]` block — import compiles cleanly
- [ ] Quality gates pass: `cargo build`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo fmt --check`, `cargo test`

**Dependencies:** None
**Complexity:** S
**Maps to PRD AC:** AC-10 (partial: dependency prerequisite for all other ACs)

---

### Ticket 2: Introduce `EventLog` Shared State and Refactor `Store` to Use `Arc<RwLock<EventLog>>`

**Description:**
Create the `EventLog` struct that holds the two in-memory index structures (`events` and
`streams`), wrap it in `Arc<tokio::sync::RwLock<EventLog>>` inside `Store`, and refactor all
existing `Store` methods (`open`, `append`, `read_stream`, `read_all`, `stream_version`,
`global_position`) to operate through the lock. All existing `store.rs` unit tests must remain
green after this refactor. The `Store` type remains synchronous (no async methods yet) because
`append` runs inside the writer task, which calls blocking methods via `spawn_blocking` or
inline within the task's own thread — the RwLock chosen is `parking_lot::RwLock` (or
`std::sync::RwLock`) rather than `tokio::sync::RwLock` so that `Store::append` stays `fn`
(not `async fn`). The `Arc` clone of the inner `RwLock<EventLog>` is what `ReadIndex` will
hold (Ticket 3).

**Implementer note:** The PRD allows `parking_lot::RwLock` or `tokio::sync::RwLock`. Prefer
`std::sync::RwLock` wrapped in `Arc` to avoid adding a new dependency and to keep `Store`
methods synchronous. The writer task is the only writer; read handlers only acquire the read
lock. This satisfies the PRD requirement that "the writer holds a write lock only during index
updates (after fsync, not during I/O)."

**Scope:**
- Modify: `src/store.rs` — add `EventLog` struct, change `Store` fields to hold
  `Arc<std::sync::RwLock<EventLog>>` and `File`, refactor all methods to read/write through
  the lock
- Modify: `Cargo.toml` — no new dependency needed if using `std::sync::RwLock`

**Acceptance Criteria:**
- [ ] `EventLog` struct is `pub` with fields `events: Vec<RecordedEvent>` and
  `streams: HashMap<Uuid, Vec<u64>>`, deriving `Debug`
- [ ] `Store` struct fields: `file: File` and `log: Arc<std::sync::RwLock<EventLog>>`
  (the `events` and `streams` fields are removed from `Store` directly)
- [ ] `Store::open(path: &Path) -> Result<Store, Error>` — same behavior as before; new-file
  path initializes `EventLog` with empty `Vec` and `HashMap` wrapped in `Arc<RwLock>`; existing-
  file path rebuilds the same structures during recovery then wraps them
- [ ] `Store::append(&mut self, ...)` acquires the write lock only after fsync succeeds (not
  during disk I/O); the disk-write logic is unchanged
- [ ] `Store::read_stream`, `Store::read_all`, `Store::stream_version`,
  `Store::global_position` acquire a read lock and return results; signatures are unchanged
- [ ] `Store` exposes a `pub fn log(&self) -> Arc<std::sync::RwLock<EventLog>>` method that
  returns a clone of the inner `Arc` for use by `ReadIndex`
- [ ] Test: all existing `store.rs` unit tests pass unchanged after the refactor — no new tests
  required in this ticket (the behavioral contract is preserved, not changed)
- [ ] Test: `Store::open` on a new path -> `store.log()` returns an `Arc` whose `read()` lock
  yields an `EventLog` with `events.len() == 0` and `streams.len() == 0`
- [ ] Test: after appending one event, `store.log()` read-locked `EventLog` has
  `events.len() == 1` and `streams` contains the stream's UUID
- [ ] Quality gates pass: `cargo build`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo fmt --check`, `cargo test`

**Dependencies:** Ticket 1
**Complexity:** L
**Maps to PRD AC:** AC-5 (partial: shared state prerequisite), AC-6 (partial: ReadIndex
prerequisite)

---

### Ticket 3: `ReadIndex` — Shared Read-Only View of the In-Memory Index

**Description:**
Create `src/reader.rs` (or add to `src/store.rs` as a sibling type) containing the `ReadIndex`
struct, which holds an `Arc<std::sync::RwLock<EventLog>>` and exposes `read_stream`,
`read_all`, `stream_version`, and `global_position` as read-only methods. This is the handle
that gRPC read handlers will hold — it allows concurrent reads from multiple threads without
going through the writer task. Declare `pub mod reader` in `lib.rs` and re-export `ReadIndex`.

**Scope:**
- Create: `src/reader.rs` (the `ReadIndex` struct and its read methods)
- Modify: `src/lib.rs` (add `pub mod reader;` and `pub use reader::ReadIndex;`)

**Acceptance Criteria:**
- [ ] `ReadIndex` struct is `pub`, `Clone`, `Debug`, holds `log: Arc<std::sync::RwLock<EventLog>>`
- [ ] `ReadIndex::new(log: Arc<std::sync::RwLock<EventLog>>) -> ReadIndex` constructor
- [ ] `ReadIndex::read_all(&self, from_position: u64, max_count: u64) -> Vec<RecordedEvent>` —
  acquires read lock, delegates to same slice logic as `Store::read_all`
- [ ] `ReadIndex::read_stream(&self, stream_id: Uuid, from_version: u64, max_count: u64) -> Result<Vec<RecordedEvent>, Error>` — acquires read lock, returns `StreamNotFound` if absent
- [ ] `ReadIndex::stream_version(&self, stream_id: &Uuid) -> Option<u64>` — read-locked lookup
- [ ] `ReadIndex::global_position(&self) -> u64` — read-locked `events.len() as u64`
- [ ] `ReadIndex` is `Clone` — cloning produces a new handle backed by the same `Arc`
- [ ] Test: create a `Store`, append 3 events, call `store.log()` to get the Arc, construct a
  `ReadIndex` from it, call `read_all(0, 100)` -> returns all 3 events
- [ ] Test: `ReadIndex::read_stream` on an appended stream -> returns correct events in version
  order
- [ ] Test: `ReadIndex::read_stream` on a non-existent UUID -> returns
  `Err(Error::StreamNotFound { .. })`
- [ ] Test: two `ReadIndex` clones backed by the same `Arc` both observe the same data after
  an append through `Store`
- [ ] Quality gates pass: `cargo build`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo fmt --check`, `cargo test`

**Dependencies:** Ticket 2
**Complexity:** M
**Maps to PRD AC:** AC-5

---

### Ticket 4: `AppendRequest` and `WriterHandle` Types in `src/writer.rs`

**Description:**
Create `src/writer.rs` with the `AppendRequest` struct and the `WriterHandle` struct. Neither
requires the full writer task to be running yet — `WriterHandle::append` is an async method
that simply packages the request and sends it over the mpsc channel, then awaits the oneshot
response. Declare `pub mod writer` in `lib.rs` and re-export `WriterHandle`. Add unit tests
for `AppendRequest` field presence and for the `WriterHandle::append` call being sendable
(loopback test using a test receiver).

**Scope:**
- Create: `src/writer.rs` (`AppendRequest`, `WriterHandle`, and their tests)
- Modify: `src/lib.rs` (add `pub mod writer;` and `pub use writer::WriterHandle;`)

**Acceptance Criteria:**
- [ ] `AppendRequest` struct has fields: `stream_id: Uuid`, `expected_version: ExpectedVersion`,
  `events: Vec<ProposedEvent>`, `response_tx: tokio::sync::oneshot::Sender<Result<Vec<RecordedEvent>, Error>>`
- [ ] `WriterHandle` struct is `pub`, derives `Clone`, holds `tx: tokio::sync::mpsc::Sender<AppendRequest>`
- [ ] `WriterHandle::append(&self, stream_id: Uuid, expected_version: ExpectedVersion, events: Vec<ProposedEvent>) -> Result<Vec<RecordedEvent>, Error>` is `async` — creates a oneshot channel, sends `AppendRequest`, awaits the receiver, and maps a closed-channel error to `Error::InvalidArgument("writer task closed".into())`
- [ ] Test: create an `mpsc::channel(8)`, construct a `WriterHandle` from the sender; in a
  `tokio::spawn` task, receive the `AppendRequest` from the receiver, assert its fields match
  what was sent, then reply via `response_tx.send(Ok(vec![]))` — the `WriterHandle::append`
  call returns `Ok(vec![])`
- [ ] Test: drop the mpsc receiver before calling `WriterHandle::append` -> returns
  `Err(Error::InvalidArgument(..))`  (channel send fails, oneshot never replied)
- [ ] Test: `WriterHandle::clone()` produces a second handle; both can independently send
  requests to the same channel
- [ ] Quality gates pass: `cargo build`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo fmt --check`, `cargo test`

**Dependencies:** Ticket 1, Ticket 2 (for `AppendRequest` to reference `Store`-related types)
**Complexity:** M
**Maps to PRD AC:** AC-1 (partial: handle construction), AC-3 (partial: handle is Clone),
AC-4 (partial: request types carry ExpectedVersion), AC-8 (partial: channel plumbing)

---

### Ticket 5: `run_writer` Task Loop and `spawn_writer` in `src/writer.rs`

**Description:**
Implement the `run_writer` async function (the writer task loop) and the `spawn_writer`
function that creates the channel, spawns the task, and returns a `(WriterHandle, ReadIndex,
JoinHandle<()>)` tuple. The task loop: receive the first request blocking on `recv()`, drain
additional pending requests with `try_recv()`, process each by calling `store.append()`, and
send each result back via its `response_tx`. Exit when the channel is closed. This ticket
covers all behavioral acceptance criteria (AC-1 through AC-9).

**Scope:**
- Modify: `src/writer.rs` (add `run_writer`, `spawn_writer`, and all writer integration tests)
- Modify: `src/lib.rs` (re-export `spawn_writer`)

**Acceptance Criteria:**
- [ ] `run_writer(store: Store, rx: tokio::sync::mpsc::Receiver<AppendRequest>)` is `pub(crate)` `async fn` — loops on `rx.recv()`, drains with `rx.try_recv()` into a local `Vec`, processes each request by calling `store.append(req.stream_id, req.expected_version, req.events)`, sends the result back via `req.response_tx.send(result)` (log a `tracing::warn!` if the receiver was already dropped), and exits cleanly when `rx.recv()` returns `None`
- [ ] `spawn_writer(store: Store, channel_capacity: usize) -> (WriterHandle, ReadIndex, tokio::task::JoinHandle<()>)` is `pub` — calls `store.log()` to clone the Arc before moving `store` into the task; constructs `ReadIndex::new(arc)` and `WriterHandle { tx }`, spawns `run_writer` via `tokio::spawn`, returns the triple
- [ ] Test (AC-1): `#[tokio::test]` — `spawn_writer` with a tempdir store, capacity 8; call
  `writer_handle.append(stream_id, ExpectedVersion::NoStream, vec![event])` -> result is `Ok`;
  returned `RecordedEvent` has `global_position == 0` and `stream_version == 0`
- [ ] Test (AC-2): `#[tokio::test]` — send 3 sequential appends (await each); global positions
  are 0, 1, 2; stream versions within the same stream are 0, 1, 2
- [ ] Test (AC-3): `#[tokio::test]` — `tokio::spawn` 10 concurrent `writer_handle.append` calls
  with `ExpectedVersion::Any`; join all; collect all `global_position` values; the set equals
  `{0..9}` with no duplicates and no errors
- [ ] Test (AC-4a): append one event with `NoStream`, then append to the same stream with
  `NoStream` again -> second call returns `Err(Error::WrongExpectedVersion { .. })`
- [ ] Test (AC-4b): append one event with `NoStream`, then with `Exact(0)` -> `Ok`
- [ ] Test (AC-4c): append one event with `NoStream`, then with `Exact(5)` ->
  `Err(Error::WrongExpectedVersion { .. })`
- [ ] Test (AC-5): append 3 events via `writer_handle`; call `read_index.read_all(0, 100)` ->
  returns 3 events; call `read_index.read_stream(stream_id, 0, 100)` -> returns correct events
- [ ] Test (AC-6): spawn writer, append 5 events, drop `writer_handle`, await `join_handle`,
  open a second `Store::open` at same tempdir path -> `read_all(0, 100)` on a fresh
  `ReadIndex` from the new store returns all 5 events
- [ ] Test (AC-7): spawn writer, drop all `WriterHandle` clones (the one from `spawn_writer`);
  `join_handle.await` resolves without panic within 1 second
- [ ] Test (AC-8): `spawn_writer` with `channel_capacity = 1`; hold the write lock on the
  store's EventLog (simulate a slow append) by... **Implementer note:** simulating backpressure
  precisely requires the task to be stalled. Instead, verify the channel is bounded: send one
  request without awaiting the response (use `WriterHandle`'s inner `tx.try_send` directly in
  the test, or use `tokio::time::timeout(Duration::from_millis(10), handle.append(...))` to
  verify the second send blocks when the channel is full)
- [ ] Test (AC-9a): append an event whose payload makes the total encoded size exceed
  `MAX_EVENT_SIZE` -> `Err(Error::EventTooLarge { .. })`
- [ ] Test (AC-9b): after the `EventTooLarge` error, a valid subsequent append to the same
  writer returns `Ok` (writer not poisoned)
- [ ] Quality gates pass: `cargo build`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo fmt --check`, `cargo test`

**Dependencies:** Tickets 1, 2, 3, 4
**Complexity:** M
**Maps to PRD AC:** AC-1, AC-2, AC-3, AC-4, AC-5, AC-6, AC-7, AC-8, AC-9

---

### Ticket 6: Verification and Integration

**Description:**
Run the complete PRD 004 acceptance criteria checklist end-to-end. Verify all six modules
(`types`, `error`, `codec`, `store`, `reader`, `writer`) integrate correctly as a cohesive
system. Confirm no regressions in PRD 001, 002, or 003 tests, and that all quality gates pass
clean.

**Acceptance Criteria:**
- [ ] All PRD 004 ACs (AC-1 through AC-9) verified by passing `cargo test` output
- [ ] `cargo test` output shows zero failures across the full crate (PRD 001–004 tests all green)
- [ ] `cargo build` completes with zero warnings
- [ ] `cargo clippy --all-targets --all-features --locked -- -D warnings` passes with zero
  diagnostics
- [ ] `cargo fmt --check` passes
- [ ] `WriterHandle`, `ReadIndex`, and `spawn_writer` are accessible at the crate root via
  `eventfold_db::WriterHandle`, `eventfold_db::ReadIndex`, `eventfold_db::spawn_writer`
- [ ] Test: import all three from the crate root in a `tests/` integration test file; call
  `spawn_writer` with a `Store::open` against a tempdir, append 2 events, read them back via
  `ReadIndex::read_all`, assert positions are 0 and 1

**Dependencies:** Tickets 1, 2, 3, 4, 5
**Complexity:** M
**Maps to PRD AC:** AC-10

---

## AC Coverage Matrix

| PRD AC # | Description                                                        | Covered By Ticket(s)   | Status  |
|----------|--------------------------------------------------------------------|------------------------|---------|
| AC-1     | Basic append through writer: correct global_position/stream_version | Ticket 4 (partial), Ticket 5 | Covered |
| AC-2     | Sequential consistency: 3 sequential appends have contiguous positions | Ticket 5           | Covered |
| AC-3     | Concurrent appends serialized: 10 concurrent, unique contiguous positions | Ticket 5        | Covered |
| AC-4     | ExpectedVersion enforcement: NoStream/Exact success and failure paths | Ticket 5           | Covered |
| AC-5     | ReadIndex reflects writes: read_all and read_stream after append   | Ticket 3, Ticket 5     | Covered |
| AC-6     | Durability (survives restart): 5 events recovered after store reopen | Ticket 5             | Covered |
| AC-7     | Graceful shutdown: task exits when all WriterHandle clones dropped | Ticket 5               | Covered |
| AC-8     | Backpressure (bounded channel): second send blocks on capacity=1   | Ticket 5               | Covered |
| AC-9     | Error propagation: EventTooLarge returned; writer not poisoned     | Ticket 5               | Covered |
| AC-10    | Build and lint: cargo build/clippy/fmt/test all pass               | Ticket 1, Ticket 6     | Covered |
