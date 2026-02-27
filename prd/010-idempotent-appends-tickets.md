# Tickets for PRD 010: Idempotent Appends

**Source PRD:** prd/010-idempotent-appends.md
**Created:** 2026-02-26
**Total Tickets:** 5
**Estimated Total Complexity:** 11 (S=1, M=2, L=3 → S+M+M+L+S = 1+2+2+3+1 = 9... see below)

> Complexity key: S=1, M=2, L=3.
> Ticket 1 (M=2) + Ticket 2 (L=3) + Ticket 3 (M=2) + Ticket 4 (M=2) + Ticket 5 (S=1) = **10**

---

### Ticket 1: Add `DedupIndex` module (`src/dedup.rs`)

**Description:**
Create the new `src/dedup.rs` module containing the `DedupIndex` struct backed by an
`lru::LruCache`. Implement `new`, `check`, `record`, and `seed_from_log` with full unit
tests. Register the module in `src/lib.rs` and add the `lru = "0.12"` dependency to
`Cargo.toml`. No callers exist yet; this ticket produces a self-contained, testable unit.

**Scope:**
- Create: `src/dedup.rs`
- Modify: `src/lib.rs` (add `pub(crate) mod dedup;`)
- Modify: `Cargo.toml` (add `lru = "0.12"` under `[dependencies]`)

**Acceptance Criteria:**
- [ ] `DedupIndex` struct has a private `cache: lru::LruCache<Uuid, Arc<Vec<RecordedEvent>>>` field and derives nothing that requires `RecordedEvent: Clone` beyond what already exists.
- [ ] `DedupIndex::new(capacity: NonZeroUsize) -> Self` constructs with the given capacity.
- [ ] `DedupIndex::check(&self, proposed: &[ProposedEvent]) -> Option<Arc<Vec<RecordedEvent>>>` checks only the first event's `event_id` (batch-level dedup key); returns `None` for an empty slice.
- [ ] `DedupIndex::record(&mut self, recorded: Vec<RecordedEvent>)` inserts one `Arc<Vec<RecordedEvent>>` shared across all per-event-ID entries in the batch; each `event_id` in `recorded` gets its own cache entry pointing to the same `Arc`.
- [ ] `DedupIndex::seed_from_log(&mut self, events: &[RecordedEvent])` inserts events in ascending global-position order so the highest-position events are LRU-hottest on completion.
- [ ] All public items have doc comments.
- [ ] Test: `DedupIndex::new(NonZeroUsize::new(4).unwrap())` -> `check(&[])` returns `None`. No panics.
- [ ] Test: `record` a batch of two events (IDs A, B) -> `check` with first event ID A returns `Some(arc)` where `arc.len() == 2`; `check` with event ID B also returns `Some` pointing to the same `Arc` (same pointer address).
- [ ] Test: `check` on a proposed batch whose first event_id was NOT recorded returns `None`.
- [ ] Test: fill the cache to capacity (N=2) with two separate single-event batches (IDs X, Y); then `record` a third batch (ID Z) which evicts the LRU entry; `check` for the evicted ID returns `None` while `check` for the still-cached IDs returns `Some`.
- [ ] Test: `seed_from_log` with 5 events (positions 0–4), capacity=3 -> only positions 2, 3, 4 remain in the index; `check` for the event_id at position 0 returns `None`.
- [ ] Test: `seed_from_log` followed by `check` for a seeded event_id returns `Some` with the correct `RecordedEvent` data (stream_id, global_position match).
- [ ] Quality gates pass: `cargo build`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo fmt --check`, `cargo test`.

**Dependencies:** None
**Complexity:** M
**Maps to PRD AC:** AC 1 (partial), AC 2 (partial), AC 6, AC 7

---

### Ticket 2: Integrate `DedupIndex` into the writer task (`src/writer.rs`)

**Description:**
Thread `DedupIndex` through `run_writer` and `spawn_writer`. In `run_writer`, before each
`store.append()` call, check the dedup index; on a hit return the cached result immediately
without writing to disk or publishing to the broker. On a miss, call `store.append()` and on
success call `dedup_index.record(...)`. In `spawn_writer`, accept a `NonZeroUsize`
`dedup_capacity` parameter, construct `DedupIndex`, seed it from `store.log()`, then enter
the writer loop.

**Scope:**
- Modify: `src/writer.rs` (`run_writer` signature + loop body; `spawn_writer` signature + body)

**Acceptance Criteria:**
- [ ] `run_writer` signature gains a `dedup: &mut DedupIndex` (or owned, passed by the spawned closure) parameter -- the dedup index lives entirely inside the writer task (no `Arc`, no `Mutex`).
- [ ] Within `run_writer`'s per-request loop: `dedup.check(&req.events)` is called before `store.append()`; a `Some(recorded)` hit sends the cached `Vec<RecordedEvent>` back via `response_tx` and skips both `store.append()` and `broker.publish()`.
- [ ] On a successful `store.append()`, `dedup.record(recorded.clone())` is called before `broker.publish()`.
- [ ] `spawn_writer` signature changes to `(store, channel_capacity, broker, dedup_capacity: NonZeroUsize) -> (WriterHandle, ReadIndex, JoinHandle<()>)`.
- [ ] Inside `spawn_writer`, `DedupIndex::new(dedup_capacity)` is constructed and `seed_from_log` is called with the events slice from `store.log().read()` before entering the writer loop.
- [ ] All existing `spawn_writer` call sites in `src/writer.rs` tests are updated to pass a `NonZeroUsize` dedup_capacity.
- [ ] Test (unit, in `src/writer.rs`): append a batch, then append the same batch again (identical event IDs) using `spawn_writer`; the second call returns `Ok` with the same `global_position` values as the first.
- [ ] Test (unit, in `src/writer.rs`): after a dedup hit, `ReadIndex::read_all(0, 1000)` returns the same count as after the first append (no duplicate events in the log).
- [ ] Test (unit, in `src/writer.rs`): after a dedup hit, the broker receives no new messages (subscribe before both appends, drain after first, assert empty after second).
- [ ] Test (unit, in `src/writer.rs`): a batch where two events share the same `event_id` is rejected with `Error::InvalidArgument` before any write; the writer is not poisoned and accepts a subsequent valid append.
- [ ] Quality gates pass.

**Dependencies:** Ticket 1
**Complexity:** L
**Maps to PRD AC:** AC 1, AC 2, AC 3, AC 4, AC 5, AC 8

---

### Ticket 3: Wire `dedup_capacity` into server config (`src/main.rs`)

**Description:**
Add `dedup_capacity: NonZeroUsize` to the `Config` struct in `src/main.rs`, read it from
the `EVENTFOLD_DEDUP_CAPACITY` environment variable (default: `NonZeroUsize::new(65536)`),
and pass it through to `spawn_writer`. Update the existing `Config` unit tests to cover the
new field's default and custom parse paths.

**Scope:**
- Modify: `src/main.rs` (`Config` struct, `Config::from_env`, `main`, and existing test suite)

**Acceptance Criteria:**
- [ ] `Config` struct has a `dedup_capacity: NonZeroUsize` field.
- [ ] `Config::from_env()` defaults `dedup_capacity` to `NonZeroUsize::new(65536).unwrap()` when `EVENTFOLD_DEDUP_CAPACITY` is unset.
- [ ] `Config::from_env()` parses `EVENTFOLD_DEDUP_CAPACITY` as `usize` then constructs `NonZeroUsize`; returns `Err(String)` if the value is not a valid nonzero usize (0 and non-numeric are both errors).
- [ ] `main()` passes `config.dedup_capacity` to `spawn_writer` as the new fourth argument.
- [ ] Doc comment on `Config` struct and `from_env` are updated to document the new env var.
- [ ] Test: `EVENTFOLD_DEDUP_CAPACITY` unset -> `config.dedup_capacity == NonZeroUsize::new(65536).unwrap()`.
- [ ] Test: `EVENTFOLD_DEDUP_CAPACITY=128` -> `config.dedup_capacity == NonZeroUsize::new(128).unwrap()`.
- [ ] Test: `EVENTFOLD_DEDUP_CAPACITY=0` -> `from_env()` returns `Err`.
- [ ] Test: `EVENTFOLD_DEDUP_CAPACITY=not-a-number` -> `from_env()` returns `Err` (existing style: assert `result.is_err()`).
- [ ] Quality gates pass.

**Dependencies:** Ticket 2
**Complexity:** M
**Maps to PRD AC:** AC 9 (partial — config wiring)

---

### Ticket 4: Integration tests for idempotent appends (`tests/idempotent_appends.rs`)

**Description:**
Write a dedicated integration test file that exercises the full dedup pipeline end-to-end
via the gRPC client against a real in-process server (following the pattern of
`tests/grpc_service.rs`). Covers restart-survival (AC-6), capacity eviction (AC-7), and
any cross-batch independence scenarios not already unit-tested inside `src/writer.rs`.

**Scope:**
- Create: `tests/idempotent_appends.rs`

**Acceptance Criteria:**
- [ ] Test helper `start_server(dedup_capacity)` starts an in-process gRPC server with the given capacity and returns a connected `EventStoreClient` + shutdown handle (follow the pattern in `tests/grpc_service.rs`).
- [ ] Test (AC-2 end-to-end): append batch with 2 events via gRPC; append same batch again; second response is `Ok`; both responses have identical `global_position` and `stream_version` for each event.
- [ ] Test (AC-3 end-to-end): after duplicate append, issue `ReadAll(from=0, max=1000)`; count equals the count after the first append only (no duplicate records in log).
- [ ] Test (AC-4 end-to-end): append batch A (IDs a1, a2) then batch B (different IDs b1, b2) to the same stream; both succeed independently; `ReadStream` returns 4 events total.
- [ ] Test (AC-6 restart): append batch, drop writer (simulate restart by re-opening `Store` and calling `spawn_writer` with seeded `DedupIndex`), then re-send the same batch; response is `Ok` with original positions (not new records).
- [ ] Test (AC-7 eviction): set `dedup_capacity=2`; append 3 separate single-event batches (IDs id1, id2, id3) so that id1 is evicted; re-send the batch with id1 -> appended as new (global_position > original); re-send batch with id3 -> dedup hit with original position.
- [ ] Test (AC-8 no broker): subscribe before first append; append batch; drain subscription; append same batch again; assert no additional messages received from broker.
- [ ] All tests use `tempfile::tempdir()` for isolation; no fixed paths.
- [ ] Quality gates pass.

**Dependencies:** Tickets 1, 2, 3
**Complexity:** L
**Maps to PRD AC:** AC 2, AC 3, AC 4, AC 6, AC 7, AC 8

---

### Ticket 5: Verification and integration check

**Description:**
Run the complete quality gate and verify all PRD acceptance criteria are covered by the
combined test suite. Confirm no regressions in any pre-existing tests. Check that the
`DedupIndex` module doc, `src/writer.rs` doc, and `src/main.rs` doc all reflect the new
behavior accurately.

**Acceptance Criteria:**
- [ ] `cargo build` exits 0 with zero warnings.
- [ ] `cargo clippy --all-targets --all-features --locked -- -D warnings` exits 0 with zero warnings.
- [ ] `cargo fmt --check` exits 0.
- [ ] `cargo test` exits 0 (all tests green, including pre-existing tests in `tests/grpc_service.rs`, `tests/server_binary.rs`, `tests/writer_integration.rs`, `tests/broker_integration.rs`).
- [ ] All 9 PRD acceptance criteria are covered by at least one passing test (verified against the AC Coverage Matrix below).
- [ ] No commented-out code, no `dbg!` macros, no `println!` debug statements remain in any modified file.
- [ ] All public items in `src/dedup.rs` have doc comments.

**Dependencies:** All previous tickets
**Complexity:** S
**Maps to PRD AC:** AC 1–9

---

## AC Coverage Matrix

| PRD AC # | Description                                                                                                        | Covered By Ticket(s)     | Status  |
|----------|--------------------------------------------------------------------------------------------------------------------|--------------------------|---------|
| 1        | Fresh append succeeds and returns newly assigned global positions, unchanged from pre-feature behavior              | Ticket 1, Ticket 2       | Covered |
| 2        | Duplicate batch (same event IDs) returns `Ok` with original positions; no new records written to log              | Ticket 2, Ticket 4       | Covered |
| 3        | After a dedup hit, `ReadAll` and `ReadStream` return the same event count as after the first append               | Ticket 2, Ticket 4       | Covered |
| 4        | Two separate batches with different event IDs succeed independently with no interference                           | Ticket 2, Ticket 4       | Covered |
| 5        | Single batch containing two events with the same `event_id` is rejected with `Error::InvalidArgument`             | Ticket 2                 | Covered |
| 6        | After simulated restart (re-open Store, seed DedupIndex, spawn new writer), re-send returns original positions    | Ticket 1 (seed), Ticket 4 | Covered |
| 7        | When log has more events than capacity, index holds exactly `capacity` most-recent entries; older IDs not deduped | Ticket 1 (seed), Ticket 4 | Covered |
| 8        | A dedup hit does not publish to the broadcast broker                                                               | Ticket 2, Ticket 4       | Covered |
| 9        | `cargo build` zero warnings; clippy passes; fmt passes; all tests green                                            | Ticket 5                 | Covered |
