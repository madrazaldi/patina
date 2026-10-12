//! Playback through an external mpv process.
//!
//! A play request becomes a Plex play queue. Every queue item is appended to
//! mpv's playlist so mpv's own next/previous bindings work, while we follow
//! along over IPC: reporting timeline progress, marking items watched,
//! mirroring audio/subtitle choices between mpv and Plex, and offering
//! intro/credits skipping from Plex markers.
//!
//! The queue can be edited while it plays (play next, remove, reorder,
//! shuffle). Edits are applied to mpv's playlist and mirrored to the Plex
//! play queue where it has the items.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
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
use tokio::sync::mpsc;

use crate::config::{self, PlayerSettings, Quality};
use crate::state::{AppState, NowPlaying, QueueItem, QueueView};

static ACTIVE: Mutex<Option<Arc<Mpv>>> = Mutex::new(None);
static COMMANDS: Mutex<Option<mpsc::UnboundedSender<QueueCommand>>> = Mutex::new(None);
static NEXT_UID: AtomicU64 = AtomicU64::new(1);

fn uid() -> u64 {
    NEXT_UID.fetch_add(1, Ordering::Relaxed)
}

/// What to play.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlayRequest {
    pub item: Metadata,
    /// Explicit bulk selection; disables automatic episode expansion.
    pub selection: Vec<Metadata>,
    /// Resume from the saved position (otherwise start from the beginning).
    pub resume: bool,
    pub shuffle: bool,
    /// Which version (media) of the starting item to play.
    pub media_index: usize,
    /// Play the item's radio station instead of the item (music).
    pub radio: bool,
    /// A tuned Live TV session; always transcoded, never scrobbled.
    #[serde(skip)]
    pub live: bool,
    /// Quality preset id or label for this playback; the settings' otherwise.
    pub quality: Option<String>,
}

impl Default for PlayRequest {
    fn default() -> Self {
        Self { item: Metadata::default(), selection: Vec::new(), resume: true, shuffle: false, media_index: 0, radio: false, live: false, quality: None }
    }
}

/// Remote-control commands for the running player.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(tag = "kind", content = "seconds", rename_all = "camelCase")]
pub enum Control {
    TogglePause,
    Play,
    Pause,
    Next,
    Previous,
    Stop,
    SeekRelative(f64),
    SeekTo(f64),
}

/// Edits to the running queue. Entries are addressed by the `uid` sent in
/// [`QueueView`], which stays stable while the queue is reordered.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum QueueCommand {
    /// Queue an item (or every item of a container) next or at the end.
    Add { item: Box<Metadata>, next: bool },
    Remove { uid: u64 },
    /// Move an entry before another one, or to the end.
    Move { uid: u64, before: Option<u64> },
    Jump { uid: u64 },
    /// Shuffle what comes after the current item, or restore its order.
    Shuffle { on: bool },
    /// Quit when the current item ends instead of moving on.
    StopAfter { on: bool },
}

pub fn play(app: AppState, req: PlayRequest) {
    app.clone().spawn(async move {
        if let Err(e) = run(app.clone(), req).await {
            tracing::error!("playback failed: {e:#}");
            app.error(format!("Playback failed: {e:#}"));
            app.set_now_playing(None);
            app.set_queue(None);
        }
    });
}

pub fn control(cmd: Control) {
    let Some(mpv) = ACTIVE.lock().unwrap().clone() else { return };
    crate::runtime().spawn(async move {
        let _ = match cmd {
            Control::TogglePause => mpv.command(vec![json!("cycle"), json!("pause")]).await.map(|_| ()),
            Control::Play => mpv.set_pause(false).await,
            Control::Pause => mpv.set_pause(true).await,
            Control::Next => mpv.playlist_next().await,
            Control::Previous => mpv.playlist_prev().await,
            Control::Stop => mpv.quit().await,
            Control::SeekRelative(s) => mpv.command(vec![json!("seek"), json!(s), json!("relative")]).await.map(|_| ()),
            Control::SeekTo(s) => mpv.seek_absolute(s).await,
        };
    });
}

/// Enqueue a selection together, including when no player is running yet.
pub fn queue_items(app: AppState, items: Vec<Metadata>) -> Result<()> {
    let Some(first) = items.first().cloned() else { return Ok(()); };
    anyhow::ensure!(items.iter().all(|m| queue_kind(m) == queue_kind(&first)), "Select only video or only music for a queue.");
    let sender = COMMANDS.lock().unwrap().clone().filter(|tx| !tx.is_closed());
    if let Some(tx) = sender {
        for item in items { tx.send(QueueCommand::Add { item: Box::new(item), next: false }).map_err(|_| anyhow!("Playback ended; try again."))?; }
    } else {
        play(app, PlayRequest { item: first, selection: items, ..Default::default() });
    }
    Ok(())
}

/// Edit the running queue. Adding with nothing playing starts playback.
pub fn queue(app: AppState, cmd: QueueCommand) {
    let tx = COMMANDS.lock().unwrap().clone();
    match (tx, cmd) {
        (Some(tx), cmd) if !tx.is_closed() => {
            let _ = tx.send(cmd);
        }
        (_, QueueCommand::Add { item, .. }) => play(app, PlayRequest { item: *item, ..Default::default() }),
        _ => {}
    }
}

/// Close mpv when Patina quits. The app exits without running destructors,
/// so `kill_on_drop` alone would leave the player window behind. Call from
/// outside the runtime.
pub fn shutdown() {
    let Some(mpv) = ACTIVE.lock().unwrap().take() else { return };
    crate::runtime().block_on(async move {
        let _ = mpv.quit().await;
        mpv.wait(Duration::from_secs(2)).await;
    });
}

struct Queue {
    items: Vec<Metadata>,
    start: usize,
    kind: &'static str,
    id: Option<i64>,
}

fn queue_kind(item: &Metadata) -> &'static str {
    match item.kind() {
        "track" | "album" | "artist" => "audio",
        "photo" | "photoalbum" => "photo",
        "playlist" => match item.playlist_type.as_deref() {
            Some("audio") => "audio",
            Some("photo") => "photo",
            _ => "video",
        },
        _ => "video",
    }
}

async fn build_queue(server: &PlexServer, req: &PlayRequest, settings: &PlayerSettings) -> Result<Queue> {
    let item = &req.item;
    if req.live {
        return Ok(Queue { items: vec![item.clone()], start: 0, kind: "video", id: None });
    }
    let rk = item.rating_key().to_string();
    let kind = queue_kind(item);
    let mut pq = PlayQueueRequest { kind: kind.into(), shuffle: req.shuffle, ..Default::default() };
    if !req.selection.is_empty() {
        let keys: Vec<&str> = req.selection.iter().map(|m| m.rating_key()).collect();
        pq.uri = Some(server.item_uri(&keys));
    } else if req.radio {
        let station = server.station(&rk).await.ok().flatten().and_then(|s| s.key);
        match station {
            Some(key) => pq.uri = Some(server.library_uri(&key)),
            None => {
                // No sonic analysis: shuffle the artist instead.
                let artist = match item.kind() {
                    "track" => item.grandparent_rating_key.clone(),
                    "album" => item.parent_rating_key.clone(),
                    _ => Some(rk.clone()),
                };
                pq.uri = Some(server.item_uri(&[artist.as_deref().unwrap_or(&rk)]));
                pq.shuffle = true;
            }
        }
        pq.kind = "audio".into();
    } else {
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
    }

    match server.create_play_queue(&pq).await {
        Ok(mc) if !mc.metadata.is_empty() => {
            let start = mc
                .metadata
                .iter()
                .position(|m| m.play_queue_item_id.is_some() && m.play_queue_item_id == mc.play_queue_selected_item_id)
                .unwrap_or(0);
            Ok(Queue { items: mc.metadata, start, kind, id: mc.play_queue_id })
        }
        Ok(_) | Err(_) if !req.selection.is_empty() => {
            let mut items = Vec::new();
            for item in &req.selection { items.extend(leaves(server, item).await?); }
            if items.is_empty() { return Err(anyhow!("nothing to play")); }
            Ok(Queue { items, start: 0, kind, id: None })
        },
        Ok(_) | Err(_) if item.is_playable() => Ok(Queue { items: vec![item.clone()], start: 0, kind, id: None }),
        Ok(_) => Err(anyhow!("nothing to play")),
        Err(e) => Err(e).context("creating play queue"),
    }
}

/// The playable items of an item: itself, or everything below a container.
async fn leaves(server: &PlexServer, item: &Metadata) -> Result<Vec<Metadata>> {
    if item.is_playable() {
        return Ok(vec![item.clone()]);
    }
    let rk = item.rating_key();
    let children = match item.kind() {
        "playlist" => server.playlist_items(rk, 0, 5000).await?.metadata,
        "collection" => server.collection_items(rk).await?.metadata,
        _ => return Ok(server.all_leaves(rk).await?.metadata),
    };
    let mut out = Vec::new();
    for child in children {
        if child.is_playable() {
            out.push(child);
        } else {
            out.extend(server.all_leaves(child.rating_key()).await?.metadata);
        }
    }
    Ok(out)
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

fn media_url(server: &PlexServer, item: &Metadata, media_index: usize, quality: &Quality, live: bool) -> Result<Loaded> {
    let video = matches!(item.kind(), "movie" | "episode" | "clip" | "trailer");
    if live || (quality.kbps.is_some() && video) {
        let session = uuid::Uuid::new_v4().to_string();
        let opts = TranscodeOptions {
            session: session.clone(),
            media_index,
            max_video_bitrate: quality.kbps,
            video_resolution: quality.kbps.map(|_| quality.resolution.to_string()),
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

/// One entry of mpv's playlist, in playlist order.
struct Entry {
    uid: u64,
    item: Metadata,
    media_index: usize,
    transcode_session: Option<String>,
}

/// Per-item playback state.
struct Current {
    uid: u64,
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
    /// In the closing credits or last seconds, with something queued after.
    ending: bool,
    last_ui: Instant,
    /// mpv's playlist entry id for this item, to match its `end-file`.
    mpv_id: Option<i64>,
}

impl Current {
    fn part(&self) -> Option<&Part> {
        part_of(&self.item, self.media_index)
    }
}

struct Player {
    app: AppState,
    server: PlexServer,
    /// Machine id of `server` when it is not the session's own; queue items are tagged with it.
    server_tag: Option<String>,
    settings: PlayerSettings,
    quality: &'static Quality,
    mpv: Arc<Mpv>,
    kind: &'static str,
    live: bool,
    play_queue_id: Option<i64>,
    entries: Vec<Entry>,
    cur: Option<Current>,
    /// Entry order before shuffling, to restore it.
    unshuffled: Option<Vec<u64>>,
    stop_after: bool,
    /// mpv's own `keep-open`, restored when "stop after this" is turned off.
    keep_open: Value,
    played_any: bool,
}

async fn spawn_mpv(settings: &PlayerSettings, headers: Vec<(String, String)>) -> Result<(Arc<Mpv>, mpsc::UnboundedReceiver<Event>)> {
    let previous = ACTIVE.lock().unwrap().take();
    if let Some(old) = previous {
        let _ = old.quit().await;
        old.wait(Duration::from_secs(3)).await;
    }
    let (mpv, events) = Mpv::spawn(SpawnOptions {
        program: settings.mpv_program(),
        extra_args: split_args(&settings.extra_args),
        http_headers: headers,
    })
    .await
    .context("starting mpv (set its path in Settings → Player if it is not on PATH)")?;
    *ACTIVE.lock().unwrap() = Some(mpv.clone());
    Ok((mpv, events))
}

/// Forget the player if it is still the active one; true when it was.
fn release(mpv: &Arc<Mpv>) -> bool {
    let mut active = ACTIVE.lock().unwrap();
    if active.as_ref().is_some_and(|a| Arc::ptr_eq(a, mpv)) {
        *active = None;
        true
    } else {
        false
    }
}

async fn run(app: AppState, req: PlayRequest) -> Result<()> {
    // An item found on another server of the account plays, and reports, through that server.
    let server = crate::servers::server_of(&app, &req.item).await?;
    let server_tag = req.item.server_id().filter(|id| !crate::servers::is_current(&app, Some(id))).map(str::to_string);
    let settings = app.settings().player;
    let queue = build_queue(&server, &req, &settings).await?;

    let info = server.client_info().clone();
    let (mpv, mut events) = spawn_mpv(
        &settings,
        vec![
            ("X-Plex-Token".into(), server.token().to_string()),
            ("X-Plex-Client-Identifier".into(), info.identifier.clone()),
            ("X-Plex-Product".into(), info.product.clone()),
        ],
    )
    .await?;
    let (tx, mut commands) = mpsc::unbounded_channel();
    *COMMANDS.lock().unwrap() = Some(tx);

    for (i, name) in ["playlist-pos", "time-pos", "pause", "duration", "aid", "sid", "idle-active", "eof-reached"]
        .iter()
        .enumerate()
    {
        mpv.observe_property(i as u64 + 1, name).await?;
    }
    if !settings.skip_key.trim().is_empty() {
        let _ = mpv.bind_message(settings.skip_key.trim(), "patina-skip").await;
    }
    let keep_open = mpv.get_property("keep-open").await.unwrap_or(json!("no"));
    if settings.media_controls {
        // Patina publishes Now Playing itself; mpv's own entry would compete (older mpv lacks the property).
        let _ = mpv.set_property("media-controls", "no").await;
    }

    let mut p = Player {
        app: app.clone(),
        server: server.clone(),
        server_tag,
        settings: settings.clone(),
        quality: req.quality.as_deref().and_then(config::quality).unwrap_or_else(|| settings.quality()),
        mpv: mpv.clone(),
        kind: queue.kind,
        live: req.live,
        play_queue_id: queue.id,
        entries: Vec::new(),
        cur: None,
        unshuffled: None,
        stop_after: false,
        keep_open,
        played_any: false,
    };

    // Resolve stream URLs up front; items that cannot be played are dropped so
    // mpv playlist positions line up with `entries`.
    let mut start = 0;
    for (i, item) in queue.items.iter().enumerate() {
        let media_index = if i == queue.start { req.media_index } else { 0 };
        let mut opts = vec![("force-media-title", item.full_title())];
        if i == queue.start
            && req.resume
            && let Some(offset) = item.view_offset.filter(|o| *o > 0)
        {
            opts.push(("start", format!("{:.3}", offset as f64 / 1000.0)));
        }
        if i == queue.start {
            start = p.entries.len();
        }
        if let Err(e) = p.insert(item.clone(), media_index, None, &opts).await {
            tracing::warn!("skipping queue item: {e}");
        }
    }
    if p.entries.is_empty() {
        let _ = mpv.quit().await;
        return Err(anyhow!("nothing in the queue could be played"));
    }
    mpv.set_property("playlist-pos", start as i64).await?;
    p.emit_queue();

    let mut ticker = tokio::time::interval(Duration::from_secs(10));
    ticker.tick().await;

    loop {
        tokio::select! {
            ev = events.recv() => {
                let Some(ev) = ev else { break };
                if !p.on_event(ev).await {
                    break;
                }
            }
            Some(cmd) = commands.recv() => {
                if let Err(e) = p.on_command(cmd).await {
                    app.error(format!("Could not change the queue: {e:#}"));
                }
                p.emit_queue();
            }
            _ = ticker.tick() => p.tick().await,
        }
    }

    if let Some(c) = p.cur.take() {
        let completed = c.reached_end;
        p.finish(c, completed).await;
    }
    for session in p.entries.iter().filter_map(|e| e.transcode_session.as_ref()) {
        let _ = server.stop_transcode(session).await;
    }
    let ours = release(&mpv);
    mpv.wait(Duration::from_secs(2)).await;
    if ours && ACTIVE.lock().unwrap().is_none() {
        *COMMANDS.lock().unwrap() = None;
        app.set_now_playing(None);
        app.set_queue(None);
    }
    app.bump();
    Ok(())
}

impl Player {
    fn reports(&self) -> bool {
        self.kind != "photo" && !self.live
    }

    fn index_of(&self, uid: u64) -> Option<usize> {
        self.entries.iter().position(|e| e.uid == uid)
    }

    fn cur_index(&self) -> Option<usize> {
        self.cur.as_ref().and_then(|c| self.index_of(c.uid))
    }

    fn has_next(&self) -> bool {
        self.cur_index().is_some_and(|i| i + 1 < self.entries.len())
    }

    /// Load an item into mpv at `index` (the end when `None`) and track it.
    async fn insert(&mut self, mut item: Metadata, media_index: usize, index: Option<usize>, opts: &[(&str, String)]) -> Result<u64> {
        item.set_server_id(self.server_tag.as_deref());
        let loaded = media_url(&self.server, &item, media_index, self.quality, self.live)?;
        let mut opts = opts.to_vec();
        if !opts.iter().any(|(k, _)| *k == "force-media-title") {
            opts.push(("force-media-title", item.full_title()));
        }
        match index {
            Some(i) if i < self.entries.len() => self.mpv.loadfile_at(&loaded.url, i, &opts).await?,
            _ => self.mpv.loadfile(&loaded.url, "append", &opts).await?,
        };
        let entry = Entry { uid: uid(), item, media_index, transcode_session: loaded.transcode_session };
        let uid = entry.uid;
        match index {
            Some(i) if i < self.entries.len() => self.entries.insert(i, entry),
            _ => self.entries.push(entry),
        }
        Ok(uid)
    }

    fn emit_queue(&self) {
        self.app.set_queue(Some(QueueView {
            items: self.entries.iter().map(|e| QueueItem { uid: e.uid, item: e.item.clone() }).collect(),
            current: self.cur.as_ref().map(|c| c.uid),
            shuffled: self.unshuffled.is_some(),
            stop_after: self.stop_after,
        }));
    }

    fn publish(&mut self) {
        let pos = self.cur_index().unwrap_or(0);
        let len = self.entries.len();
        let Some(c) = self.cur.as_mut() else { return };
        c.last_ui = Instant::now();
        self.app.set_now_playing(Some(NowPlaying {
            item: c.item.clone(),
            position_ms: c.time_ms,
            duration_ms: c.duration_ms,
            paused: c.paused,
            transcoding: c.transcode_session.is_some(),
            queue_len: len,
            queue_pos: pos,
            uid: c.uid,
            ending: c.ending,
            stop_after: self.stop_after,
            live: self.live,
            local: false,
        }));
    }

    /// Handle one mpv event; false when playback is over.
    async fn on_event(&mut self, ev: Event) -> bool {
        match ev {
            Event::Shutdown => return false,
            Event::PropertyChange { name, data, .. } => match name.as_str() {
                "playlist-pos" => self.on_position(data.as_i64().unwrap_or(-1)).await,
                "time-pos" => {
                    if let Some(t) = data.as_f64() {
                        self.on_time((t * 1000.0) as i64).await;
                    }
                }
                "duration" => {
                    if let (Some(c), Some(d)) = (self.cur.as_mut(), data.as_f64()) {
                        c.duration_ms = (d * 1000.0) as i64;
                    }
                }
                "pause" => {
                    let paused = data.as_bool().unwrap_or(false);
                    if let Some(c) = self.cur.as_mut()
                        && paused != c.paused
                    {
                        c.paused = paused;
                        self.publish();
                        if self.reports()
                            && let Some(c) = self.cur.as_ref()
                        {
                            report(&self.server, c, if paused { TimelineState::Paused } else { TimelineState::Playing }).await;
                        }
                    }
                }
                "eof-reached" => {
                    // With keep-open mpv parks on the last frame instead of ending the file.
                    if data.as_bool() == Some(true)
                        && let Some(c) = self.cur.as_mut()
                    {
                        c.reached_end = true;
                        if self.stop_after {
                            let _ = self.mpv.quit().await;
                        }
                    }
                }
                "aid" | "sid" => {
                    if let Some(c) = self.cur.as_ref()
                        && self.settings.sync_streams
                        && c.transcode_session.is_none()
                        && Instant::now() > c.applying_until
                    {
                        sync_stream_choice(&self.server, &self.mpv, c, &name, &data).await;
                    }
                }
                "idle-active" if data.as_bool() == Some(true) && self.played_any => {
                    let _ = self.mpv.quit().await;
                }
                _ => {}
            },
            Event::FileLoaded => {
                self.played_any = true;
                if let Some(c) = self.cur.as_mut()
                    && self.settings.sync_streams
                    && c.transcode_session.is_none()
                {
                    apply_plex_streams(&self.server, &self.mpv, c).await;
                }
            }
            Event::EndFile { reason, playlist_entry_id, .. } => {
                // Jumping or skipping moves `playlist-pos` before the old file's
                // end-file arrives; that one ended already, not the new item.
                let other = matches!((playlist_entry_id, self.cur.as_ref().and_then(|c| c.mpv_id)), (Some(e), Some(c)) if e != c);
                if !other && let Some(mut c) = self.cur.take() {
                    let completed = reason == "eof";
                    c.reached_end |= completed;
                    self.finish(c, completed).await;
                }
            }
            Event::ClientMessage { args } => {
                if args.first().map(String::as_str) == Some("patina-skip")
                    && let Some(m) = self.cur.as_ref().and_then(|c| c.active_marker.clone())
                {
                    self.skip_marker(&m).await;
                }
            }
            _ => {}
        }
        true
    }

    async fn on_position(&mut self, pos: i64) {
        let uid = (pos >= 0).then(|| self.entries.get(pos as usize).map(|e| e.uid)).flatten();
        if self.cur.as_ref().map(|c| c.uid) == uid {
            return;
        }
        if let Some(c) = self.cur.take() {
            self.finish(c, false).await;
        }
        if self.stop_after {
            self.set_stop_after(false).await;
        }
        let Some(index) = uid.and_then(|u| self.index_of(u)) else { return };
        let e = &self.entries[index];
        let mut c = begin(&self.server, &e.item, e.uid, e.media_index, e.transcode_session.clone()).await;
        c.mpv_id = self.mpv.get_property(&format!("playlist/{pos}/id")).await.ok().and_then(|v| v.as_i64());
        self.cur = Some(c);
        self.publish();
        self.emit_queue();
        if self.reports()
            && let Some(c) = self.cur.as_ref()
        {
            report(&self.server, c, TimelineState::Playing).await;
        }
        if index + 1 == self.entries.len() {
            self.extend_from_plex().await;
        }
    }

    async fn on_time(&mut self, t: i64) {
        let next_title = self.cur_index().and_then(|i| self.entries.get(i + 1)).map(|e| e.item.full_title());
        let video = self.kind == "video" && !self.live;
        let Some(c) = self.cur.as_mut() else { return };
        c.time_ms = t;
        let in_credits = c
            .item
            .markers
            .iter()
            .any(|m| m.kind.as_deref() == Some("credits") && m.is_final && m.start_time_offset.is_some_and(|s| s <= t));
        let near_end = c.duration_ms > 0 && c.duration_ms - t < 30_000;
        let ending = next_title.is_some() && video && (in_credits || near_end);
        let changed = ending != c.ending;
        c.ending = ending;
        if changed
            && !self.stop_after
            && let Some(title) = next_title.filter(|_| ending)
        {
            let _ = self.mpv.show_text(&format!("Up next: {title}"), 4000).await;
        }
        self.handle_markers().await;
        let stale = self.cur.as_ref().is_some_and(|c| c.last_ui.elapsed() >= Duration::from_millis(900));
        if changed || stale {
            self.publish();
        }
    }

    async fn tick(&mut self) {
        let Some(c) = self.cur.as_ref() else { return };
        if self.reports() {
            report(&self.server, c, if c.paused { TimelineState::Paused } else { TimelineState::Playing }).await;
        }
        if let Some(session) = &c.transcode_session {
            let _ = self.server.ping_transcode(session).await;
        }
    }

    async fn finish(&self, c: Current, completed: bool) {
        if !self.reports() {
            return;
        }
        let mut c = c;
        let progress = if c.duration_ms > 0 { c.time_ms as f64 / c.duration_ms as f64 } else { 0.0 };
        let watched = completed || c.reached_end || progress >= self.settings.watched_threshold;
        if watched && c.duration_ms > 0 {
            c.time_ms = c.duration_ms;
        }
        report(&self.server, &c, TimelineState::Stopped).await;
        if watched
            && let Some(rk) = c.item.rating_key.as_deref()
            && let Err(e) = self.server.scrobble(rk).await
        {
            tracing::warn!("scrobble failed: {e}");
        }
        self.app.bump();
    }

    async fn handle_markers(&mut self) {
        let settings = &self.settings;
        let Some(c) = self.cur.as_mut() else { return };
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
            self.skip_marker(&m).await;
        } else {
            let what = if is_intro { "intro" } else { "credits" };
            let key = settings.skip_key.trim().to_string();
            let _ = self.mpv.show_text(&format!("Press {key} to skip {what}"), 5000).await;
        }
    }

    async fn skip_marker(&mut self, m: &Marker) {
        let is_credits = m.kind.as_deref() == Some("credits");
        let has_next = self.has_next();
        let Some(c) = self.cur.as_mut() else { return };
        c.active_marker = None;
        if is_credits && m.is_final && (has_next || self.stop_after) {
            c.reached_end = true;
            if self.stop_after {
                let _ = self.mpv.quit().await;
            } else {
                let _ = self.mpv.show_text("Next episode", 1500).await;
                let _ = self.mpv.playlist_next().await;
            }
            return;
        }
        if let Some(end) = m.end_time_offset {
            let _ = self.mpv.seek_absolute(end as f64 / 1000.0).await;
            let _ = self.mpv.show_text(if is_credits { "Skipped credits" } else { "Skipped intro" }, 1500).await;
        }
    }

    async fn set_stop_after(&mut self, on: bool) {
        self.stop_after = on;
        let value = if on { json!("always") } else { self.keep_open.clone() };
        let _ = self.mpv.set_property("keep-open", value).await;
        if on {
            let _ = self.mpv.show_text("Stopping after this", 1500).await;
        }
        self.publish();
    }

    /// Big and endless queues (radio) arrive in windows; fetch what follows.
    async fn extend_from_plex(&mut self) {
        let Some(id) = self.play_queue_id else { return };
        let Ok(mc) = self.server.play_queue(id, Some(50)).await else { return };
        let known: Vec<i64> = self.entries.iter().filter_map(|e| e.item.play_queue_item_id).collect();
        let Some(last) = known.last().copied() else { return };
        let Some(after) = mc.metadata.iter().position(|m| m.play_queue_item_id == Some(last)) else { return };
        let fresh: Vec<Metadata> = mc.metadata[after + 1..]
            .iter()
            .filter(|m| m.play_queue_item_id.is_some_and(|id| !known.contains(&id)))
            .cloned()
            .collect();
        for item in fresh {
            if let Err(e) = self.insert(item, 0, None, &[]).await {
                tracing::debug!("could not extend queue: {e}");
            }
        }
        self.emit_queue();
    }

    async fn on_command(&mut self, cmd: QueueCommand) -> Result<()> {
        match cmd {
            QueueCommand::Add { item, next } => self.add(*item, next).await?,
            QueueCommand::Remove { uid } => {
                let Some(i) = self.index_of(uid) else { return Ok(()) };
                self.mpv.playlist_remove(i).await?;
                let e = self.entries.remove(i);
                if let Some(s) = &e.transcode_session {
                    let _ = self.server.stop_transcode(s).await;
                }
                if let (Some(id), Some(item_id)) = (self.play_queue_id, e.item.play_queue_item_id) {
                    let _ = self.server.remove_from_play_queue(id, item_id).await;
                }
                if let Some(order) = self.unshuffled.as_mut() {
                    order.retain(|u| *u != uid);
                }
            }
            QueueCommand::Move { uid, before } => {
                let Some(from) = self.index_of(uid) else { return Ok(()) };
                let to = before.and_then(|b| self.index_of(b)).unwrap_or(self.entries.len());
                self.move_entry(from, to).await?;
                let now = self.index_of(uid).unwrap_or(0);
                let after = now.checked_sub(1).and_then(|i| self.entries[i].item.play_queue_item_id);
                if let (Some(id), Some(item_id)) = (self.play_queue_id, self.entries[now].item.play_queue_item_id) {
                    let _ = self.server.move_play_queue_item(id, item_id, after).await;
                }
            }
            QueueCommand::Jump { uid } => {
                if let Some(i) = self.index_of(uid) {
                    self.mpv.set_property("playlist-pos", i as i64).await?;
                }
            }
            QueueCommand::Shuffle { on } => self.shuffle(on).await?,
            QueueCommand::StopAfter { on } => self.set_stop_after(on).await,
        }
        Ok(())
    }

    async fn add(&mut self, item: Metadata, next: bool) -> Result<()> {
        if item.server_id() != self.server_tag.as_deref() {
            return Err(anyhow!("A queue plays from one server; “{}” is on another.", item.title()));
        }
        let items = leaves(&self.server, &item).await?;
        if items.is_empty() {
            return Err(anyhow!("nothing playable in “{}”", item.title()));
        }
        let first = if next { self.cur_index().map(|i| i + 1) } else { None };
        let mut added = Vec::new();
        for (n, it) in items.into_iter().enumerate() {
            match self.insert(it, 0, first.map(|i| i + n), &[]).await {
                Ok(uid) => added.push(uid),
                Err(e) => tracing::warn!("could not queue item: {e}"),
            }
        }
        // Keep the Plex queue in step so other apps (and later edits) agree.
        if let Some(id) = self.play_queue_id {
            let uri = match item.kind() {
                "playlist" => self.server.library_uri(&format!("/playlists/{}/items", item.rating_key())),
                _ => self.server.item_uri(&[item.rating_key()]),
            };
            if let Ok(mc) = self.server.add_to_play_queue(id, &uri, next).await {
                let mut known: Vec<i64> = self.entries.iter().filter_map(|e| e.item.play_queue_item_id).collect();
                for uid in &added {
                    let Some(i) = self.index_of(*uid) else { continue };
                    let rk = self.entries[i].item.rating_key.clone();
                    let found = mc.metadata.iter().find(|m| {
                        m.rating_key == rk && m.play_queue_item_id.is_some_and(|pid| !known.contains(&pid))
                    });
                    if let Some(pid) = found.and_then(|m| m.play_queue_item_id) {
                        self.entries[i].item.play_queue_item_id = Some(pid);
                        known.push(pid);
                    }
                }
            }
        }
        if let Some(order) = self.unshuffled.as_mut() {
            order.extend(&added);
        }
        let what = if added.len() == 1 { "1 item".to_string() } else { format!("{} items", added.len()) };
        self.app.toast(if next { format!("Playing {what} next") } else { format!("Added {what} to the queue") });
        Ok(())
    }

    /// Move entry `from` before entry `to` (or to the end), in mpv and locally.
    async fn move_entry(&mut self, from: usize, to: usize) -> Result<()> {
        if from == to || from + 1 == to {
            return Ok(());
        }
        self.mpv.playlist_move(from, to).await?;
        let e = self.entries.remove(from);
        let dest = if from < to { to - 1 } else { to };
        self.entries.insert(dest.min(self.entries.len()), e);
        Ok(())
    }

    /// Put the entries after the current one into `order`.
    async fn reorder_upcoming(&mut self, order: &[u64]) -> Result<()> {
        let base = self.cur_index().map(|i| i + 1).unwrap_or(0);
        for (k, uid) in order.iter().enumerate() {
            let Some(i) = self.index_of(*uid) else { continue };
            if i >= base && i != base + k {
                self.move_entry(i, base + k).await?;
            }
        }
        Ok(())
    }

    async fn shuffle(&mut self, on: bool) -> Result<()> {
        let base = self.cur_index().map(|i| i + 1).unwrap_or(0);
        if on {
            if self.unshuffled.is_none() {
                self.unshuffled = Some(self.entries.iter().map(|e| e.uid).collect());
            }
            let mut upcoming: Vec<u64> = self.entries[base.min(self.entries.len())..].iter().map(|e| e.uid).collect();
            fastrand::shuffle(&mut upcoming);
            self.reorder_upcoming(&upcoming).await?;
            if let Some(id) = self.play_queue_id {
                let _ = self.server.shuffle_play_queue(id).await;
            }
        } else if let Some(order) = self.unshuffled.take() {
            let upcoming: Vec<u64> = self.entries[base.min(self.entries.len())..].iter().map(|e| e.uid).collect();
            let restored: Vec<u64> = order.into_iter().filter(|u| upcoming.contains(u)).collect();
            self.reorder_upcoming(&restored).await?;
            if let Some(id) = self.play_queue_id {
                let _ = self.server.unshuffle_play_queue(id).await;
            }
        }
        Ok(())
    }
}

async fn begin(server: &PlexServer, item: &Metadata, uid: u64, media_index: usize, transcode_session: Option<String>) -> Current {
    // Queue entries lack markers and sometimes streams; fetch the full item.
    let full = match item.rating_key.as_deref() {
        Some(rk) if item.kind() != "photo" => server.metadata(rk, MetadataOptions::playback()).await.ok().flatten(),
        _ => None,
    };
    let mut full = full.unwrap_or_else(|| item.clone());
    full.play_queue_item_id = item.play_queue_item_id;
    Current {
        uid,
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
        ending: false,
        last_ui: Instant::now() - Duration::from_secs(5),
        mpv_id: None,
    }
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
        && let Some(id) = audio.index.and_then(|i| find("audio", i))
    {
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

// Offline playback -----------------------------------------------------------

/// Play downloaded files with local checkpoints; the outbox reconciles them when connected.
pub fn play_files(app: AppState, files: Vec<crate::downloads::Download>) {
    app.clone().spawn(async move {
        if let Err(e) = run_files(app.clone(), files).await {
            app.error(format!("Playback failed: {e:#}"));
            app.set_now_playing(None);
        }
    });
}

async fn run_files(app: AppState, files: Vec<crate::downloads::Download>) -> Result<()> {
    let settings = app.settings().player;
    let (mpv, mut events) = spawn_mpv(&settings, Vec::new()).await?;
    *COMMANDS.lock().unwrap() = None;
    for (i, name) in ["playlist-pos", "time-pos", "pause", "duration", "idle-active"].iter().enumerate() {
        mpv.observe_property(i as u64 + 1, name).await?;
    }
    for download in &files {
        let path = download.file.as_deref().ok_or_else(|| anyhow!("Downloaded file is missing"))?;
        mpv.loadfile(path, "append", &[("force-media-title", download.item.full_title()),
            ("start", format!("{:.3}", crate::offline::position(download) as f64 / 1000.0))]).await?;
    }
    mpv.set_property("playlist-pos", 0).await?;

    let mut np: Option<NowPlaying> = None;
    let mut last_ui = Instant::now();
    let mut played_any = false;
    let mut checkpoint = Instant::now();
    let mut current_entry_id = None;
    let save = |np: &Option<NowPlaying>, completed: bool| {
        if let Some(n) = np && let Some(d) = files.get(n.queue_pos)
            && let Err(e) = crate::offline::record(d, n.position_ms, n.duration_ms, completed, settings.watched_threshold) {
            app.error(format!("Could not save offline progress: {e:#}"));
        }
    };
    while let Some(ev) = events.recv().await {
        match ev {
            Event::Shutdown => { save(&np, false); break; },
            Event::FileLoaded => played_any = true,
            Event::EndFile { reason, playlist_entry_id, .. } => {
                // A skip can publish the new playlist position before the old entry ends.
                let stale = matches!((playlist_entry_id, current_entry_id), (Some(ended), Some(current)) if ended != current);
                if !stale {
                    save(&np, reason == "eof");
                    np = None;
                    current_entry_id = None;
                }
            },
            Event::PropertyChange { name, data, .. } => {
                match name.as_str() {
                    "playlist-pos" => {
                        save(&np, false);
                        let pos = data.as_i64().unwrap_or(-1);
                        current_entry_id = if pos >= 0 {
                            mpv.get_property(&format!("playlist/{pos}/id")).await.ok().and_then(|v| v.as_i64())
                        } else { None };
                        np = (pos >= 0).then(|| files.get(pos as usize)).flatten().map(|d| NowPlaying {
                            item: d.item.clone(),
                            position_ms: crate::offline::position(d),
                            duration_ms: d.item.duration.unwrap_or(0),
                            paused: false,
                            transcoding: false,
                            queue_len: files.len(),
                            queue_pos: pos as usize,
                            uid: 0,
                            ending: false,
                            stop_after: false,
                            live: false,
                            local: true,
                        });
                    }
                    "time-pos" => {
                        if let (Some(n), Some(t)) = (np.as_mut(), data.as_f64()) {
                            n.position_ms = (t * 1000.0) as i64;
                        }
                        if last_ui.elapsed() < Duration::from_millis(900) {
                            continue;
                        }
                    }
                    "duration" => {
                        if let (Some(n), Some(d)) = (np.as_mut(), data.as_f64()) {
                            n.duration_ms = (d * 1000.0) as i64;
                        }
                    }
                    "pause" => {
                        if let Some(n) = np.as_mut() {
                            n.paused = data.as_bool().unwrap_or(false);
                        }
                    }
                    "idle-active" if data.as_bool() == Some(true) && played_any => {
                        let _ = mpv.quit().await;
                    }
                    _ => continue,
                }
                if checkpoint.elapsed() >= Duration::from_secs(5) || name == "pause" {
                    save(&np, false);
                    checkpoint = Instant::now();
                }
                last_ui = Instant::now();
                app.set_now_playing(np.clone());
            }
            _ => {}
        }
    }
    save(&np, false);
    let ours = release(&mpv);
    mpv.wait(Duration::from_secs(2)).await;
    if ours && ACTIVE.lock().unwrap().is_none() {
        app.set_now_playing(None);
    }
    Ok(())
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
