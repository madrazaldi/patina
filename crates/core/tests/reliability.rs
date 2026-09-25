//! Real HTTP against tools/mock_pms.py; isolated subprocesses keep global config/player state out of other tests.
use patina_core::state::Session;
use patina_core::{AppState, CoreEvent, EventSink, downloads, offline};
use patina_plex::{ClientInfo, MetadataOptions, PlexServer};
use std::{
    io::BufRead,
    process::{Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};

#[test]
fn interrupted_downloads_and_offline_sync() {
    let temp = tempfile::tempdir().unwrap();
    let media = temp.path().join("media");
    std::fs::create_dir_all(&media).unwrap();
    std::fs::write(media.join("movie.mkv"), vec![42u8; 8 * 1024 * 1024]).unwrap();
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/mock_pms.py");
    let events_path = temp.path().join("events.log");
    let events = std::fs::File::create(&events_path).unwrap();
    let mut server = Command::new("python3")
        .arg(script)
        .args(["--port", "0", "--download-delay", "0.012", "--drop-download-once"])
        .arg("--media")
        .arg(&media)
        .stderr(Stdio::piped())
        .stdout(events)
        .spawn()
        .unwrap();
    let mut log = std::io::BufReader::new(server.stderr.take().unwrap());
    let mut first = String::new();
    log.read_line(&mut first).unwrap();
    let url = first.trim().strip_prefix("mock PMS on ").expect("mock server startup").to_owned();
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        std::io::Read::read_to_string(&mut log, &mut s).unwrap();
        s
    });
    let result = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "download_and_offline_worker", "--nocapture"])
        .env("PATINA_TEST_SERVER", &url)
        .env("XDG_CONFIG_HOME", temp.path().join("config"))
        .env("PATINA_TEST_DOWNLOADS", temp.path().join("files"))
        .status()
        .unwrap();
    let recovery = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "restart_worker", "--nocapture"])
        .env("PATINA_TEST_SERVER", &url)
        .env("XDG_CONFIG_HOME", temp.path().join("config"))
        .env("PATINA_TEST_DOWNLOADS", temp.path().join("files"))
        .status()
        .unwrap();
    let _ = server.kill();
    let _ = server.wait();
    let log = reader.join().unwrap();
    assert!(result.success(), "{log}");
    assert!(recovery.success(), "{log}");
    let events = std::fs::read_to_string(events_path).unwrap();
    assert!(events.contains("bytes="), "resume must issue a Range request: {events}");
}

struct Sink;
impl EventSink for Sink {
    fn emit(&self, _: CoreEvent) {}
}

#[tokio::test]
async fn download_and_offline_worker() {
    let Ok(url) = std::env::var("PATINA_TEST_SERVER") else {
        return;
    };
    let app = AppState::new(Arc::new(Sink), tokio::runtime::Handle::current());
    app.update_settings(|s| s.player.downloads_dir = std::env::var("PATINA_TEST_DOWNLOADS").unwrap());
    let session = |token: &str| Session {
        tv: None,
        server: PlexServer::new(ClientInfo::new("test", "Patina", "test"), &url, token).unwrap(),
        server_id: "mock-machine-0001".into(),
        server_name: "Mock".into(),
        user: None,
        owned: true,
        servers: vec![],
    };
    app.set_session(Some(session("alice")), vec![]);
    downloads::enqueue(&app, "100").await.unwrap();
    let start = Instant::now();
    loop {
        let d = downloads::list().remove(0);
        if d.received > 0 {
            downloads::pause(&app, &d.id);
            tokio::time::sleep(Duration::from_millis(350)).await;
            assert_eq!(downloads::list()[0].state, downloads::State::Paused);
            let partial = format!("{}.part", d.file.unwrap());
            assert!(std::fs::metadata(&partial).unwrap().len() > 0);
            downloads::retry(&app, &d.id);
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(15), "download did not start: {d:?}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let d = loop {
        let d = downloads::list().remove(0);
        if d.state == downloads::State::Done {
            break d;
        }
        assert_ne!(d.state, downloads::State::Failed, "{d:?}");
        assert!(start.elapsed() < Duration::from_secs(30));
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    assert_eq!(std::fs::read(d.file.as_ref().unwrap()).unwrap(), vec![42u8; 8 * 1024 * 1024]);
    offline::record(&d, 90000, 180000, false, 0.9).unwrap();
    assert_eq!(offline::position(&d), 90000);
    app.set_session(Some(session("bob")), vec![]);
    offline::sync(&app).await.unwrap();
    let server = session("alice").server;
    let item = server.metadata("100", MetadataOptions::default()).await.unwrap().unwrap();
    assert_eq!(item.view_offset, Some(61000), "other profile must not replay Alice's progress");
    app.set_session(Some(session("alice")), vec![]);
    offline::sync(&app).await.unwrap();
    assert_eq!(server.metadata("100", MetadataOptions::default()).await.unwrap().unwrap().view_offset, Some(90000));
    offline::record(&d, 179000, 180000, true, 0.9).unwrap();
    offline::sync(&app).await.unwrap();
    assert!(server.metadata("100", MetadataOptions::default()).await.unwrap().unwrap().is_watched());
    assert_eq!(offline::position(&d), 0);
    // Leave a second transfer unfinished, then exit this process entirely.
    downloads::enqueue(&app, "101").await.unwrap();
    let began = Instant::now();
    loop {
        if downloads::list().iter().any(|d| d.item.rating_key() == "101" && d.state == downloads::State::Downloading && d.received > 0) {
            break;
        }
        assert!(began.elapsed() < Duration::from_secs(10));
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn restart_worker() {
    let Ok(url) = std::env::var("PATINA_TEST_SERVER") else {
        return;
    };
    let app = AppState::new(Arc::new(Sink), tokio::runtime::Handle::current());
    app.set_session(
        Some(Session {
            tv: None,
            server: PlexServer::new(ClientInfo::new("test", "Patina", "test"), &url, "alice").unwrap(),
            server_id: "mock-machine-0001".into(),
            server_name: "Mock".into(),
            user: None,
            owned: true,
            servers: vec![],
        }),
        vec![],
    );
    downloads::resume(&app);
    let start = Instant::now();
    loop {
        let d = downloads::list().into_iter().find(|d| d.item.rating_key() == "101").expect("interrupted transfer persisted");
        if d.state == downloads::State::Done {
            assert_eq!(std::fs::read(d.file.unwrap()).unwrap(), vec![42u8; 8 * 1024 * 1024]);
            break;
        }
        assert_ne!(d.state, downloads::State::Failed, "{d:?}");
        assert!(start.elapsed() < Duration::from_secs(15));
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Optional actual-mpv coverage, separate from portable mock-server checks.
#[test]
#[ignore = "requires mpv and ffmpeg on PATH"]
fn mpv_playback_smoke() {
    let temp = tempfile::tempdir().unwrap();
    let media = temp.path().join("tone.wav");
    assert!(
        Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "sine=frequency=440:duration=20"])
            .arg(&media)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "mpv_worker", "--nocapture"])
            .env("PATINA_TEST_MPV_FILE", &media)
            .env("XDG_CONFIG_HOME", temp.path().join("config"))
            .status()
            .unwrap()
            .success()
    );
}

#[tokio::test]
async fn mpv_worker() {
    let Ok(file) = std::env::var("PATINA_TEST_MPV_FILE") else {
        return;
    };
    let app = AppState::new(Arc::new(Sink), tokio::runtime::Handle::current());
    app.update_settings(|s| {
        s.player.extra_args = "--no-config --vo=null --ao=null".into();
        s.auth.account_id = Some("alice".into());
    });
    let d = downloads::Download {
        id: "mock:alice:tone".into(),
        server_id: "mock".into(),
        server_name: "Mock".into(),
        item: patina_plex::Metadata {
            rating_key: Some("tone".into()),
            title: Some("Tone".into()),
            kind: Some("track".into()),
            duration: Some(20000),
            ..Default::default()
        },
        file: Some(file),
        poster: None,
        size: 0,
        received: 0,
        state: downloads::State::Done,
        error: None,
        added_at: 0,
        account: "alice".into(),
        validator: None,
        source_key: None,
    };
    patina_core::storage::update_json::<Vec<downloads::Download>, _>(&patina_core::config::config_dir().join("downloads.json"), |list| {
        list.push(d.clone())
    })
    .unwrap();
    offline::record(&d, 8000, 20000, false, 0.9).unwrap();
    patina_core::ops::playback::play_downloads(&app, std::slice::from_ref(&d.id)).unwrap();
    let start = Instant::now();
    loop {
        if app.now_playing().is_some_and(|n| n.position_ms > 8100 && n.position_ms < 14000) {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(8), "mpv did not resume from the saved position");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    patina_core::player::control(patina_core::player::Control::SeekTo(18.0));
    loop {
        if app.now_playing().is_none() && offline::position(&d) == 0 {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(15), "mpv did not persist completion");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    tokio::task::spawn_blocking(patina_core::player::shutdown).await.unwrap();
}
