//! Integration tests for EventfoldDB idempotent (dedup) appends.
//!
//! Exercises the full dedup pipeline end-to-end via a gRPC client against a
//! real in-process server. Covers duplicate detection, restart survival,
//! capacity eviction, and broker silence on dedup hits.

use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::path::Path;

use eventfold_db::proto::event_store_client::EventStoreClient;
use eventfold_db::proto::event_store_server::EventStoreServer;
use eventfold_db::proto::{self, expected_version};
use eventfold_db::{Broker, EventfoldService, Store, WriterHandle, spawn_writer};
use tonic::transport::Channel;

/// Handle to a running test server for lifecycle control.
///
/// Provides a `shutdown()` method that stops the tonic server, drops the writer
/// handle, and awaits the writer task for clean teardown.
struct ServerHandle {
    /// Handle to the writer task's mpsc sender. Drop to close the writer channel.
    writer_handle: WriterHandle,
    /// JoinHandle for the writer task. Await after closing the channel.
    writer_join: tokio::task::JoinHandle<()>,
    /// JoinHandle for the tonic server task.
    server_join: tokio::task::JoinHandle<()>,
    /// Oneshot sender to trigger graceful shutdown of the tonic server.
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

impl ServerHandle {
    /// Shut down the server and writer task gracefully.
    async fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
        let _ = self.server_join.await;
        drop(self.writer_handle);
        let _ = self.writer_join.await;
    }
}

/// Start an in-process gRPC server with the given dedup capacity.
///
/// Opens a `Store` at `data_path`, spawns the writer task with the specified
/// `dedup_capacity`, and binds a tonic server on an ephemeral IPv6 loopback
/// port. Returns a connected gRPC client and a `ServerHandle` for shutdown.
///
/// # Arguments
///
/// * `data_path` - Path to the append-only log file.
/// * `dedup_capacity` - Maximum number of event IDs tracked in the dedup index.
///
/// # Panics
///
/// Panics if the store cannot be opened, the listener cannot bind, or the
/// client cannot connect.
async fn start_server(
    data_path: &Path,
    dedup_capacity: NonZeroUsize,
) -> (EventStoreClient<Channel>, ServerHandle) {
    let store = Store::open(data_path).expect("store open should succeed");
    let broker = Broker::new(1024);
    let (writer_handle, read_index, writer_join) =
        spawn_writer(store, 64, broker.clone(), dedup_capacity);

    let service = EventfoldService::new(writer_handle.clone(), read_index, broker);

    let listen_addr: SocketAddr = "[::1]:0".parse().expect("valid addr");
    let listener = tokio::net::TcpListener::bind(listen_addr)
        .await
        .expect("bind should succeed");
    let addr = listener.local_addr().expect("should have local addr");
    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let server_join = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(EventStoreServer::new(service))
            .serve_with_incoming_shutdown(incoming, async {
                let _ = shutdown_rx.await;
            })
            .await
            .expect("server should run");
    });

    // Give the server time to start accepting connections.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let client = EventStoreClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .expect("client connect should succeed");

    let handle = ServerHandle {
        writer_handle,
        writer_join,
        server_join,
        shutdown_tx,
    };

    (client, handle)
}

/// Start a simple in-process gRPC server (no shutdown handle) for tests that
/// do not need restart capability.
///
/// Follows the pattern from `tests/grpc_service.rs` using `serve_with_incoming`.
async fn start_simple_server(
    data_path: &Path,
    dedup_capacity: NonZeroUsize,
) -> EventStoreClient<Channel> {
    let store = Store::open(data_path).expect("store open should succeed");
    let broker = Broker::new(1024);
    let (writer_handle, read_index, _join_handle) =
        spawn_writer(store, 64, broker.clone(), dedup_capacity);

    let service = EventfoldService::new(writer_handle, read_index, broker);

    let listen_addr: SocketAddr = "[::1]:0".parse().expect("valid addr");
    let listener = tokio::net::TcpListener::bind(listen_addr)
        .await
        .expect("bind should succeed");
    let addr = listener.local_addr().expect("should have local addr");
    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

    tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(EventStoreServer::new(service))
            .serve_with_incoming(incoming)
            .await
            .expect("server should run");
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    EventStoreClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .expect("client connect should succeed")
}

/// Default dedup capacity for tests that do not exercise eviction.
fn default_dedup_cap() -> NonZeroUsize {
    NonZeroUsize::new(128).expect("nonzero")
}

/// Helper: create a proto ProposedEvent with a specific event_id string and event type.
fn make_proposed_with_id(event_id: &str, event_type: &str) -> proto::ProposedEvent {
    proto::ProposedEvent {
        event_id: event_id.to_string(),
        event_type: event_type.to_string(),
        metadata: vec![],
        payload: b"{}".to_vec(),
    }
}

/// Helper: create an ExpectedVersion::Any.
fn any_version() -> Option<proto::ExpectedVersion> {
    Some(proto::ExpectedVersion {
        kind: Some(expected_version::Kind::Any(proto::Empty {})),
    })
}

// -- Test (AC-2 end-to-end): Duplicate batch returns identical positions --

#[tokio::test]
async fn dedup_duplicate_batch_returns_identical_positions() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let data_path = dir.path().join("events.log");

    let (mut client, handle) = start_server(&data_path, default_dedup_cap()).await;

    let stream_id = uuid::Uuid::new_v4().to_string();
    let id1 = uuid::Uuid::new_v4().to_string();
    let id2 = uuid::Uuid::new_v4().to_string();

    let events = vec![
        make_proposed_with_id(&id1, "Evt1"),
        make_proposed_with_id(&id2, "Evt2"),
    ];

    // First append: creates events.
    let first_resp = client
        .append(proto::AppendRequest {
            stream_id: stream_id.clone(),
            expected_version: any_version(),
            events: events.clone(),
        })
        .await
        .expect("first append should succeed")
        .into_inner();

    // Second append: same batch (same event IDs) -- dedup hit.
    let second_resp = client
        .append(proto::AppendRequest {
            stream_id: stream_id.clone(),
            expected_version: any_version(),
            events,
        })
        .await
        .expect("dedup hit should return Ok")
        .into_inner();

    // Both responses must have identical positions.
    assert_eq!(
        first_resp.first_global_position,
        second_resp.first_global_position
    );
    assert_eq!(
        first_resp.last_global_position,
        second_resp.last_global_position
    );
    assert_eq!(
        first_resp.first_stream_version,
        second_resp.first_stream_version
    );
    assert_eq!(
        first_resp.last_stream_version,
        second_resp.last_stream_version
    );

    handle.shutdown().await;
}

// -- Test (AC-3 end-to-end): No duplicate records in log after dedup hit --

#[tokio::test]
async fn dedup_no_duplicate_records_in_log() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let data_path = dir.path().join("events.log");

    let (mut client, handle) = start_server(&data_path, default_dedup_cap()).await;

    let stream_id = uuid::Uuid::new_v4().to_string();
    let id1 = uuid::Uuid::new_v4().to_string();
    let id2 = uuid::Uuid::new_v4().to_string();

    let events = vec![
        make_proposed_with_id(&id1, "Evt1"),
        make_proposed_with_id(&id2, "Evt2"),
    ];

    // First append.
    client
        .append(proto::AppendRequest {
            stream_id: stream_id.clone(),
            expected_version: any_version(),
            events: events.clone(),
        })
        .await
        .expect("first append should succeed");

    // Second append (dedup hit).
    client
        .append(proto::AppendRequest {
            stream_id: stream_id.clone(),
            expected_version: any_version(),
            events,
        })
        .await
        .expect("dedup hit should return Ok");

    // ReadAll should return exactly 2 events (from the first append only).
    let read_resp = client
        .read_all(proto::ReadAllRequest {
            from_position: 0,
            max_count: 1000,
        })
        .await
        .expect("read_all should succeed")
        .into_inner();

    assert_eq!(
        read_resp.events.len(),
        2,
        "expected 2 events in log, got {}",
        read_resp.events.len()
    );

    handle.shutdown().await;
}

// -- Test (AC-4 end-to-end): Two different batches succeed independently --

#[tokio::test]
async fn dedup_different_batches_succeed_independently() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let data_path = dir.path().join("events.log");

    let (mut client, handle) = start_server(&data_path, default_dedup_cap()).await;

    let stream_id = uuid::Uuid::new_v4().to_string();
    let a1 = uuid::Uuid::new_v4().to_string();
    let a2 = uuid::Uuid::new_v4().to_string();
    let b1 = uuid::Uuid::new_v4().to_string();
    let b2 = uuid::Uuid::new_v4().to_string();

    // Batch A: events a1, a2.
    let resp_a = client
        .append(proto::AppendRequest {
            stream_id: stream_id.clone(),
            expected_version: any_version(),
            events: vec![
                make_proposed_with_id(&a1, "A1"),
                make_proposed_with_id(&a2, "A2"),
            ],
        })
        .await
        .expect("batch A should succeed")
        .into_inner();

    // Batch B: events b1, b2 (different IDs).
    let resp_b = client
        .append(proto::AppendRequest {
            stream_id: stream_id.clone(),
            expected_version: Some(proto::ExpectedVersion {
                kind: Some(expected_version::Kind::Exact(1)),
            }),
            events: vec![
                make_proposed_with_id(&b1, "B1"),
                make_proposed_with_id(&b2, "B2"),
            ],
        })
        .await
        .expect("batch B should succeed")
        .into_inner();

    // Batch A starts at global_position 0, batch B starts at 2.
    assert_eq!(resp_a.first_global_position, 0);
    assert_eq!(resp_a.last_global_position, 1);
    assert_eq!(resp_b.first_global_position, 2);
    assert_eq!(resp_b.last_global_position, 3);

    // ReadStream should return 4 events total.
    let read_resp = client
        .read_stream(proto::ReadStreamRequest {
            stream_id,
            from_version: 0,
            max_count: 100,
        })
        .await
        .expect("read_stream should succeed")
        .into_inner();

    assert_eq!(
        read_resp.events.len(),
        4,
        "expected 4 events in stream, got {}",
        read_resp.events.len()
    );

    handle.shutdown().await;
}

// -- Test (AC-6 restart): Dedup survives restart via seed_from_log --

#[tokio::test]
async fn dedup_survives_restart() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let data_path = dir.path().join("events.log");

    let stream_id = uuid::Uuid::new_v4().to_string();
    let id1 = uuid::Uuid::new_v4().to_string();

    // Use a single-event batch. After restart, seed_from_log seeds each
    // event individually, so single-event batches produce exact position
    // matches on dedup hits.
    let events = vec![make_proposed_with_id(&id1, "Evt1")];

    // First server: append the batch, then shut down.
    let first_resp = {
        let (mut client, handle) = start_server(&data_path, default_dedup_cap()).await;

        let resp = client
            .append(proto::AppendRequest {
                stream_id: stream_id.clone(),
                expected_version: any_version(),
                events: events.clone(),
            })
            .await
            .expect("first append should succeed")
            .into_inner();

        handle.shutdown().await;
        resp
    };

    // Second server: re-open at the same data path (simulates restart).
    // spawn_writer calls seed_from_log internally, which rebuilds the
    // dedup index from the on-disk log.
    {
        let (mut client, handle) = start_server(&data_path, default_dedup_cap()).await;

        // Re-send the same batch. The dedup index, seeded from the log,
        // should recognize the event ID and return the original position.
        let second_resp = client
            .append(proto::AppendRequest {
                stream_id: stream_id.clone(),
                expected_version: any_version(),
                events,
            })
            .await
            .expect("dedup hit after restart should return Ok")
            .into_inner();

        // Positions must match the original append (not new records).
        assert_eq!(
            first_resp.first_global_position,
            second_resp.first_global_position
        );
        assert_eq!(
            first_resp.last_global_position,
            second_resp.last_global_position
        );

        // ReadAll should still show only 1 event (no duplicates written).
        let read_resp = client
            .read_all(proto::ReadAllRequest {
                from_position: 0,
                max_count: 1000,
            })
            .await
            .expect("read_all should succeed")
            .into_inner();

        assert_eq!(
            read_resp.events.len(),
            1,
            "expected 1 event after restart dedup, got {}",
            read_resp.events.len()
        );

        handle.shutdown().await;
    }
}

// -- Test (AC-7 eviction): Evicted event IDs are appended as new --

#[tokio::test]
async fn dedup_eviction_allows_reappend() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let data_path = dir.path().join("events.log");

    // Dedup capacity of 2 event IDs.
    let small_cap = NonZeroUsize::new(2).expect("nonzero");
    let (mut client, handle) = start_server(&data_path, small_cap).await;

    let stream_id = uuid::Uuid::new_v4().to_string();
    let id1 = uuid::Uuid::new_v4().to_string();
    let id2 = uuid::Uuid::new_v4().to_string();
    let id3 = uuid::Uuid::new_v4().to_string();

    // Append 3 separate single-event batches so that id1 is evicted.
    // After batch 1: cache = {id1}
    // After batch 2: cache = {id1, id2}
    // After batch 3: cache = {id2, id3} (id1 evicted)
    let resp1 = client
        .append(proto::AppendRequest {
            stream_id: stream_id.clone(),
            expected_version: any_version(),
            events: vec![make_proposed_with_id(&id1, "Evt1")],
        })
        .await
        .expect("append id1 should succeed")
        .into_inner();

    client
        .append(proto::AppendRequest {
            stream_id: stream_id.clone(),
            expected_version: Some(proto::ExpectedVersion {
                kind: Some(expected_version::Kind::Exact(0)),
            }),
            events: vec![make_proposed_with_id(&id2, "Evt2")],
        })
        .await
        .expect("append id2 should succeed");

    let resp3 = client
        .append(proto::AppendRequest {
            stream_id: stream_id.clone(),
            expected_version: Some(proto::ExpectedVersion {
                kind: Some(expected_version::Kind::Exact(1)),
            }),
            events: vec![make_proposed_with_id(&id3, "Evt3")],
        })
        .await
        .expect("append id3 should succeed")
        .into_inner();

    // Re-send id1: should be appended as NEW (evicted from cache).
    let resp1_retry = client
        .append(proto::AppendRequest {
            stream_id: stream_id.clone(),
            expected_version: any_version(),
            events: vec![make_proposed_with_id(&id1, "Evt1")],
        })
        .await
        .expect("re-append of evicted id1 should succeed")
        .into_inner();

    // id1 was evicted, so it should get a new global position > original.
    assert!(
        resp1_retry.first_global_position > resp1.first_global_position,
        "evicted id1 should get a new (higher) global_position: retry={} original={}",
        resp1_retry.first_global_position,
        resp1.first_global_position
    );

    // Re-send id3: should be a dedup hit returning the original position.
    let resp3_retry = client
        .append(proto::AppendRequest {
            stream_id: stream_id.clone(),
            expected_version: any_version(),
            events: vec![make_proposed_with_id(&id3, "Evt3")],
        })
        .await
        .expect("dedup hit on id3 should return Ok")
        .into_inner();

    assert_eq!(
        resp3.first_global_position, resp3_retry.first_global_position,
        "id3 should be a dedup hit with original position"
    );

    handle.shutdown().await;
}

// -- Test (AC-8 no broker): Dedup hit does not produce subscription messages --

#[tokio::test]
async fn dedup_hit_does_not_publish_to_subscription() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let data_path = dir.path().join("events.log");

    let mut client = start_simple_server(&data_path, default_dedup_cap()).await;

    let stream_id = uuid::Uuid::new_v4().to_string();
    let id1 = uuid::Uuid::new_v4().to_string();

    let events = vec![make_proposed_with_id(&id1, "Evt1")];

    // Start a SubscribeAll subscription before any appends.
    let mut sub = client
        .subscribe_all(proto::SubscribeAllRequest { from_position: 0 })
        .await
        .expect("subscribe_all should succeed")
        .into_inner();

    let timeout_dur = std::time::Duration::from_secs(5);

    // CaughtUp immediately (store is empty).
    let msg = tokio::time::timeout(timeout_dur, sub.message())
        .await
        .expect("should not timeout")
        .expect("message should succeed")
        .expect("stream should not end");
    assert!(
        matches!(
            msg.content,
            Some(proto::subscribe_response::Content::CaughtUp(_))
        ),
        "expected CaughtUp on empty store"
    );

    // First append: creates the event.
    client
        .append(proto::AppendRequest {
            stream_id: stream_id.clone(),
            expected_version: any_version(),
            events: events.clone(),
        })
        .await
        .expect("first append should succeed");

    // Drain the live event from the subscription.
    let msg = tokio::time::timeout(timeout_dur, sub.message())
        .await
        .expect("should not timeout")
        .expect("message should succeed")
        .expect("stream should not end");
    match msg.content.expect("content should be set") {
        proto::subscribe_response::Content::Event(e) => {
            assert_eq!(e.global_position, 0);
        }
        proto::subscribe_response::Content::CaughtUp(_) => {
            panic!("expected Event, got CaughtUp");
        }
    }

    // Second append: same event IDs (dedup hit).
    client
        .append(proto::AppendRequest {
            stream_id: stream_id.clone(),
            expected_version: any_version(),
            events,
        })
        .await
        .expect("dedup hit should return Ok");

    // No additional messages should arrive from the broker for the dedup hit.
    // Use a short timeout to confirm silence.
    let silence = tokio::time::timeout(std::time::Duration::from_millis(300), sub.message()).await;
    assert!(
        silence.is_err(),
        "expected timeout (no message from dedup hit), but got a message"
    );
}
