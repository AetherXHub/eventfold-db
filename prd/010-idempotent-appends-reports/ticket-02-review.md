# Code Review: Ticket 2 -- Integrate `DedupIndex` into the writer task (`src/writer.rs`)

**Ticket:** 2 -- Integrate `DedupIndex` into the writer task (`src/writer.rs`)
**Impl Report:** prd/010-idempotent-appends-reports/ticket-02-impl.md
**Date:** 2026-02-27 01:00
**Verdict:** APPROVED

---

## AC Coverage

| AC # | Description | Status | Notes |
|------|-------------|--------|-------|
| 1 | `run_writer` signature gains `dedup: &mut DedupIndex` | Met | Line 175: `dedup: &mut DedupIndex` parameter added to `run_writer`. |
| 2 | Dedup check before `store.append()`; hit returns cached, skips append+broker | Met | Lines 202-211: `dedup.check(&req.events)` called before line 214 `store.append()`. On `Some(cached)`, result is cloned from Arc and sent back via `response_tx`, then `continue` skips append and broker. |
| 3 | On successful append, `dedup.record(recorded.clone())` called before `broker.publish()` | Met | Lines 217-219: `dedup.record(recorded.clone())` on line 218 before `broker.publish(recorded)` on line 219. |
| 4 | `spawn_writer` signature: `(store, channel_capacity, broker, dedup_capacity: NonZeroUsize)` | Met | Lines 254-258: signature matches exactly. |
| 5 | Inside `spawn_writer`, `DedupIndex::new` constructed and `seed_from_log` called | Met | Lines 269-273: `DedupIndex::new(dedup_capacity)` followed by `dedup.seed_from_log(&log.events)` using events from the recovered log. |
| 6 | All existing `spawn_writer` call sites updated to pass `NonZeroUsize` dedup_capacity | Met | All unit tests in `src/writer.rs` use `test_dedup_cap()`. All integration tests (`broker_integration.rs`, `grpc_service.rs`, `server_binary.rs`, `writer_integration.rs`) add `test_dedup_cap()` helper and pass it. `src/main.rs` passes `config.dedup_capacity`. `src/broker.rs` tests pass `NonZeroUsize::new(128)`. |
| 7 | Test: dedup hit returns same `global_position` values | Met | `dedup_hit_returns_same_positions` (line 991): appends with fixed `event_id`, asserts second returns Ok with same `global_position`. |
| 8 | Test: after dedup hit, `read_all` returns same count (no duplicates) | Met | `dedup_hit_does_not_duplicate_events_in_log` (line 1030): appends twice with same `event_id`, asserts `read_all` returns exactly 1. |
| 9 | Test: after dedup hit, broker receives no new messages | Met | `dedup_hit_does_not_publish_to_broker` (line 1068): subscribes before both appends, drains after first, asserts `try_recv() == Empty` after dedup hit. |
| 10 | Test: duplicate event_id within batch rejected with `InvalidArgument`; writer not poisoned | Met | `duplicate_event_id_within_batch_rejected` (line 1117): sends batch with two identical event_ids, asserts `InvalidArgument` with "duplicate event_id" message, then verifies subsequent valid append succeeds. |
| 11 | Quality gates pass | Met | Verified: `cargo build` (0 warnings), `cargo clippy --all-targets --all-features --locked -- -D warnings` (clean), `cargo fmt --check` (clean), `cargo test` (220 passed, 0 failed). |

## Issues Found

### Critical (must fix before merge)
- None.

### Major (should fix, risk of downstream problems)
- None.

### Minor (nice to fix, not blocking)
1. **Scope extension into Ticket 3 territory (`src/main.rs`).** The implementer added `dedup_capacity: NonZeroUsize` to `Config`, `EVENTFOLD_DEDUP_CAPACITY` env var parsing in `from_env()`, `DEFAULT_DEDUP_CAPACITY` constant, and 2 new config tests (`from_env_defaults_when_only_data_set` updated, dedup capacity assertions). This is Ticket 3's scope. The justification (required for compilation after `spawn_writer` signature change) is valid, but it means Ticket 3 may be substantially complete before it starts. Not blocking.

2. **Scope extension into `src/dedup.rs` (Ticket 1 file).** The `check` method was changed from `&self` with `peek()` (Ticket 1 API) to `&mut self` with `get()` for LRU promotion on hit. This is a correctness improvement (retried batches stay warm in the LRU), but modifies the Ticket 1 API contract. Also removed `#![allow(dead_code)]`. Both changes are justified but technically out of Ticket 2's listed scope.

3. **Redundant `dedup_cap` variable in some tests.** Tests `dedup_hit_returns_same_positions`, `dedup_hit_does_not_duplicate_events_in_log`, `dedup_hit_does_not_publish_to_broker`, and `duplicate_event_id_within_batch_rejected` create a local `let dedup_cap = NonZeroUsize::new(128).expect("nonzero")` instead of calling `test_dedup_cap()` which is defined at line 289. The existing pre-Ticket-2 tests use `test_dedup_cap()`. This is a minor inconsistency.

## Suggestions (non-blocking)
- The new dedup tests could use the `test_dedup_cap()` helper (line 289) instead of inlining `NonZeroUsize::new(128).expect("nonzero")` for consistency with the existing tests.

## Scope Check
- Files within scope: YES for `src/writer.rs` (primary target)
- Out-of-scope files touched: `src/dedup.rs` (Ticket 1), `src/main.rs` (Ticket 3), `src/broker.rs` (broker tests), `tests/broker_integration.rs`, `tests/grpc_service.rs`, `tests/server_binary.rs`, `tests/writer_integration.rs`
- Scope creep detected: YES (minor) -- `src/main.rs` Config changes are Ticket 3 scope; `src/dedup.rs` `check` signature change modifies Ticket 1 API. Both are justified by compilation requirements and correctness. The integration test + broker test updates are mechanical (adding the new 4th argument) and necessary for compilation.
- Unauthorized dependencies added: NO

## Risk Assessment
- Regression risk: LOW -- All 220 pre-existing + new tests pass. The dedup check is purely additive; it returns early on hit (skipping the existing write path) and records on miss (after the existing write path). The existing write/read/subscribe flow is unchanged for non-duplicate appends.
- Security concerns: NONE
- Performance concerns: NONE -- `validate_batch_unique_ids` uses `HashSet::with_capacity` for O(n) uniqueness checking with pre-allocation. The dedup `check` is O(1) LRU lookup. The `record` call clones the `Vec<RecordedEvent>` once for the dedup index, which is unavoidable since the broker needs the original.
