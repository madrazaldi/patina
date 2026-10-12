//! Listener lifecycle against fake servers that hold their notification
//! stream open. The worker runs in a subprocess with its own config dir, as
//! in `reliability.rs`, so settings written by `AppState::new` stay isolated.
use patina_core::state::Session;
use patina_core::{AppState, CoreEvent, EventSink, live};
use patina_plex::{ClientInfo, PlexServer};
use std::{
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

struct Sink;
impl EventSink for Sink {
    fn emit(&self, _: CoreEvent) {}
}

#[test]
fn repeated_server_switches_leave_one_listener() {
    let temp = tempfile::tempdir().unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "switch_worker", "--nocapture"])
        .env("PATINA_TEST_LIVE", "1")
        .env("XDG_CONFIG_HOME", temp.path().join("config"))
        .status()
        .unwrap();
    assert!(status.success());
}

/// A server that answers ordinary requests with an empty container and holds
/// `/:/eventsource/notifications` open until the client hangs up. `open`
/// counts the notification streams currently connected.
async fn fake_server() -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let open = Arc::new(AtomicUsize::new(0));
    let count = open.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else { return };
            let open = count.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).into_owned();
                if head.contains("/:/eventsource/notifications") {
                    open.fetch_add(1, Ordering::SeqCst);
                    let _ = socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\n").await;
                    // Parked here like a real server between events, until the client goes away.
                    while matches!(socket.read(&mut buf).await, Ok(n) if n > 0) {}
                    open.fetch_sub(1, Ordering::SeqCst);
                } else {
                    let body = r#"{"MediaContainer":{"size":0}}"#;
                    let reply = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                    let _ = socket.write_all(reply.as_bytes()).await;
                }
            });
        }
    });
    (url, open)
}

async fn until(what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < Duration::from_secs(10), "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn switch_worker() {
    if std::env::var("PATINA_TEST_LIVE").is_err() {
        return;
    }
    let app = AppState::new(Arc::new(Sink), tokio::runtime::Handle::current());
    let mut servers = Vec::new();
    for _ in 0..3 {
        servers.push(fake_server().await);
    }
    let session = |url: &str| Session {
        tv: None,
        server: PlexServer::new(ClientInfo::new("test", "Patina", "test"), url, "token").unwrap(),
        server_id: url.into(),
        server_name: "Fake".into(),
        user: None,
        owned: true,
        servers: vec![],
    };
    let total = || servers.iter().map(|(_, open)| open.load(Ordering::SeqCst)).sum::<usize>();

    // Switch between servers over and over, as the session flow does: new session, then a fresh listener.
    for i in 0..12 {
        let (url, open) = &servers[i % servers.len()];
        app.set_session(Some(session(url)), vec![]);
        live::watch(app.clone());
        until("the new listener to connect", || open.load(Ordering::SeqCst) == 1).await;
    }
    let (last_url, last_open) = &servers[11 % servers.len()];
    assert!(live::listening(&app));
    // Every earlier listener closed its connection; only the current server has one.
    until("old listeners to hang up", || total() == 1).await;
    assert_eq!(last_open.load(Ordering::SeqCst), 1, "the open stream belongs to {last_url}");

    // Signing out leaves none.
    live::stop(&app);
    until("the listener to hang up", || total() == 0).await;
    assert!(!live::listening(&app));

    // Replacing a listener that is waiting out a backoff after its server went away also leaves one.
    drop(servers);
    app.set_session(Some(session("http://127.0.0.1:9")), vec![]);
    live::watch(app.clone());
    tokio::time::sleep(Duration::from_millis(100)).await;
    live::watch(app.clone());
    live::stop(&app);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!live::listening(&app));
}
