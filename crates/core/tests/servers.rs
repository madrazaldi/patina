//! Real HTTP against several instances of tools/mock_pms.py: searching every server of the
//! account, keeping each item with its own server, and undo.
//!
//! Each test uses its own machine ids, because connections to other servers are cached
//! per account and machine id for the whole process.
use std::io::BufRead;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use patina_core::ops::xsearch::{self, ServerState};
use patina_core::state::Session;
use patina_core::{AppState, CoreEvent, EventSink, servers, undo};
use patina_plex::{ClientInfo, MetadataOptions, PlexServer, PlexTv, Resource};
use serde_json::json;

struct Mock {
    child: Child,
    url: String,
    port: u16,
    machine: String,
    name: String,
    _media: tempfile::TempDir,
}

impl Mock {
    fn start(name: &str, machine: &str, seed: u32, extra: &[&str]) -> Self {
        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/mock_pms.py");
        let media = tempfile::tempdir().unwrap();
        std::fs::write(media.path().join("movie.mkv"), vec![42u8; 1024]).unwrap();
        let mut child = Command::new("python3")
            .arg(script)
            .args(["--port", "0", "--name", name, "--machine", machine, "--seed", &seed.to_string()])
            .args(extra)
            .arg("--media")
            .arg(media.path())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("python3 is needed for the mock server");
        let mut log = std::io::BufReader::new(child.stderr.take().unwrap());
        let mut first = String::new();
        log.read_line(&mut first).unwrap();
        let url = first.trim().strip_prefix("mock PMS on ").expect("mock server startup").to_owned();
        // Keep reading so the mock never blocks on a full pipe.
        std::thread::spawn(move || {
            let mut sink = String::new();
            let _ = std::io::Read::read_to_string(&mut log, &mut sink);
        });
        let port = url.rsplit(':').next().unwrap().parse().unwrap();
        Self { child, url, port, machine: machine.into(), name: name.into(), _media: media }
    }

    fn resource(&self) -> Resource {
        serde_json::from_value(json!({
            "name": self.name, "clientIdentifier": self.machine, "provides": "server", "owned": true, "presence": true,
            "accessToken": "t",
            "connections": [{ "protocol": "http", "address": "127.0.0.1", "port": self.port, "uri": self.url, "local": true, "relay": false }],
        }))
        .unwrap()
    }
}

impl Drop for Mock {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Held while a test changes a session: a pending playback failure is kept for the whole
/// process, and a session change in a parallel test would drop it.
static SESSIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn set_session(app: &AppState, session: Option<Session>) {
    let _held = SESSIONS.lock().unwrap_or_else(|e| e.into_inner());
    app.set_session(session, vec![]);
}

struct Sink;
impl EventSink for Sink {
    fn emit(&self, _: CoreEvent) {}
}

/// An app connected to `current`, on an account that also has the `others`.
fn app(current: &Mock, others: &[&Mock]) -> AppState {
    let app = AppState::new(Arc::new(Sink), tokio::runtime::Handle::current());
    let info = ClientInfo::new("test", "Patina", "test");
    let mut servers = vec![current.resource()];
    servers.extend(others.iter().map(|m| m.resource()));
    set_session(
        &app,
        Some(Session {
            tv: Some(PlexTv::new(info.clone(), Some("x".into())).unwrap()),
            server: PlexServer::new(info, &current.url, "alice").unwrap(),
            server_id: current.machine.clone(),
            server_name: current.name.clone(),
            user: None,
            owned: true,
            servers,
        }),
    );
    app
}

#[tokio::test]
async fn a_search_groups_a_title_across_servers_with_each_servers_copy() {
    let (alpha, beta) = (Mock::start("Alpha", "alpha-search", 0, &[]), Mock::start("Beta", "beta-search", 1, &[]));
    let app = app(&alpha, &[&beta]);

    let mut updates = 0;
    let found = xsearch::search_all(&app, "e", |_| updates += 1).await;
    assert!(found.done);
    assert!(updates >= 3, "the start and each server's answer are reported, got {updates}");
    assert!(found.servers.iter().all(|s| s.state == ServerState::Done), "{:?}", found.servers);

    // Ten titles are on both servers, each in its own quality.
    let shared: Vec<_> = found.groups.iter().filter(|g| g.copies.len() == 2).collect();
    assert!(!shared.is_empty(), "no title was found on both servers");
    let group = shared[0];
    assert!(group.copies[0].current, "the connected server's copy comes first");
    assert_eq!(group.copies[0].item.server_id(), None);
    assert_eq!(group.copies[1].item.server_id(), Some("beta-search"));
    assert_eq!(group.copies[0].versions[0].resolution.as_deref(), Some("720p"));
    assert_eq!(group.copies[1].versions[0].resolution.as_deref(), Some("1080p"));

    // Titles only one server has are still listed.
    assert!(found.groups.iter().any(|g| g.copies.len() == 1 && g.copies[0].item.server_id() == Some("beta-search")));
    assert!(found.groups.iter().any(|g| g.copies.len() == 1 && g.copies[0].current));

    // The other server's copy is fetched from that server: the same rating key means another title on this one.
    let copy = &group.copies[1].item;
    let there = servers::server_of(&app, copy).await.unwrap().metadata(copy.rating_key(), MetadataOptions::default()).await.unwrap().unwrap();
    assert_eq!(there.title(), group.title);
    let here = app.server().unwrap().metadata(copy.rating_key(), MetadataOptions::default()).await.unwrap().unwrap();
    assert_ne!(here.title(), group.title, "the key must not be looked up on the connected server");
}

#[tokio::test]
async fn a_slow_server_times_out_while_the_others_answer() {
    let (alpha, slow) = (Mock::start("Alpha", "alpha-slow", 0, &[]), Mock::start("Slow", "slow-slow", 1, &["--search-delay", "4"]));
    let beta = Mock::start("Beta", "beta-slow", 2, &[]);
    let app = app(&alpha, &[&slow, &beta]);

    let began = Instant::now();
    let mut seen_pending = false;
    let found = xsearch::search_all_within(&app, "e", Duration::from_millis(800), |r| {
        seen_pending |= r.servers.iter().any(|s| s.state == ServerState::Pending) && !r.groups.is_empty();
    })
    .await;
    assert!(began.elapsed() < Duration::from_secs(3), "the slow server held the search for {:?}", began.elapsed());
    assert!(found.done);
    let state = |id: &str| found.servers.iter().find(|s| s.id == id).unwrap().state;
    assert_eq!(state("slow-slow"), ServerState::TimedOut);
    assert_eq!(state("alpha-slow"), ServerState::Done);
    assert_eq!(state("beta-slow"), ServerState::Done);
    assert!(seen_pending, "partial results are reported before the slow server finishes");
    assert!(!found.groups.is_empty());
    assert!(found.groups.iter().all(|g| g.copies.iter().all(|c| c.server_id != "slow-slow")));
}

#[tokio::test]
async fn watch_state_changes_and_their_undo_stay_on_their_own_server() {
    let (alpha, beta) = (Mock::start("Alpha", "alpha-watch", 0, &[]), Mock::start("Beta", "beta-watch", 1, &[]));
    let app = app(&alpha, &[&beta]);
    let here = app.server().unwrap();
    let there = servers::server_for(&app, Some("beta-watch")).await.unwrap();
    let watched = |s: &PlexServer| {
        let s = s.clone();
        async move { s.metadata("112", MetadataOptions::default()).await.unwrap().unwrap().is_watched() }
    };

    // Marking the other server's item leaves this server's item with the same key alone.
    undo::set_watched(&app, Some("beta-watch"), &["112".to_string()], true).await.unwrap();
    assert!(watched(&there).await);
    assert!(!watched(&here).await);
    let (_, label) = undo::last(&app).expect("the change is undoable");
    assert!(label.contains("watched"), "{label}");
    undo::undo(&app, None).await.unwrap();
    assert!(!watched(&there).await);
    assert!(undo::last(&app).is_none());

    // Bulk: several at once, undone together. Whatever was watched before stays watched.
    let keys: Vec<String> = ["112", "113", "114"].map(String::from).to_vec();
    let states = || {
        let (here, keys) = (here.clone(), keys.clone());
        async move {
            let mut out = Vec::new();
            for key in &keys {
                out.push(here.metadata(key, MetadataOptions::default()).await.unwrap().unwrap().is_watched());
            }
            out
        }
    };
    let before = states().await;
    assert!(before.contains(&false), "the test needs an unwatched item");
    undo::set_watched(&app, None, &keys, true).await.unwrap();
    assert_eq!(states().await, [true, true, true]);
    undo::undo(&app, None).await.unwrap();
    assert_eq!(states().await, before);
}

#[tokio::test]
async fn playlist_entries_come_back_where_they_were_and_resume_points_return() {
    let alpha = Mock::start("Alpha", "alpha-list", 0, &[]);
    let app = app(&alpha, &[]);
    let server = app.server().unwrap();
    let order = || {
        let server = server.clone();
        async move { server.playlist_items("950", 0, 100).await.unwrap().metadata.iter().map(|m| m.rating_key().to_string()).collect::<Vec<_>>() }
    };
    assert_eq!(order().await, ["104", "105", "1012"]);

    // The middle entry goes back between its neighbours.
    undo::remove_from_playlist(&app, "950", 7105).await.unwrap();
    assert_eq!(order().await, ["104", "1012"]);
    undo::undo(&app, None).await.unwrap();
    assert_eq!(order().await, ["104", "105", "1012"]);

    // So does the first.
    undo::remove_from_playlist(&app, "950", 7104).await.unwrap();
    assert_eq!(order().await, ["105", "1012"]);
    undo::undo(&app, None).await.unwrap();
    assert_eq!(order().await, ["104", "105", "1012"]);

    // Hiding an item from Continue Watching and clearing its progress give the resume point back.
    let offset = |s: PlexServer| async move { s.metadata("100", MetadataOptions::default()).await.unwrap().unwrap().view_offset.unwrap_or(0) };
    let before = offset(server.clone()).await;
    assert!(before > 0, "the mock item has a resume point");
    undo::clear_progress(&app, "100").await.unwrap();
    assert_eq!(offset(server.clone()).await, 0);
    undo::undo(&app, None).await.unwrap();
    assert_eq!(offset(server.clone()).await, before);

    undo::remove_from_continue_watching(&app, "100").await.unwrap();
    undo::undo(&app, None).await.unwrap();
    assert_eq!(offset(server.clone()).await, before);
}

#[tokio::test]
async fn undo_history_is_dropped_when_the_server_changes() {
    let (alpha, beta) = (Mock::start("Alpha", "alpha-scope", 0, &[]), Mock::start("Beta", "beta-scope", 1, &[]));
    let app = app(&alpha, &[&beta]);
    undo::set_watched(&app, None, &["112".to_string()], true).await.unwrap();
    assert!(undo::last(&app).is_some());

    let other = self::app(&beta, &[&alpha]);
    // A different session on the same state: the old entries must not apply to the new server.
    set_session(&app, other.session());
    assert!(undo::last(&app).is_none());
    assert!(undo::undo(&app, None).await.is_err());
}

#[tokio::test]
async fn a_playback_failure_is_not_retried_on_another_server() {
    use patina_core::failure::{self, Phase, Recover, Report};
    use patina_core::player::PlayRequest;
    let (alpha, beta) = (Mock::start("Alpha", "alpha-failure", 0, &[]), Mock::start("Beta", "beta-failure", 1, &[]));
    let app = app(&alpha, &[&beta]);
    let other = self::app(&beta, &[&alpha]);
    let item = app.server().unwrap().metadata("112", MetadataOptions::default()).await.unwrap().unwrap();
    let report = || Report {
        item: item.clone(),
        error: Some("loading failed".into()),
        kind: None,
        phase: Phase::Start,
        position_ms: 61_000,
        media_index: 0,
        quality: "original".into(),
        remaining: 0,
        request: PlayRequest { item: item.clone(), ..Default::default() },
    };

    let _held = SESSIONS.lock().unwrap_or_else(|e| e.into_inner());
    // Connecting to the same server again keeps the failure on offer.
    failure::record(&app, report());
    app.set_session(app.session(), vec![]);
    assert!(failure::current().is_some());

    // Another server: its rating key "112" is a different item, so the failure is dropped.
    app.set_session(other.session(), vec![]);
    assert!(failure::current().is_none());
    assert!(failure::recover(&app, Recover::Retry).is_err());
}
