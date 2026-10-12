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
    download_reliability("normal");
}

#[test]
fn resumed_download_without_validator_restarts() {
    download_reliability("missing");
}

#[test]
fn resumed_download_with_changed_validator_restarts() {
    download_reliability("changed");
}

fn download_reliability(validator: &str) {
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
        .args(["--resume-validator", validator, "--missing-metadata", "999998"])
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
        .env("PATINA_TEST_RESUME_VALIDATOR", validator)
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
            if std::env::var("PATINA_TEST_RESUME_VALIDATOR").unwrap() != "normal" {
                // Simulate a partial from a different source with the same length.
                use std::io::Write;
                std::fs::OpenOptions::new().write(true).open(&partial).unwrap().write_all(&[0; 1024]).unwrap();
            }
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
    app.set_session(Some(session("bob")), vec![]);
    // Both a 404 and an empty metadata response must leave later outbox entries usable.
    for key in ["999998", "999999"] {
        let mut missing = d.clone();
        missing.id = format!("{}:{key}", d.id);
        missing.item.rating_key = Some(key.into());
        offline::record(&missing, 45000, 180000, false, 0.9).unwrap();
    }
    offline::record(&d, 90000, 180000, false, 0.9).unwrap();
    assert_eq!(offline::position(&d), 90000);
    offline::sync(&app).await.unwrap();
    let server = session("alice").server;
    let item = server.metadata("100", MetadataOptions::default()).await.unwrap().unwrap();
    assert_eq!(item.view_offset, Some(61000), "other profile must not replay Alice's progress");
    app.set_session(Some(session("alice")), vec![]);
    sync_until(&app, &server, |item| item.view_offset == Some(90000)).await;
    let progress: Vec<offline::Progress> = patina_core::storage::read_json(&patina_core::config::config_dir().join("offline-progress.json")).unwrap();
    for key in ["999998", "999999"] {
        let missing = progress.iter().find(|p| p.rating_key == key).unwrap();
        assert!(!missing.pending, "missing titles must be retired from the outbox");
        assert_eq!(missing.position, 45000, "local resume points must survive removal from Plex");
    }
    offline::record(&d, 179000, 180000, true, 0.9).unwrap();
    sync_until(&app, &server, |item| item.is_watched()).await;
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

/// The background synchronizer may own the lease; retry until the server acknowledges the change.
async fn sync_until(app: &AppState, server: &PlexServer, expected: impl Fn(&patina_plex::Metadata) -> bool) {
    let began = Instant::now();
    loop {
        offline::sync(app).await.unwrap();
        if expected(&server.metadata("100", MetadataOptions::default()).await.unwrap().unwrap()) {
            return;
        }
        assert!(began.elapsed() < Duration::from_secs(10), "offline progress was not replayed");
        tokio::time::sleep(Duration::from_millis(100)).await;
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
    let mut next = d.clone();
    next.id = "mock:alice:next".into();
    next.item.rating_key = Some("next".into());
    patina_core::storage::update_json::<Vec<downloads::Download>, _>(&patina_core::config::config_dir().join("downloads.json"), |list| {
        list.extend([d.clone(), next.clone()]);
    })
    .unwrap();
    offline::record(&d, 8000, 20000, false, 0.9).unwrap();
    offline::record(&next, 4000, 20000, false, 0.9).unwrap();
    patina_core::ops::playback::play_downloads(&app, &[d.id.clone(), next.id.clone()]).unwrap();
    let start = Instant::now();
    loop {
        if app.now_playing().is_some_and(|n| n.position_ms > 8100 && n.position_ms < 14000) {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(8), "mpv did not resume from the saved position");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    patina_core::player::control(patina_core::player::Control::Next);
    loop {
        if app.now_playing().is_some_and(|n| n.queue_pos == 1 && n.position_ms > 5100) {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(12), "skipping lost the new entry's now-playing state");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(offline::position(&d) > 8100, "skipping must checkpoint the previous entry");
    patina_core::player::control(patina_core::player::Control::SeekTo(18.0));
    loop {
        if app.now_playing().is_none() && offline::position(&next) == 0 {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(15), "mpv did not persist completion");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    tokio::task::spawn_blocking(patina_core::player::shutdown).await.unwrap();
}

// Playback reporting ------------------------------------------------------------------

/// Pause, queue edits and stopping must not wait for Plex, however slowly (or whether) it answers reports.
#[test]
#[ignore = "requires mpv, ffmpeg and python3"]
fn mpv_slow_reports_do_not_block_playback() {
    reports_worker_run(&["--report-delay", "4"]);
}

#[test]
#[ignore = "requires mpv, ffmpeg and python3"]
fn mpv_failing_reports_do_not_block_playback() {
    reports_worker_run(&["--report-status", "503"]);
}

fn reports_worker_run(mock_args: &[&str]) {
    let temp = tempfile::tempdir().unwrap();
    let media = temp.path().join("media");
    std::fs::create_dir_all(&media).unwrap();
    assert!(
        Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "sine=frequency=440:duration=40"])
            .arg(media.join("movie.mkv"))
            .status()
            .unwrap()
            .success()
    );
    let events_path = temp.path().join("events.log");
    let mut server = Command::new("python3")
        .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/mock_pms.py"))
        .args(["--port", "0"])
        .args(mock_args)
        .arg("--media")
        .arg(&media)
        .stderr(Stdio::piped())
        .stdout(std::fs::File::create(&events_path).unwrap())
        .spawn()
        .unwrap();
    let mut log = std::io::BufReader::new(server.stderr.take().unwrap());
    let mut first = String::new();
    log.read_line(&mut first).unwrap();
    let url = first.trim().strip_prefix("mock PMS on ").expect("mock server startup").to_owned();
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "reports_worker", "--nocapture"])
        .env("PATINA_TEST_SERVER", &url)
        .env("XDG_CONFIG_HOME", temp.path().join("config"))
        .env("PATINA_DATA_DIR", temp.path().join("data"))
        .status()
        .unwrap();
    // Give the mock a moment to log the last request it was handling.
    std::thread::sleep(Duration::from_millis(500));
    let _ = server.kill();
    let _ = server.wait();
    assert!(status.success());
    let events = std::fs::read_to_string(events_path).unwrap();
    let delivered = |key: &str| {
        events.lines().any(|l| l.contains("\"what\": \"timeline\"") && l.contains(&format!("\"ratingKey\": \"{key}\"")) && l.contains("\"state\": \"stopped\""))
    };
    if mock_args.contains(&"--report-delay") {
        assert!(delivered("100") && delivered("101"), "final stopped reports must reach Plex, however late:\n{events}");
    } else {
        assert!(events.contains("failed report"), "the failing server was never asked:\n{events}");
    }
    // Progress is collapsed rather than queued up behind a slow server.
    assert!(events.lines().filter(|l| l.contains("\"what\": \"timeline\"")).count() <= 8, "{events}");
}

#[tokio::test]
async fn reports_worker() {
    let Ok(url) = std::env::var("PATINA_TEST_SERVER") else {
        return;
    };
    use patina_core::player::{self, Control, PlayRequest, QueueCommand};
    let app = AppState::new(Arc::new(Sink), tokio::runtime::Handle::current());
    app.update_settings(|s| s.player.extra_args = "--no-config --vo=null --ao=null".into());
    let server = PlexServer::new(ClientInfo::new("test", "Patina", "test"), &url, "alice").unwrap();
    app.set_session(
        Some(Session {
            tv: None,
            server: server.clone(),
            server_id: "mock-machine-0001".into(),
            server_name: "Mock".into(),
            user: None,
            owned: true,
            servers: vec![],
        }),
        vec![],
    );
    let first = server.metadata("100", MetadataOptions::default()).await.unwrap().unwrap();
    let second = server.metadata("101", MetadataOptions::default()).await.unwrap().unwrap();
    player::play(app.clone(), PlayRequest { item: first, resume: false, ..Default::default() });
    wait_for("playback to start", Duration::from_secs(15), || app.now_playing().is_some_and(|n| n.position_ms > 1500)).await;

    let began = Instant::now();
    player::control(Control::Pause);
    wait_for("pause", Duration::from_secs(3), || app.now_playing().is_some_and(|n| n.paused)).await;
    assert!(began.elapsed() < Duration::from_millis(1500), "pause took {:?}", began.elapsed());

    let began = Instant::now();
    player::queue(app.clone(), QueueCommand::Add { item: Box::new(second), next: false });
    wait_for("the queue edit", Duration::from_secs(3), || app.snapshot().queue.is_some_and(|q| q.items.len() == 2)).await;
    assert!(began.elapsed() < Duration::from_millis(1500), "queue edit took {:?}", began.elapsed());

    player::control(Control::Play);
    let began = Instant::now();
    player::control(Control::Next);
    wait_for("the next item", Duration::from_secs(3), || app.now_playing().is_some_and(|n| n.queue_pos == 1 && !n.paused)).await;
    assert!(began.elapsed() < Duration::from_millis(1500), "skipping took {:?}", began.elapsed());

    let began = Instant::now();
    player::control(Control::Stop);
    wait_for("playback to end", Duration::from_secs(6), || app.now_playing().is_none()).await;
    assert!(began.elapsed() < Duration::from_secs(4), "stopping took {:?}", began.elapsed());
    patina_core::report::flush(Duration::from_secs(40)).await;
}

async fn wait_for(what: &str, within: Duration, mut done: impl FnMut() -> bool) {
    let began = Instant::now();
    while !done() {
        assert!(began.elapsed() < within, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

// Failures, controls and recovery ------------------------------------------------------

/// A file mpv cannot play is reported with a reason and can be retried; controls, diagnostics and the
/// saved queue work against a real mpv.
#[test]
#[ignore = "requires mpv, ffmpeg and python3"]
fn mpv_failure_controls_and_resume() {
    let temp = tempfile::tempdir().unwrap();
    let media = temp.path().join("media");
    std::fs::create_dir_all(&media).unwrap();
    // Not a media file: the first attempt fails, and the worker swaps in a real one before retrying.
    std::fs::write(media.join("movie.mkv"), vec![7u8; 4096]).unwrap();
    let mut server = Command::new("python3")
        .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/mock_pms.py"))
        .args(["--port", "0", "--media"])
        .arg(&media)
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let mut log = std::io::BufReader::new(server.stderr.take().unwrap());
    let mut first = String::new();
    log.read_line(&mut first).unwrap();
    let url = first.trim().strip_prefix("mock PMS on ").expect("mock server startup").to_owned();
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "failure_worker", "--nocapture"])
        .env("PATINA_TEST_SERVER", &url)
        .env("PATINA_TEST_MEDIA", &media)
        .env("XDG_CONFIG_HOME", temp.path().join("config"))
        .env("PATINA_DATA_DIR", temp.path().join("data"))
        .status()
        .unwrap();
    let _ = server.kill();
    let _ = server.wait();
    assert!(status.success());
}

/// Keeps the events a front end would receive.
#[derive(Default)]
struct Recorder(std::sync::Mutex<Vec<(&'static str, serde_json::Value)>>);

impl EventSink for Recorder {
    fn emit(&self, event: CoreEvent) {
        self.0.lock().unwrap().push((event.name(), event.payload()));
    }
}

impl Recorder {
    fn last(&self, name: &str) -> Option<serde_json::Value> {
        self.0.lock().unwrap().iter().rev().find(|(n, _)| *n == name).map(|(_, v)| v.clone())
    }
}

#[tokio::test]
async fn failure_worker() {
    let Ok(url) = std::env::var("PATINA_TEST_SERVER") else {
        return;
    };
    use patina_core::controls::{Repeat, Tune};
    use patina_core::failure::{self, Recover};
    use patina_core::player::{self, Control, PlayRequest};
    let media = std::path::PathBuf::from(std::env::var("PATINA_TEST_MEDIA").unwrap());
    let events = Arc::new(Recorder::default());
    let app = AppState::new(events.clone(), tokio::runtime::Handle::current());
    app.update_settings(|s| s.player.extra_args = "--no-config --vo=null --ao=null".into());
    let server = PlexServer::new(ClientInfo::new("test", "Patina", "test"), &url, "alice").unwrap();
    app.set_session(
        Some(Session {
            tv: None,
            server: server.clone(),
            server_id: "mock-machine-0001".into(),
            server_name: "Mock".into(),
            user: None,
            owned: true,
            servers: vec![],
        }),
        vec![],
    );
    let item = server.metadata("100", MetadataOptions::default()).await.unwrap().unwrap();

    // A file mpv cannot open: the failure arrives with a reason, before anything loaded.
    player::play(app.clone(), PlayRequest { item: item.clone(), resume: false, ..Default::default() });
    wait_for("the failure", Duration::from_secs(20), || failure::current().is_some()).await;
    let f = failure::current().unwrap();
    println!("failure: {} / {:?} / {:?} / {:?}", f.reason, f.detail, f.kind, f.position_ms);
    assert_eq!(f.phase, failure::Phase::Start);
    assert!(!f.reason.is_empty() && f.can_retry);
    assert_eq!(f.position_ms, 0, "the item was meant to start from the beginning");
    assert_eq!(f.lower_quality.as_ref().map(|q| q.id.as_str()), Some("20m-1080"));
    assert_eq!(events.last("playback-failure").unwrap()["item"]["ratingKey"], "100");
    wait_for("playback to end", Duration::from_secs(10), || app.now_playing().is_none() && !patina_core::player::is_active()).await;

    // Fix the file and retry: playback starts and the failure is gone.
    let fixed = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "sine=frequency=440:duration=300"])
        .arg(media.join("movie.mkv"))
        .status()
        .unwrap();
    assert!(fixed.success());
    failure::recover(&app, Recover::Retry).unwrap();
    wait_for("playback after retry", Duration::from_secs(15), || app.now_playing().is_some_and(|n| n.position_ms > 1000)).await;
    assert!(failure::current().is_none());
    assert!(events.last("playback-failure").unwrap().is_null());

    // Controls and diagnostics.
    player::tune(&app, Tune::Volume { value: 40.0 });
    player::tune(&app, Tune::Speed { value: 1.5 });
    player::tune(&app, Tune::Repeat { mode: Repeat::All });
    player::tune(&app, Tune::SleepMinutes { minutes: 30 });
    wait_for("the controls to apply", Duration::from_secs(5), || {
        patina_core::controls::playback_state().is_some_and(|s| s.volume == 40.0 && s.speed == 1.5 && s.repeat == Repeat::All && s.sleep.is_some())
    })
    .await;
    let state = events.last("playback-state").unwrap();
    assert_eq!(state["repeat"], "all");
    assert_eq!(state["sleep"]["mode"], "minutes");
    assert_eq!(state["diagnostics"]["connection"]["kind"], "local");
    assert!(state["diagnostics"]["bufferedSecs"].is_number(), "{state}");
    player::tune(&app, Tune::SleepEndOfItem);
    wait_for("end-of-item sleep", Duration::from_secs(5), || app.now_playing().is_some_and(|n| n.stop_after)).await;
    player::tune(&app, Tune::SleepOff);
    wait_for("sleep off", Duration::from_secs(5), || app.now_playing().is_some_and(|n| !n.stop_after)).await;
    player::tune(&app, Tune::Mute { on: Some(true) });
    wait_for("mute", Duration::from_secs(5), || patina_core::controls::playback_state().is_some_and(|s| s.muted)).await;
    player::tune(&app, Tune::Repeat { mode: Repeat::Off });

    // The queue is saved while playing, and offered again after stopping.
    wait_for("a saved queue", Duration::from_secs(15), || patina_core::ops::playback::saved_queue(&app).is_some_and(|q| q.position_ms > 1000)).await;
    player::control(Control::Stop);
    wait_for("playback to end", Duration::from_secs(8), || app.now_playing().is_none() && !patina_core::player::is_active()).await;
    let saved = patina_core::ops::playback::saved_queue(&app).expect("the queue stays saved after stopping");
    assert_eq!(saved.current.rating_key(), "100");
    println!("saved at {} ms", saved.position_ms);

    patina_core::ops::playback::resume_saved(&app).unwrap();
    wait_for("the resumed playback", Duration::from_secs(15), || app.now_playing().is_some_and(|n| n.position_ms >= saved.position_ms - 1000)).await;
    assert!(app.now_playing().unwrap().position_ms < saved.position_ms + 15_000, "resumed at the saved position, not the start");

    // Stopping and forgetting clears the offer; a finished queue is not offered.
    player::control(Control::Stop);
    wait_for("playback to end", Duration::from_secs(8), || app.now_playing().is_none() && !patina_core::player::is_active()).await;
    patina_core::ops::playback::forget_saved(&app);
    assert!(patina_core::ops::playback::saved_queue(&app).is_none());
    patina_core::report::flush(Duration::from_secs(10)).await;
}
