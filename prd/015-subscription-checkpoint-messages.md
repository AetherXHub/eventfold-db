# PRD 015: Subscription Checkpoint Messages

**Status:** DRAFT
**Created:** 2026-02-26
**Author:** PRD Writer Agent

---

## Problem Statement

Projection services that subscribe to EventfoldDB must persist their own checkpoint (the last
processed global position or stream version) so they can resume from the correct place after a
restart or lag-triggered re-subscription. Currently the subscription protocol offers no signal for
when to checkpoint: the service must infer a safe point from the event positions it receives. Adding
periodic `Checkpoint` messages gives projection services an explicit, reliable signal to persist
their progress without polling, guessing, or writing a checkpoint after every single event.

## Goals

- Extend the `SubscribeAll` and `SubscribeStream` response protocol with a third message variant,
  `Checkpoint`, that the server sends periodically during the live phase.
- Provide two orthogonal triggering strategies: event-count-based (every N live events) and
  idle-time-based (every T seconds when no events have arrived).
- Make both thresholds configurable via environment variables with sensible defaults (N = 100
  events, T = 5 seconds), following the existing `Config` pattern in `src/main.rs`.
- Keep the storage engine, writer task, and broker entirely unchanged.

## Non-Goals

- Checkpoint messages during the catch-up phase. Only the live phase (after `CaughtUp`) produces
  checkpoint messages.
- Server-side persistence of checkpoint state. Checkpoints are hints to the client; the server
  remains stateless with respect to subscriber progress.
- Checkpoint acknowledgment or ack/nack. Clients process or ignore checkpoint messages silently.
- Changing the `CaughtUp` marker into a checkpoint. `CaughtUp` retains its existing meaning and
  position in the stream.
- Any change to `ReadAll`, `ReadStream`, or `Append` RPCs.
- Clustering, replication, or multi-subscriber coordination.
- Clients that do not wish to use checkpoints require no code change beyond ignoring the new
  `oneof` variant; no behavior changes for existing clients that pattern-match exhaustively.

## User Stories

- As a projection service author, I want to receive a `Checkpoint` message every N live events so
  that I know when to persist my progress without writing a checkpoint after every single event.
- As a projection service author, I want to receive a `Checkpoint` message after T seconds of
  inactivity so that my checkpoint stays reasonably fresh even when the event rate is low.
- As an operator, I want to tune the checkpoint interval and timeout via environment variables so
  that I can match the checkpoint frequency to the latency requirements of my deployment.
- As a developer using the console TUI, I want checkpoint markers to appear in the live tail view
  so that I can verify the server is emitting them correctly.

## Technical Approach

### Overview

The change touches five areas: the `.proto` file, Rust domain types (`types.rs`), the broker
subscription logic (`broker.rs`), the gRPC service layer (`service.rs`), and server config
(`main.rs`). The console TUI (`eventfold-console`) needs a display update for the live tail view.
The storage engine (`store.rs`, `writer.rs`) and `codec.rs` are unaffected.

### File-Change Table

| File | Change |
|------|--------|
| `proto/eventfold.proto` | Add `Checkpoint` variant to `SubscribeResponse.content` oneof |
| `src/types.rs` | Add `Checkpoint { position: u64 }` variant to `SubscriptionMessage` |
| `src/broker.rs` | Add `CheckpointConfig`; thread it through `subscribe_all` / `subscribe_stream`; inject checkpoint emission logic in the live phase |
| `src/service.rs` | Handle `SubscriptionMessage::Checkpoint` in both subscription RPC stream loops |
| `src/main.rs` | Add `EVENTFOLD_CHECKPOINT_INTERVAL` and `EVENTFOLD_CHECKPOINT_TIMEOUT_SECS` to `Config`; pass `CheckpointConfig` to `subscribe_all` / `subscribe_stream` |
| `eventfold-console/src/views/live_tail.rs` | Render checkpoint markers in the live tail view |

### Proto change

Add a `Checkpoint` message and a third `oneof` arm to `SubscribeResponse`:

```proto
message CheckpointPosition {
    uint64 position = 1;
}

message SubscribeResponse {
    oneof content {
        RecordedEvent event = 1;
        Empty caught_up = 2;
        CheckpointPosition checkpoint = 3;
    }
}
```

`position` carries global position for `SubscribeAll` and stream version for `SubscribeStream`,
matching the natural coordinate space of each subscription type.

### Domain type change

In `src/types.rs`, add to `SubscriptionMessage`:

```rust
/// A periodic checkpoint emitted during the live phase.
///
/// For `SubscribeAll`, `position` is the global position of the last delivered event.
/// For `SubscribeStream`, `position` is the stream version of the last delivered event.
/// Clients should persist this value as their resume checkpoint.
Checkpoint { position: u64 },
```

### Checkpoint configuration

Add a `CheckpointConfig` struct in `src/broker.rs`:

```rust
/// Configuration for periodic checkpoint emission during the live phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckpointConfig {
    /// Emit a checkpoint after this many live events have been delivered.
    pub interval: u64,
    /// Emit a checkpoint after this many seconds with no live events.
    pub timeout_secs: u64,
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self { interval: 100, timeout_secs: 5 }
    }
}
```

### Broker changes

Update `subscribe_all` and `subscribe_stream` signatures to accept `CheckpointConfig`:

```rust
pub async fn subscribe_all(
    read_index: ReadIndex,
    broker: &Broker,
    from_position: u64,
    checkpoint_cfg: CheckpointConfig,
) -> impl Stream<Item = Result<SubscriptionMessage, Error>>
```

In the live phase loop, maintain:
- `events_since_checkpoint: u64` -- reset to 0 after each checkpoint emission
- `last_event_position: Option<u64>` -- set on each delivered event

Replace the unconditional `rx.recv().await` with a `tokio::select!` that races:
- `rx.recv()` -- incoming event
- `tokio::time::sleep(Duration::from_secs(checkpoint_cfg.timeout_secs))` -- idle timer

On event receipt: increment `events_since_checkpoint`; if it reaches `checkpoint_cfg.interval`,
emit `SubscriptionMessage::Checkpoint { position }` and reset the counter. The idle timer arm fires
only when no event has been received within the timeout window; emit a checkpoint if
`last_event_position` is `Some` (i.e., at least one live event has been delivered), then continue
the loop (the select resets the sleep future naturally each iteration).

Checkpoint position values:
- `subscribe_all`: `arc_event.global_position`
- `subscribe_stream`: `arc_event.stream_version`

The idle timer must be re-created each loop iteration using `tokio::time::sleep` inside
`tokio::select!` so that the deadline resets after every received event.

### gRPC service changes

In `src/service.rs`, add a match arm for `SubscriptionMessage::Checkpoint` in both
`subscribe_all` and `subscribe_stream` stream loops:

```rust
Some(Ok(SubscriptionMessage::Checkpoint { position })) => {
    yield Ok(proto::SubscribeResponse {
        content: Some(proto::subscribe_response::Content::Checkpoint(
            proto::CheckpointPosition { position },
        )),
    });
}
```

`EventfoldService` must receive `CheckpointConfig` at construction time or via a new field. The
simplest approach: add `checkpoint_cfg: CheckpointConfig` as a field on `EventfoldService` and
pass it through to both subscription functions.

### Config changes

In `src/main.rs`, add to `Config`:

```rust
/// Number of live events between checkpoint messages.
checkpoint_interval: u64,
/// Seconds of idle time before a checkpoint is emitted.
checkpoint_timeout_secs: u64,
```

Parse from:
- `EVENTFOLD_CHECKPOINT_INTERVAL` (optional, default `100`)
- `EVENTFOLD_CHECKPOINT_TIMEOUT_SECS` (optional, default `5`)

Both must reject non-numeric values and values of `0` with a clear error message.

At startup, construct `CheckpointConfig` from parsed values and inject into `EventfoldService`.

### Console TUI changes

In `eventfold-console/src/views/live_tail.rs`, when the gRPC subscription stream yields a message
with `content = Checkpoint(pos)`, push a visual marker row into the `VecDeque` buffer:

```
[CHECKPOINT  pos=42]
```

The marker uses a distinct style (e.g., dim foreground) to differentiate it from event rows. It
counts against the 10,000-row buffer cap defined in PRD 009.

## Acceptance Criteria

1. `cargo build` at the workspace root produces zero warnings. `cargo clippy --all-targets
   --all-features --locked -- -D warnings` passes. `cargo fmt --check` passes. All pre-existing
   tests continue to pass.

2. `SubscribeResponse` in `proto/eventfold.proto` contains a `checkpoint` arm in its `oneof
   content` field. The generated Rust types expose
   `proto::subscribe_response::Content::Checkpoint(proto::CheckpointPosition { position })`.

3. `SubscriptionMessage` in `src/types.rs` contains a `Checkpoint { position: u64 }` variant. It
   derives `Debug` and `Clone` and is re-exported from `lib.rs`.

4. `CheckpointConfig` in `src/broker.rs` has `interval: u64` and `timeout_secs: u64` fields. Its
   `Default` impl produces `interval = 100` and `timeout_secs = 5`. Both `subscribe_all` and
   `subscribe_stream` accept a `CheckpointConfig` parameter.

5. A unit test in `src/broker.rs` verifies the count-based trigger: given a `CheckpointConfig`
   with `interval = 3` and a very long `timeout_secs`, appending 6 live events to a
   `subscribe_all` subscription produces exactly 2 `Checkpoint` messages (at positions 2 and 5),
   with no checkpoint before the 3rd or 6th event.

6. A unit test in `src/broker.rs` verifies the idle-time trigger: given a `CheckpointConfig` with
   `interval = 1000` and `timeout_secs = 1`, delivering 1 live event then waiting more than 1
   second causes a `Checkpoint` message to be emitted without any additional events arriving. Test
   uses `tokio::time::pause()` and `tokio::time::advance()` to control the clock.

7. A unit test in `src/broker.rs` verifies that no `Checkpoint` is emitted when the live phase
   has received zero events (i.e., the idle timer fires but `last_event_position` is `None`).

8. A unit test in `src/broker.rs` verifies that `subscribe_stream` emits a `Checkpoint` carrying
   `stream_version` (not `global_position`) after the count-based interval is reached.

9. `Config::from_env()` in `src/main.rs` parses `EVENTFOLD_CHECKPOINT_INTERVAL` and
   `EVENTFOLD_CHECKPOINT_TIMEOUT_SECS`. When both are unset, defaults to `interval = 100` and
   `timeout_secs = 5`. When either is set to `"0"` or a non-numeric value, `from_env()` returns
   `Err` with a message naming the offending variable.

10. An integration test in `tests/` subscribes to `SubscribeAll` via a live tonic client,
    configures `CheckpointConfig { interval: 5, timeout_secs: 60 }`, appends 5 events, and
    asserts that the gRPC stream yields exactly one `proto::subscribe_response::Content::Checkpoint`
    message after the 5 event messages (and after `CaughtUp`).

11. In `eventfold-console`, the live tail view renders a visually distinct row for checkpoint
    messages in the format `[CHECKPOINT  pos=N]`. The row is dim-styled and is included in the
    10,000-row buffer. Checkpoint rows do not appear in the catch-up phase (before the "Live"
    status indicator).

## Open Questions

- Should checkpoints be emitted when `subscribe_all` is called on an already-idle store (i.e.,
  immediately after `CaughtUp` with no events ever delivered live)? The current spec says no
  checkpoint fires if `last_event_position` is `None`, which means a projection on a quiescent
  store never gets a checkpoint. This is the safe default; it can be relaxed later.
- Should `checkpoint_interval = 0` be treated as "disable count-based checkpoints" rather than an
  error? The spec treats it as an error for simplicity; operators who want only the idle timer
  must use a very large interval. This can be revisited.
- The `CheckpointConfig` is currently passed through `subscribe_all` / `subscribe_stream` function
  parameters. If `EventfoldService` accumulates many config fields in future PRDs, a dedicated
  `ServiceConfig` struct may be cleaner. Not blocked on this.

## Dependencies

- **Depends on**: PRDs 001-009 (all complete: types, codec, store, writer, broker, gRPC service,
  server binary, batch hardening, console TUI workspace)
- **Depended on by**: Nothing currently — future client SDKs will want to consume checkpoint
  messages
- **External crates**: No new dependencies required. `tokio::time::sleep` and `tokio::select!` are
  already available via `tokio` with the `time` feature (already enabled).
