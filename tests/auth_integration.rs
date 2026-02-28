//! Integration tests for EventfoldDB JWT authentication interceptor.
//!
//! Each test spins up a real in-process tonic server with or without
//! `JwtInterceptor` wired, connects a gRPC client, and verifies that
//! authentication is enforced (or bypassed) correctly.

use std::net::SocketAddr;
use std::num::NonZeroUsize;

use eventfold_db::auth::JwtInterceptor;
use eventfold_db::proto::event_store_client::EventStoreClient;
use eventfold_db::proto::event_store_server::EventStoreServer;
use eventfold_db::proto::{self, expected_version};
use eventfold_db::{Broker, EventfoldService, Store, spawn_writer};
use tempfile::TempDir;
use tonic::service::interceptor::InterceptedService;

/// Default dedup capacity for integration tests.
fn test_dedup_cap() -> NonZeroUsize {
    NonZeroUsize::new(128).expect("nonzero")
}

/// Helper: create a proto ProposedEvent with a random UUID and given event type.
fn make_proposed(event_type: &str) -> proto::ProposedEvent {
    proto::ProposedEvent {
        event_id: uuid::Uuid::new_v4().to_string(),
        event_type: event_type.to_string(),
        metadata: vec![],
        payload: b"{}".to_vec(),
    }
}

/// Helper: create an ExpectedVersion::NoStream.
fn no_stream() -> Option<proto::ExpectedVersion> {
    Some(proto::ExpectedVersion {
        kind: Some(expected_version::Kind::NoStream(proto::Empty {})),
    })
}

/// Mint an HS256 JWT token with `sub = "test"` and `exp = now + exp_offset_secs`.
///
/// When `exp_offset_secs` is negative, the token is already expired.
fn mint_token(secret: &str, exp_offset_secs: i64) -> String {
    #[derive(serde::Serialize)]
    struct Claims {
        sub: String,
        exp: u64,
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_secs();

    // Apply the signed offset, clamping to 0 if it would go negative.
    let exp = if exp_offset_secs >= 0 {
        now + exp_offset_secs as u64
    } else {
        now.saturating_sub(exp_offset_secs.unsigned_abs())
    };

    let claims = Claims {
        sub: "test".to_string(),
        exp,
    };

    jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .expect("JWT encoding should not fail in tests")
}

/// Spin up an in-process gRPC server with JWT authentication enabled on an
/// ephemeral `[::1]:0` port.
///
/// Returns `(SocketAddr, TempDir)`. The `TempDir` must be kept alive for the
/// duration of the test to prevent cleanup of the event log.
async fn start_authed_test_server(secret: &str) -> (SocketAddr, TempDir) {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let path = dir.path().join("events.log");
    let store = Store::open(&path).expect("open should succeed");
    let broker = Broker::new(1024);
    let (writer_handle, read_index, _join_handle) =
        spawn_writer(store, 64, broker.clone(), test_dedup_cap());
    let service = EventfoldService::new(writer_handle, read_index, broker);

    let interceptor = JwtInterceptor::new(secret);
    let svc = InterceptedService::new(EventStoreServer::new(service), interceptor);

    let listener = tokio::net::TcpListener::bind("[::1]:0")
        .await
        .expect("bind should succeed");
    let addr = listener.local_addr().expect("should have local addr");
    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

    tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(svc)
            .serve_with_incoming(incoming)
            .await
            .expect("authed server should run");
    });

    // Give the server a moment to start accepting connections.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    (addr, dir)
}

/// Spin up an in-process gRPC server WITHOUT authentication (no interceptor)
/// on an ephemeral `[::1]:0` port.
///
/// Returns `(SocketAddr, TempDir)`. Used for backward-compatibility testing.
async fn start_plain_test_server() -> (SocketAddr, TempDir) {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let path = dir.path().join("events.log");
    let store = Store::open(&path).expect("open should succeed");
    let broker = Broker::new(1024);
    let (writer_handle, read_index, _join_handle) =
        spawn_writer(store, 64, broker.clone(), test_dedup_cap());
    let service = EventfoldService::new(writer_handle, read_index, broker);

    let listener = tokio::net::TcpListener::bind("[::1]:0")
        .await
        .expect("bind should succeed");
    let addr = listener.local_addr().expect("should have local addr");
    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

    tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(EventStoreServer::new(service))
            .serve_with_incoming(incoming)
            .await
            .expect("plain server should run");
    });

    // Give the server a moment to start accepting connections.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    (addr, dir)
}

// -- Test: valid token allows Append to succeed --

#[tokio::test]
async fn auth_valid_token_append_succeeds() {
    let (addr, _dir) = start_authed_test_server("testsecret").await;

    let mut client = EventStoreClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .expect("client connect should succeed");

    let token = mint_token("testsecret", 3600);
    let stream_id = uuid::Uuid::new_v4().to_string();

    let mut request = tonic::Request::new(proto::AppendRequest {
        stream_id,
        expected_version: no_stream(),
        events: vec![make_proposed("AuthEvent")],
    });
    request.metadata_mut().insert(
        "authorization",
        format!("Bearer {token}").parse().expect("valid ASCII"),
    );

    let result = client.append(request).await;
    assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
}

// -- Test: missing token returns UNAUTHENTICATED --

#[tokio::test]
async fn auth_missing_token_append_rejected() {
    let (addr, _dir) = start_authed_test_server("testsecret").await;

    let mut client = EventStoreClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .expect("client connect should succeed");

    let stream_id = uuid::Uuid::new_v4().to_string();

    // No authorization header attached.
    let request = tonic::Request::new(proto::AppendRequest {
        stream_id,
        expected_version: no_stream(),
        events: vec![make_proposed("AuthEvent")],
    });

    let result = client.append(request).await;
    let status = result.expect_err("expected Unauthenticated error");
    assert_eq!(
        status.code(),
        tonic::Code::Unauthenticated,
        "expected UNAUTHENTICATED, got: {:?}",
        status.code()
    );
}

// -- Test: expired token returns UNAUTHENTICATED --

#[tokio::test]
async fn auth_expired_token_append_rejected() {
    let (addr, _dir) = start_authed_test_server("testsecret").await;

    let mut client = EventStoreClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .expect("client connect should succeed");

    // Mint a token that expired 1 hour ago.
    let token = mint_token("testsecret", -3600);
    let stream_id = uuid::Uuid::new_v4().to_string();

    let mut request = tonic::Request::new(proto::AppendRequest {
        stream_id,
        expected_version: no_stream(),
        events: vec![make_proposed("AuthEvent")],
    });
    request.metadata_mut().insert(
        "authorization",
        format!("Bearer {token}").parse().expect("valid ASCII"),
    );

    let result = client.append(request).await;
    let status = result.expect_err("expected Unauthenticated error for expired token");
    assert_eq!(
        status.code(),
        tonic::Code::Unauthenticated,
        "expected UNAUTHENTICATED, got: {:?}",
        status.code()
    );
}

// -- Test: no-secret server accepts unauthenticated requests (backward-compat) --

#[tokio::test]
async fn auth_disabled_no_secret_append_succeeds() {
    let (addr, _dir) = start_plain_test_server().await;

    let mut client = EventStoreClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .expect("client connect should succeed");

    let stream_id = uuid::Uuid::new_v4().to_string();

    // No authorization header -- should still succeed without interceptor.
    let request = tonic::Request::new(proto::AppendRequest {
        stream_id,
        expected_version: no_stream(),
        events: vec![make_proposed("PlainEvent")],
    });

    let result = client.append(request).await;
    assert!(
        result.is_ok(),
        "expected Ok on no-auth server, got: {:?}",
        result.err()
    );
}

// -- Test: SubscribeAll without token is rejected --

#[tokio::test]
async fn auth_streaming_subscribe_all_rejected_without_token() {
    let (addr, _dir) = start_authed_test_server("testsecret").await;

    let mut client = EventStoreClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .expect("client connect should succeed");

    // No authorization header on the subscribe_all request.
    let request = tonic::Request::new(proto::SubscribeAllRequest { from_position: 0 });

    let result = client.subscribe_all(request).await;

    // The interceptor rejects before the stream opens, so the RPC itself fails.
    let status = result.expect_err("expected Unauthenticated error on subscribe_all");
    assert_eq!(
        status.code(),
        tonic::Code::Unauthenticated,
        "expected UNAUTHENTICATED, got: {:?}",
        status.code()
    );
}

// -- Test: SubscribeStream without token is rejected --

#[tokio::test]
async fn auth_streaming_subscribe_stream_rejected_without_token() {
    let (addr, _dir) = start_authed_test_server("testsecret").await;

    let mut client = EventStoreClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .expect("client connect should succeed");

    let stream_id = uuid::Uuid::new_v4().to_string();

    // No authorization header on the subscribe_stream request.
    let request = tonic::Request::new(proto::SubscribeStreamRequest {
        stream_id,
        from_version: 0,
    });

    let result = client.subscribe_stream(request).await;

    let status = result.expect_err("expected Unauthenticated error on subscribe_stream");
    assert_eq!(
        status.code(),
        tonic::Code::Unauthenticated,
        "expected UNAUTHENTICATED, got: {:?}",
        status.code()
    );
}

// -- Test: valid token on SubscribeAll stays open and receives events --

#[tokio::test]
async fn auth_valid_token_subscribe_all_stays_open() {
    let (addr, _dir) = start_authed_test_server("testsecret").await;

    let token = mint_token("testsecret", 3600);

    // Open a SubscribeAll stream with a valid token.
    let mut sub_client = EventStoreClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .expect("sub client connect should succeed");

    let mut sub_request = tonic::Request::new(proto::SubscribeAllRequest { from_position: 0 });
    sub_request.metadata_mut().insert(
        "authorization",
        format!("Bearer {token}").parse().expect("valid ASCII"),
    );

    let mut sub = sub_client
        .subscribe_all(sub_request)
        .await
        .expect("subscribe_all with valid token should succeed")
        .into_inner();

    // The stream should first yield a CaughtUp marker (no events exist yet).
    let timeout_dur = std::time::Duration::from_secs(5);
    let msg = tokio::time::timeout(timeout_dur, sub.message())
        .await
        .expect("should not timeout waiting for CaughtUp")
        .expect("message should succeed")
        .expect("stream should not end");
    let content = msg.content.expect("content should be set");
    assert!(
        matches!(content, proto::subscribe_response::Content::CaughtUp(_)),
        "expected CaughtUp marker, got event"
    );

    // Append one event via a separate client with a valid token.
    let mut append_client = EventStoreClient::connect(format!("http://[::1]:{}", addr.port()))
        .await
        .expect("append client connect should succeed");

    let append_token = mint_token("testsecret", 3600);
    let stream_id = uuid::Uuid::new_v4().to_string();

    let mut append_request = tonic::Request::new(proto::AppendRequest {
        stream_id,
        expected_version: no_stream(),
        events: vec![make_proposed("LiveAuthEvent")],
    });
    append_request.metadata_mut().insert(
        "authorization",
        format!("Bearer {append_token}")
            .parse()
            .expect("valid ASCII"),
    );

    append_client
        .append(append_request)
        .await
        .expect("authenticated append should succeed");

    // Receive the live event from the subscription stream.
    let msg = tokio::time::timeout(timeout_dur, sub.message())
        .await
        .expect("should not timeout waiting for live event")
        .expect("message should succeed")
        .expect("stream should not end");
    let content = msg.content.expect("content should be set");
    match content {
        proto::subscribe_response::Content::Event(e) => {
            assert_eq!(e.event_type, "LiveAuthEvent");
            assert_eq!(e.global_position, 0);
        }
        proto::subscribe_response::Content::CaughtUp(_) => {
            panic!("expected live Event, got CaughtUp");
        }
    }
}
