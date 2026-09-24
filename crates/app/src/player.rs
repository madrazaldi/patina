//! Playback through an external mpv process.
//!
//! A play request becomes a Plex play queue. Every queue item is appended to
//! mpv's playlist so mpv's own next/previous bindings work, while we follow
//! along over IPC: reporting timeline progress, marking items watched,
//! mirroring audio/subtitle choices between mpv and Plex, and offering
//! intro/credits skipping from Plex markers.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use patina_mpv::{Event, Mpv, SpawnOptions};
use patina_plex::{
    Marker, Metadata, MetadataOptions, Part, PlayQueueRequest, PlexServer, TimelineState, TimelineUpdate,
    TranscodeOptions, stream_type,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::config::PlayerSettings;
use crate::state::{AppState, NowPlaying};

static ACTIVE: Mutex<Option<Arc<Mpv>>> = Mutex::new(None);

/// What to play.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlayRequest {
    pub item: Metadata,
    /// Resume from the saved position (otherwise start from the beginning).
    pub resume: bool,
    pub shuffle: bool,
    /// Which version (media) of the starting item to play.
    pub media_index: usize,
}

impl Default for PlayRequest {
    fn default() -> Self {
        Self { item: Metadata::default(), resume: true, shuffle: false, media_index: 0 }
    }
}

/// Remote-control commands for the running player.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(tag = "kind", content = "seconds", rename_all = "camelCase")]
pub enum Control {
    TogglePause,
    Next,
    Previous,
    Stop,
    SeekRelative(f64),
}

pub fn play(app: AppState, req: PlayRequest) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run(app.clone(), req).await {
            tracing::error!("playback failed: {e:#}");
            app.error(format!("Playback failed: {e:#}"));
            app.set_now_playing(None);
        }
    });
}

pub fn control(cmd: Control) {
    let Some(mpv) = ACTIVE.lock().unwrap().clone() else { return };
    tauri::async_runtime::spawn(async move {
        let _ = match cmd {
            Control::TogglePause => mpv.command(vec![json!("cycle"), json!("pause")]).await.map(|_| ()),
            Control::Next => mpv.playlist_next().await,
            Control::Previous => mpv.playlist_prev().await,
            Control::Stop => mpv.quit().await,
            Control::SeekRelative(s) => mpv.command(vec![json!("seek"), json!(s), json!("relative")]).await.map(|_| ()),
        };
    });
}

/// Close mpv when Patina quits. The app exits without running destructors,
/// so `kill_on_drop` alone would leave the player window behind.
pub fn shutdown() {
    let Some(mpv) = ACTIVE.lock().unwrap().take() else { return };
    tauri::async_runtime::block_on(async move {
        let _ = mpv.quit().await;
        mpv.wait(Duration::from_secs(2)).await;
    });
}

struct Queue {
    items: Vec<Metadata>,
    start: usize,
    kind: &'static str,
}

async fn build_queue(server: &PlexServer, req: &PlayRequest, settings: &PlayerSettings) -> Result<Queue> {
    let item = &req.item;
    let rk = item.rating_key().to_string();
    let kind: &'static str = match item.kind() {
        "track" | "album" | "artist" => "audio",
        "photo" | "photoalbum" => "photo",
        "playlist" => match item.playlist_type.as_deref() {
            Some("audio") => "audio",
            Some("photo") => "photo",
            _ => "video",
        },
        _ => "video",
    };
    let mut pq = PlayQueueRequest { kind: kind.into(), shuffle: req.shuffle, ..Default::default() };
    match item.kind() {
        "playlist" => pq.playlist_id = Some(rk.clone()),
        "collection" => {
            let children = server.collection_items(&rk).await?.metadata;
            let keys: Vec<&str> = children.iter().map(|m| m.rating_key()).collect();
            pq.uri = Some(server.item_uri(&keys));
        }
        "show" | "season" | "artist" | "album" => {
            pq.uri = Some(server.item_uri(&[&rk]));
            if !req.shuffle {
                pq.key = item.on_deck_item().and_then(|m| m.key.clone());
            }
        }
        _ => {
            pq.uri = Some(server.item_uri(&[&rk]));
            pq.key = item.key.clone();
            pq.continuous = item.kind() == "episode" && settings.continuous;
        }
    }

    match server.create_play_queue(&pq).await {
        Ok(mc) if !mc.metadata.is_empty() => {
            let start = mc
                .metadata
                .iter()
                .position(|m| m.play_queue_item_id.is_some() && m.play_queue_item_id == mc.play_queue_selected_item_id)
                .unwrap_or(0);
            Ok(Queue { items: mc.metadata, start, kind })
        }
        Ok(_) | Err(_) if item.is_playable() => Ok(Queue { items: vec![item.clone()], start: 0, kind }),
        Ok(_) => Err(anyhow!("nothing to play")),
        Err(e) => Err(e).context("creating play queue"),
    }
}

fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut has = false;
    for c in s.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                has = true
            }
            (None, c) if c.is_whitespace() => {
                if has || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if has || !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn part_of(item: &Metadata, media_index: usize) -> Option<&Part> {
    item.media.get(media_index).or(item.media.first())?.parts.first()
}

struct Loaded {
    url: String,
    transcode_session: Option<String>,
}

fn media_url(server: &PlexServer, item: &Metadata, media_index: usize, settings: &PlayerSettings) -> Result<Loaded> {
    let quality = settings.quality();
    let video = matches!(item.kind(), "movie" | "episode" | "clip" | "trailer");
    if let (Some(kbps), true) = (quality.kbps, video) {
        let session = uuid::Uuid::new_v4().to_string();
        let opts = TranscodeOptions {
            session: session.clone(),
            media_index,
            max_video_bitrate: Some(kbps),
            video_resolution: Some(quality.resolution.to_string()),
            subtitles: "burn".into(),
            ..Default::default()
        };
        let key = item.key.as_deref().ok_or_else(|| anyhow!("item has no key"))?;
        let url = server.transcode_url(key, &opts)?;
        return Ok(Loaded { url: url.to_string(), transcode_session: Some(session) });
    }
    let part = part_of(item, media_index).ok_or_else(|| anyhow!("\"{}\" has no playable file", item.title()))?;
    let key = part.key.as_deref().ok_or_else(|| anyhow!("media part has no key"))?;
    Ok(Loaded { url: server.url(key)?.to_string(), transcode_session: None })
}

/// Per-item playback state.
struct Current {
    index: usize,
    item: Metadata,
    media_index: usize,
    time_ms: i64,
    duration_ms: i64,
    paused: bool,
    reached_end: bool,
    transcode_session: Option<String>,
    /// Ignore track changes until this instant (we are applying Plex's choice).
    applying_until: Instant,
    /// Sidecar subtitle URL -> Plex stream id.
    sidecars: HashMap<String, i64>,
    active_marker: Option<Marker>,
    prompted_marker: Option<i64>,
    last_ui: Instant,
}

impl Current {
    fn part(&self) -> Option<&Part> {
        part_of(&self.item, self.media_index)
    }
}

async fn run(app: AppState, req: PlayRequest) -> Result<()> {
    let previous = ACTIVE.lock().unwrap().take();
    if let Some(old) = previous {
        let _ = old.quit().await;
        old.wait(Duration::from_secs(3)).await;
    }

    let server = app.server()?;
    let settings = app.settings().player;
    let queue = build_queue(&server, &req, &settings).await?;
    let is_photo = queue.kind == "photo";

    let info = server.client_info().clone();
    let (mpv, mut events) = Mpv::spawn(SpawnOptions {
        program: settings.mpv_program(),
        extra_args: split_args(&settings.extra_args),
        http_headers: vec![
            ("X-Plex-Token".into(), server.token().to_string()),
            ("X-Plex-Client-Identifier".into(), info.identifier.clone()),
            ("X-Plex-Product".into(), info.product.clone()),
        ],
    })
    .await
    .context("starting mpv (set its path in Settings → Player if it is not on PATH)")?;
    *ACTIVE.lock().unwrap() = Some(mpv.clone());

    for (i, name) in ["playlist-pos", "time-pos", "pause", "duration", "aid", "sid", "idle-active", "eof-reached"]
        .iter()
        .enumerate()
    {
        mpv.observe_property(i as u64 + 1, name).await?;
    }
    if !settings.skip_key.trim().is_empty() {
        let _ = mpv.bind_message(settings.skip_key.trim(), "patina-skip").await;
    }

    // Resolve stream URLs up front; items that cannot be played are dropped so
    // mpv playlist positions line up with `items`.
    let mut items = Vec::new();
    let mut transcode_sessions = Vec::new();
    let mut start = 0;
    for (i, item) in queue.items.iter().enumerate() {
        let media_index = if i == queue.start { req.media_index } else { 0 };
        let loaded = match media_url(&server, item, media_index, &settings) {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!("skipping queue item: {e}");
                continue;
            }
        };
        if i == queue.start {
            start = items.len();
        }
        let mut opts = vec![("force-media-title", item.full_title())];
        if i == queue.start && req.resume
            && let Some(offset) = item.view_offset.filter(|o| *o > 0) {
                opts.push(("start", format!("{:.3}", offset as f64 / 1000.0)));
            }
        mpv.loadfile(&loaded.url, "append", &opts).await?;
        transcode_sessions.push(loaded.transcode_session);
        items.push((item.clone(), media_index));
    }
    if items.is_empty() {
        let _ = mpv.quit().await;
        return Err(anyhow!("nothing in the queue could be played"));
    }
    mpv.set_property("playlist-pos", start as i64).await?;

    let queue_len = items.len();
    let mut cur: Option<Current> = None;
    let mut played_any = false;
    let mut ticker = tokio::time::interval(Duration::from_secs(10));
    ticker.tick().await;

    loop {
        tokio::select! {
            ev = events.recv() => {
                let Some(ev) = ev else { break };
                match ev {
                    Event::Shutdown => break,
                    Event::PropertyChange { name, data, .. } => match name.as_str() {
                        "playlist-pos" => {
                            let pos = data.as_i64().unwrap_or(-1);
                            if cur.as_ref().map(|c| c.index as i64) != Some(pos) {
                                if let Some(c) = cur.take() {
                                    finish(&app, &server, &settings, c, false).await;
                                }
                                if pos >= 0 && (pos as usize) < items.len() {
                                    let index = pos as usize;
                                    let (item, media_index) = &items[index];
                                    let session = transcode_sessions[index].clone();
                                    cur = Some(begin(&server, item, index, *media_index, session).await);
                                    if let Some(c) = cur.as_mut() {
                                        publish(&app, c, queue_len, true);
                                        if !is_photo { report(&server, c, TimelineState::Playing).await; }
                                    }
                                }
                            }
                        }
                        "time-pos" => if let Some(c) = cur.as_mut()
                            && let Some(t) = data.as_f64() {
                                c.time_ms = (t * 1000.0) as i64;
                                handle_markers(&mpv, &settings, c, queue_len).await;
                                if c.last_ui.elapsed() >= Duration::from_millis(900) {
                                    publish(&app, c, queue_len, false);
                                }
                            },
                        "duration" => if let Some(c) = cur.as_mut()
                            && let Some(d) = data.as_f64() { c.duration_ms = (d * 1000.0) as i64; },
                        "pause" => if let Some(c) = cur.as_mut() {
                            let paused = data.as_bool().unwrap_or(false);
                            if paused != c.paused {
                                c.paused = paused;
                                publish(&app, c, queue_len, true);
                                if !is_photo {
                                    report(&server, c, if paused { TimelineState::Paused } else { TimelineState::Playing }).await;
                                }
                            }
                        },
                        "eof-reached" => if let Some(c) = cur.as_mut() {
                            // With keep-open=yes mpv parks on the last frame instead of ending the file.
                            if data.as_bool() == Some(true) { c.reached_end = true; }
                        },
                        "aid" | "sid" => if let Some(c) = cur.as_ref()
                            && settings.sync_streams && c.transcode_session.is_none() && Instant::now() > c.applying_until {
                                sync_stream_choice(&server, &mpv, c, &name, &data).await;
                            },
                        "idle-active"
                            if data.as_bool() == Some(true) && played_any => {
                                let _ = mpv.quit().await;
                            }
                        _ => {}
                    },
                    Event::FileLoaded => {
                        played_any = true;
                        if let Some(c) = cur.as_mut()
                            && settings.sync_streams && c.transcode_session.is_none() {
                                apply_plex_streams(&server, &mpv, c).await;
                            }
                    }
                    Event::EndFile { reason, .. } => {
                        if let Some(mut c) = cur.take() {
                            let completed = reason == "eof";
                            c.reached_end |= completed;
                            finish(&app, &server, &settings, c, completed).await;
                        }
                    }
                    Event::ClientMessage { args } => {
                        if args.first().map(String::as_str) == Some("patina-skip")
                            && let Some(c) = cur.as_mut()
                                && let Some(m) = c.active_marker.clone() {
                                    skip_marker(&mpv, c, &m, queue_len).await;
                                }
                    }
                    _ => {}
                }
            }
            _ = ticker.tick() => {
                if let Some(c) = cur.as_ref() {
                    if !is_photo {
                        report(&server, c, if c.paused { TimelineState::Paused } else { TimelineState::Playing }).await;
                    }
                    if let Some(session) = &c.transcode_session {
                        let _ = server.ping_transcode(session).await;
                    }
                }
            }
        }
    }

    if let Some(c) = cur.take() {
        let completed = c.reached_end;
        finish(&app, &server, &settings, c, completed).await;
    }
    for session in transcode_sessions.into_iter().flatten() {
        let _ = server.stop_transcode(&session).await;
    }
    {
        let mut active = ACTIVE.lock().unwrap();
        if active.as_ref().is_some_and(|a| Arc::ptr_eq(a, &mpv)) {
            *active = None;
        }
    }
    mpv.wait(Duration::from_secs(2)).await;
    let still_ours = ACTIVE.lock().unwrap().is_none();
    if still_ours {
        app.set_now_playing(None);
    }
    app.bump();
    Ok(())
}

async fn begin(server: &PlexServer, item: &Metadata, index: usize, media_index: usize, transcode_session: Option<String>) -> Current {
    // Queue entries lack markers and sometimes streams; fetch the full item.
    let full = match item.rating_key.as_deref() {
        Some(rk) => server.metadata(rk, MetadataOptions::playback()).await.ok().flatten(),
        None => None,
    };
    let mut full = full.unwrap_or_else(|| item.clone());
    full.play_queue_item_id = item.play_queue_item_id;
    Current {
        index,
        duration_ms: full.duration.unwrap_or(0),
        item: full,
        media_index,
        time_ms: 0,
        paused: false,
        reached_end: false,
        transcode_session,
        applying_until: Instant::now() + Duration::from_secs(3),
        sidecars: HashMap::new(),
        active_marker: None,
        prompted_marker: None,
        last_ui: Instant::now() - Duration::from_secs(5),
    }
}

fn publish(app: &AppState, c: &mut Current, queue_len: usize, _force: bool) {
    c.last_ui = Instant::now();
    app.set_now_playing(Some(NowPlaying {
        item: c.item.clone(),
        position_ms: c.time_ms,
        duration_ms: c.duration_ms,
        paused: c.paused,
        transcoding: c.transcode_session.is_some(),
        queue_len,
        queue_pos: c.index,
    }));
}

async fn report(server: &PlexServer, c: &Current, state: TimelineState) {
    let (Some(rk), Some(key)) = (c.item.rating_key.clone(), c.item.key.clone()) else { return };
    let update = TimelineUpdate {
        rating_key: rk,
        key,
        state,
        time_ms: c.time_ms,
        duration_ms: c.duration_ms,
        play_queue_item_id: c.item.play_queue_item_id,
        session: c.transcode_session.clone(),
    };
    if let Err(e) = server.timeline(&update).await {
        tracing::debug!("timeline report failed: {e}");
    }
}

async fn finish(app: &AppState, server: &PlexServer, settings: &PlayerSettings, c: Current, completed: bool) {
    if c.item.kind() == "photo" {
        return;
    }
    let mut c = c;
    let progress = if c.duration_ms > 0 { c.time_ms as f64 / c.duration_ms as f64 } else { 0.0 };
    let watched = completed || c.reached_end || progress >= settings.watched_threshold;
    if watched && c.duration_ms > 0 {
        c.time_ms = c.duration_ms;
    }
    report(server, &c, TimelineState::Stopped).await;
    if watched
        && let Some(rk) = c.item.rating_key.as_deref()
            && let Err(e) = server.scrobble(rk).await {
                tracing::warn!("scrobble failed: {e}");
            }
    app.bump();
}

async fn handle_markers(mpv: &Mpv, settings: &PlayerSettings, c: &mut Current, queue_len: usize) {
    let t = c.time_ms;
    let marker = c
        .item
        .markers
        .iter()
        .find(|m| {
            matches!(m.kind.as_deref(), Some("intro" | "credits"))
                && m.start_time_offset.unwrap_or(i64::MAX) <= t
                && t < m.end_time_offset.unwrap_or(0) - 1000
        })
        .cloned();
    c.active_marker = marker.clone();
    let Some(m) = marker else { return };
    if c.prompted_marker == m.id {
        return;
    }
    c.prompted_marker = m.id;
    let is_intro = m.kind.as_deref() == Some("intro");
    let auto = if is_intro { settings.auto_skip_intro } else { settings.auto_skip_credits };
    if auto {
        skip_marker(mpv, c, &m, queue_len).await;
    } else {
        let what = if is_intro { "intro" } else { "credits" };
        let key = settings.skip_key.trim();
        let _ = mpv.show_text(&format!("Press {key} to skip {what}"), 5000).await;
    }
}

async fn skip_marker(mpv: &Mpv, c: &mut Current, m: &Marker, queue_len: usize) {
    let is_credits = m.kind.as_deref() == Some("credits");
    c.active_marker = None;
    if is_credits && m.is_final && c.index + 1 < queue_len {
        c.reached_end = true;
        let _ = mpv.show_text("Next episode", 1500).await;
        let _ = mpv.playlist_next().await;
        return;
    }
    if let Some(end) = m.end_time_offset {
        let _ = mpv.seek_absolute(end as f64 / 1000.0).await;
        let _ = mpv.show_text(if is_credits { "Skipped credits" } else { "Skipped intro" }, 1500).await;
    }
}

fn tracks(list: &Value) -> Vec<&Value> {
    list.as_array().map(|a| a.iter().collect()).unwrap_or_default()
}

/// Select mpv tracks matching the streams chosen in Plex, and add sidecar subtitles.
async fn apply_plex_streams(server: &PlexServer, mpv: &Mpv, c: &mut Current) {
    c.applying_until = Instant::now() + Duration::from_secs(3);
    let Some(part) = c.part().cloned() else { return };
    let Ok(list) = mpv.get_property("track-list").await else { return };
    let find = |kind: &str, ff_index: i64| {
        tracks(&list)
            .into_iter()
            .find(|t| t["type"] == kind && t["ff-index"].as_i64() == Some(ff_index) && t["external"] != true)
            .and_then(|t| t["id"].as_i64())
    };

    if let Some(audio) = part.audio_streams().find(|s| s.selected)
        && let Some(id) = audio.index.and_then(|i| find("audio", i)) {
            let _ = mpv.set_property("aid", id).await;
        }

    let selected_sub = part.subtitle_streams().find(|s| s.selected).cloned();
    for s in part.subtitle_streams().filter(|s| s.is_external()) {
        let (Some(key), Some(id)) = (s.key.as_deref(), s.id) else { continue };
        let Ok(url) = server.url(key) else { continue };
        let url = url.to_string();
        let flag = if selected_sub.as_ref().and_then(|x| x.id) == Some(id) { "select" } else { "auto" };
        let lang = s.language_code.clone().unwrap_or_default();
        if mpv.sub_add(&url, flag, &s.short_label(), &lang).await.is_ok() {
            c.sidecars.insert(url, id);
        }
    }
    match &selected_sub {
        Some(s) if !s.is_external() => {
            if let Some(id) = s.index.and_then(|i| find("sub", i)) {
                let _ = mpv.set_property("sid", id).await;
            }
        }
        Some(_) => {}
        None => {
            let _ = mpv.set_property("sid", "no").await;
        }
    }
    c.applying_until = Instant::now() + Duration::from_millis(1500);
}

/// Save a track choice made in mpv back to Plex so other apps agree.
async fn sync_stream_choice(server: &PlexServer, mpv: &Mpv, c: &Current, prop: &str, data: &Value) {
    let Some(part) = c.part() else { return };
    let Some(part_id) = part.id else { return };
    let disabled = data == &Value::Bool(false) || data.as_str() == Some("no");
    let stream_id = if disabled {
        (prop == "sid").then_some(0)
    } else {
        let Some(track_id) = data.as_i64() else { return };
        let Ok(list) = mpv.get_property("track-list").await else { return };
        let kind = if prop == "aid" { "audio" } else { "sub" };
        let Some(track) = tracks(&list).into_iter().find(|t| t["type"] == kind && t["id"].as_i64() == Some(track_id)) else {
            return;
        };
        if track["external"] == true {
            track["external-filename"].as_str().and_then(|f| c.sidecars.get(f).copied())
        } else {
            let wanted = if prop == "aid" { stream_type::AUDIO } else { stream_type::SUBTITLE };
            let ff = track["ff-index"].as_i64();
            part.streams.iter().find(|s| s.stream_type == Some(wanted) && s.index == ff && ff.is_some()).and_then(|s| s.id)
        }
    };
    let Some(stream_id) = stream_id else { return };
    let (audio, sub) = if prop == "aid" { (Some(stream_id), None) } else { (None, Some(stream_id)) };
    if let Err(e) = server.select_streams(part_id, audio, sub, true).await {
        tracing::debug!("could not save stream choice: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::split_args;

    #[test]
    fn splits_quoted_args() {
        assert_eq!(split_args(r#"--fs --title="a b" 'x y'"#), vec!["--fs", "--title=a b", "x y"]);
        assert!(split_args("  ").is_empty());
    }
}
