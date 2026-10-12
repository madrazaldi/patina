//! Player controls beyond transport (volume, speed, chapters, repeat, a sleep
//! timer) and the live diagnostics shown beside them (buffering, hardware
//! decoding, dropped frames, connection type).
//!
//! The controls act on mpv directly. What the front ends show arrives as one
//! [`PlaybackState`] event, emitted when something changes (diagnostics at
//! most once a second). The state is also kept here, for front ends that open
//! while something is already playing.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use patina_mpv::Mpv;
use patina_plex::Chapter;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::state::{AppState, CoreEvent, Session};

/// What happens when the queue or item ends.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Repeat {
    #[default]
    Off,
    One,
    All,
}

impl Repeat {
    /// Off, then the whole queue, then the item, then off again.
    pub fn next(self) -> Self {
        match self {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Repeat::Off => "off",
            Repeat::One => "repeat one",
            Repeat::All => "repeat all",
        }
    }
}

/// Sleep timer presets cycled through by [`Tune::CycleSleep`], in minutes.
pub const SLEEP_PRESETS: [u32; 5] = [15, 30, 45, 60, 90];

/// A change to the running player.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Tune {
    /// Volume in percent (0 to 130).
    Volume { value: f64 },
    VolumeBy { delta: f64 },
    /// Mute, unmute, or toggle with `None`.
    Mute { on: Option<bool> },
    /// Playback speed (0.25 to 4).
    Speed { value: f64 },
    SpeedBy { delta: f64 },
    Repeat { mode: Repeat },
    CycleRepeat,
    /// Jump to a chapter by index.
    Chapter { index: usize },
    ChapterBy { delta: i64 },
    SleepOff,
    SleepMinutes { minutes: u32 },
    SleepEndOfItem,
    /// Off, then each of [`SLEEP_PRESETS`], then the end of the item, then off.
    CycleSleep,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterView {
    pub title: String,
    pub time_ms: i64,
}

/// How the server is reached.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfo {
    /// `local`, `remote`, `relay` or `offline` (a downloaded file).
    pub kind: String,
    pub address: Option<String>,
}

impl ConnectionInfo {
    pub fn offline() -> Self {
        Self { kind: "offline".into(), address: None }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    /// Playback is stalled waiting for data.
    pub buffering: bool,
    /// How full the buffer is while stalled, in percent.
    pub buffer_percent: Option<i64>,
    /// Seconds of media ahead of the playhead that are already buffered.
    pub buffered_secs: Option<f64>,
    /// The hardware decoder in use; `no` is software decoding, `None` is unknown (or audio only).
    pub hwdec: Option<String>,
    /// Frames the video output dropped.
    pub dropped_frames: i64,
    /// Frames the decoder dropped.
    pub decoder_dropped_frames: i64,
    pub connection: Option<ConnectionInfo>,
    /// Where playback restarted from a local checkpoint after the connection failed, if it did.
    pub recovered_from_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SleepState {
    /// `minutes` or `endOfItem`.
    pub mode: String,
    /// When the timer ends, in milliseconds since the epoch (`minutes` only).
    pub ends_at_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackState {
    /// Percent; mpv allows up to 130.
    pub volume: f64,
    pub muted: bool,
    pub speed: f64,
    pub repeat: Repeat,
    pub sleep: Option<SleepState>,
    pub chapters: Vec<ChapterView>,
    /// Index into `chapters` of the playing chapter.
    pub chapter: Option<usize>,
    pub diagnostics: Diagnostics,
}

// Process-wide settings ---------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Sleep {
    Minutes { id: u64, ends_at_ms: i64 },
    EndOfItem,
}

struct Tuning {
    volume: Option<f64>,
    muted: bool,
    repeat: Repeat,
    sleep: Option<Sleep>,
    /// Mirrors the player, which owns "stop after this".
    stop_after: bool,
}

static TUNING: Mutex<Tuning> = Mutex::new(Tuning { volume: None, muted: false, repeat: Repeat::Off, sleep: None, stop_after: false });
static STATE: Mutex<Option<PlaybackState>> = Mutex::new(None);
static NEXT_TIMER: AtomicU64 = AtomicU64::new(1);

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or_default()
}

/// The last published state; `None` when nothing is playing.
pub fn playback_state() -> Option<PlaybackState> {
    STATE.lock().unwrap().clone()
}

pub fn repeat() -> Repeat {
    TUNING.lock().unwrap().repeat
}

/// The player toggled "stop after this"; repeat-one must not keep the item from ending.
pub fn set_stop_after(on: bool) {
    let mut t = TUNING.lock().unwrap();
    t.stop_after = on;
    if !on && t.sleep == Some(Sleep::EndOfItem) {
        t.sleep = None;
    }
}

/// Whether the sleep timer is "end of this item".
pub fn sleep_at_item_end() -> bool {
    TUNING.lock().unwrap().sleep == Some(Sleep::EndOfItem)
}

/// Make a freshly started mpv honour the volume, mute and repeat choices made earlier.
pub async fn apply_saved(mpv: &Mpv) {
    let (volume, muted, repeat, stop_after) = {
        let t = TUNING.lock().unwrap();
        (t.volume, t.muted, t.repeat, t.stop_after)
    };
    if let Some(v) = volume {
        let _ = mpv.set_property("volume", v).await;
    }
    if muted {
        let _ = mpv.set_property("mute", true).await;
    }
    apply_repeat(mpv, repeat, stop_after).await;
}

/// mpv's loop properties for a repeat mode. "Stop after this" wins over repeating the item.
pub async fn apply_repeat(mpv: &Mpv, repeat: Repeat, stop_after: bool) {
    let file = if repeat == Repeat::One && !stop_after { json!("inf") } else { json!("no") };
    let list = if repeat == Repeat::All { json!("inf") } else { json!("no") };
    let _ = mpv.set_property("loop-file", file).await;
    let _ = mpv.set_property("loop-playlist", list).await;
}

/// Playback ended: drop what only applies to it.
pub fn finished(app: &AppState) {
    TUNING.lock().unwrap().sleep = None;
    TUNING.lock().unwrap().stop_after = false;
    *STATE.lock().unwrap() = None;
    app.emit(CoreEvent::PlaybackState(None));
}

/// Apply a [`Tune`] to the running player.
pub async fn apply(app: &AppState, mpv: &Mpv, cmd: Tune) -> Result<()> {
    match cmd {
        Tune::Volume { value } => set_volume(mpv, value).await?,
        Tune::VolumeBy { delta } => {
            let now = mpv.get_property("volume").await.ok().and_then(|v| v.as_f64()).or(TUNING.lock().unwrap().volume).unwrap_or(100.0);
            set_volume(mpv, now + delta).await?;
        }
        Tune::Mute { on } => {
            let now = mpv.get_property("mute").await.ok().and_then(|v| v.as_bool()).unwrap_or(false);
            let muted = on.unwrap_or(!now);
            mpv.set_property("mute", muted).await?;
            TUNING.lock().unwrap().muted = muted;
        }
        Tune::Speed { value } => set_speed(mpv, value).await?,
        Tune::SpeedBy { delta } => {
            let now = mpv.get_property("speed").await.ok().and_then(|v| v.as_f64()).unwrap_or(1.0);
            set_speed(mpv, now + delta).await?;
        }
        Tune::Repeat { mode } => set_repeat(mpv, mode).await,
        Tune::CycleRepeat => set_repeat(mpv, repeat().next()).await,
        Tune::Chapter { index } => jump_chapter(mpv, index as i64).await?,
        Tune::ChapterBy { delta } => {
            let state = playback_state().ok_or_else(|| anyhow!("Nothing is playing"))?;
            let current = state.chapter.map(|c| c as i64).unwrap_or(if delta > 0 { -1 } else { state.chapters.len() as i64 });
            jump_chapter(mpv, current + delta).await?;
        }
        Tune::SleepOff => set_sleep(app, None),
        Tune::SleepMinutes { minutes } => set_sleep(app, Some(minutes_timer(minutes))),
        Tune::SleepEndOfItem => set_sleep(app, Some(Sleep::EndOfItem)),
        Tune::CycleSleep => {
            let next = match TUNING.lock().unwrap().sleep {
                None => Some(minutes_timer(SLEEP_PRESETS[0])),
                Some(Sleep::Minutes { ends_at_ms, .. }) => {
                    // The preset at or above what is left, then the one after it.
                    let left = ((ends_at_ms - now_ms()).max(0) as f64 / 60_000.0).ceil() as u32;
                    match SLEEP_PRESETS.iter().position(|p| *p >= left) {
                        Some(i) if i + 1 < SLEEP_PRESETS.len() => Some(minutes_timer(SLEEP_PRESETS[i + 1])),
                        _ => Some(Sleep::EndOfItem),
                    }
                }
                Some(Sleep::EndOfItem) => None,
            };
            set_sleep(app, next);
        }
    }
    refresh(app);
    Ok(())
}

fn minutes_timer(minutes: u32) -> Sleep {
    Sleep::Minutes { id: NEXT_TIMER.fetch_add(1, Ordering::Relaxed), ends_at_ms: now_ms() + minutes as i64 * 60_000 }
}

async fn set_volume(mpv: &Mpv, value: f64) -> Result<()> {
    let v = value.clamp(0.0, 130.0).round();
    mpv.set_property("volume", v).await?;
    TUNING.lock().unwrap().volume = Some(v);
    Ok(())
}

async fn set_speed(mpv: &Mpv, value: f64) -> Result<()> {
    Ok(mpv.set_property("speed", (value.clamp(0.25, 4.0) * 100.0).round() / 100.0).await?)
}

async fn set_repeat(mpv: &Mpv, mode: Repeat) {
    let stop_after = {
        let mut t = TUNING.lock().unwrap();
        t.repeat = mode;
        t.stop_after
    };
    apply_repeat(mpv, mode, stop_after).await;
}

async fn jump_chapter(mpv: &Mpv, index: i64) -> Result<()> {
    let state = playback_state().ok_or_else(|| anyhow!("Nothing is playing"))?;
    let chapter = usize::try_from(index).ok().and_then(|i| state.chapters.get(i)).ok_or_else(|| anyhow!("No such chapter"))?;
    Ok(mpv.seek_absolute(chapter.time_ms as f64 / 1000.0).await?)
}

fn set_sleep(app: &AppState, sleep: Option<Sleep>) {
    TUNING.lock().unwrap().sleep = sleep;
    if let Some(Sleep::Minutes { id, ends_at_ms }) = sleep {
        let app = app.clone();
        let wait = Duration::from_millis((ends_at_ms - now_ms()).max(0) as u64);
        crate::runtime().spawn(async move {
            tokio::time::sleep(wait).await;
            let due = matches!(TUNING.lock().unwrap().sleep, Some(Sleep::Minutes { id: current, .. }) if current == id);
            if due {
                TUNING.lock().unwrap().sleep = None;
                app.toast("Sleep timer: playback stopped");
                crate::player::control(crate::player::Control::Stop);
                refresh(&app);
            }
        });
    }
}

/// Publish the shared state again after a control changed the process-wide parts of it.
fn refresh(app: &AppState) {
    let state = {
        let mut guard = STATE.lock().unwrap();
        let Some(state) = guard.as_mut() else { return };
        let (repeat, sleep) = global_parts();
        state.repeat = repeat;
        state.sleep = sleep;
        state.clone()
    };
    app.emit(CoreEvent::PlaybackState(Some(Box::new(state))));
}

fn global_parts() -> (Repeat, Option<SleepState>) {
    let t = TUNING.lock().unwrap();
    let sleep = t.sleep.map(|s| match s {
        Sleep::Minutes { ends_at_ms, .. } => SleepState { mode: "minutes".into(), ends_at_ms: Some(ends_at_ms) },
        Sleep::EndOfItem => SleepState { mode: "endOfItem".into(), ends_at_ms: None },
    });
    (t.repeat, sleep)
}

// Following mpv -----------------------------------------------------------------

/// mpv properties [`Tracker`] follows; observe each with a distinct id.
pub const OBSERVED: &[&str] = &[
    "volume",
    "mute",
    "speed",
    "chapter-list",
    "chapter",
    "paused-for-cache",
    "cache-buffering-state",
    "demuxer-cache-duration",
    "hwdec-current",
    "frame-drop-count",
    "decoder-frame-drop-count",
];

/// First id used to observe [`OBSERVED`]; the player's own properties use the ids below it.
pub const OBSERVE_BASE: u64 = 100;

/// Keeps the [`PlaybackState`] of one player up to date from its property changes.
pub struct Tracker {
    volume: f64,
    muted: bool,
    speed: f64,
    mpv_chapters: Vec<ChapterView>,
    plex_chapters: Vec<ChapterView>,
    mpv_chapter: Option<i64>,
    position_ms: i64,
    diag: Diagnostics,
    emitted: Option<PlaybackState>,
    last_emit: Instant,
}

impl Tracker {
    pub fn new(connection: ConnectionInfo) -> Self {
        Self {
            volume: 100.0,
            muted: false,
            speed: 1.0,
            mpv_chapters: Vec::new(),
            plex_chapters: Vec::new(),
            mpv_chapter: None,
            position_ms: 0,
            diag: Diagnostics { connection: Some(connection), ..Default::default() },
            emitted: None,
            last_emit: Instant::now() - Duration::from_secs(5),
        }
    }

    pub fn observes(name: &str) -> bool {
        OBSERVED.contains(&name)
    }

    /// Start observing; errors for properties an older mpv lacks are ignored.
    pub async fn observe(mpv: &Mpv) {
        for (i, name) in OBSERVED.iter().enumerate() {
            let _ = mpv.observe_property(OBSERVE_BASE + i as u64, name).await;
        }
    }

    /// Take in a property change; true when it should be published right away.
    pub fn on_property(&mut self, name: &str, data: &Value) -> bool {
        match name {
            "volume" => self.volume = data.as_f64().unwrap_or(self.volume),
            "mute" => self.muted = data.as_bool().unwrap_or(false),
            "speed" => self.speed = data.as_f64().unwrap_or(1.0),
            "chapter-list" => {
                self.mpv_chapters = data
                    .as_array()
                    .map(|list| {
                        list.iter()
                            .enumerate()
                            .map(|(i, c)| ChapterView {
                                title: c["title"].as_str().filter(|t| !t.trim().is_empty()).map(str::to_string).unwrap_or_else(|| format!("Chapter {}", i + 1)),
                                time_ms: (c["time"].as_f64().unwrap_or(0.0) * 1000.0) as i64,
                            })
                            .collect()
                    })
                    .unwrap_or_default();
            }
            "chapter" => self.mpv_chapter = data.as_i64(),
            "paused-for-cache" => self.diag.buffering = data.as_bool().unwrap_or(false),
            "cache-buffering-state" => self.diag.buffer_percent = data.as_i64(),
            "demuxer-cache-duration" => self.diag.buffered_secs = data.as_f64().map(|s| (s * 10.0).round() / 10.0),
            "hwdec-current" => self.diag.hwdec = data.as_str().filter(|s| !s.is_empty()).map(str::to_string),
            "frame-drop-count" => self.diag.dropped_frames = data.as_i64().unwrap_or(0),
            "decoder-frame-drop-count" => self.diag.decoder_dropped_frames = data.as_i64().unwrap_or(0),
            _ => return false,
        }
        !matches!(name, "demuxer-cache-duration" | "cache-buffering-state" | "frame-drop-count" | "decoder-frame-drop-count")
    }

    pub fn on_time(&mut self, ms: i64) {
        self.position_ms = ms;
    }

    /// A new file: per-file figures start over.
    pub fn new_item(&mut self, plex: &[Chapter]) {
        self.mpv_chapters.clear();
        self.mpv_chapter = None;
        self.diag.buffering = false;
        self.diag.buffer_percent = None;
        self.diag.buffered_secs = None;
        self.diag.dropped_frames = 0;
        self.diag.decoder_dropped_frames = 0;
        self.diag.recovered_from_ms = None;
        self.set_plex_chapters(plex);
    }

    /// Chapters Plex knows, for streams (transcodes) that carry none of their own.
    pub fn set_plex_chapters(&mut self, plex: &[Chapter]) {
        self.plex_chapters = plex
            .iter()
            .enumerate()
            .map(|(i, c)| ChapterView {
                title: c.tag.clone().filter(|t| !t.trim().is_empty()).unwrap_or_else(|| format!("Chapter {}", i + 1)),
                time_ms: c.start_time_offset.unwrap_or(0),
            })
            .collect();
    }

    pub fn recovered_from(&mut self, ms: Option<i64>) {
        self.diag.recovered_from_ms = ms;
    }

    fn view(&self) -> PlaybackState {
        let chapters = if self.mpv_chapters.is_empty() { self.plex_chapters.clone() } else { self.mpv_chapters.clone() };
        let chapter = if !self.mpv_chapters.is_empty() {
            self.mpv_chapter.and_then(|c| usize::try_from(c).ok())
        } else {
            chapters.iter().rposition(|c| c.time_ms <= self.position_ms)
        };
        let (repeat, sleep) = global_parts();
        PlaybackState {
            volume: self.volume,
            muted: self.muted,
            speed: self.speed,
            repeat,
            sleep,
            chapters,
            chapter,
            diagnostics: self.diag.clone(),
        }
    }

    /// Emit the state if it changed. Quiet changes (buffer levels, frame counts) wait for the
    /// next second; `urgent` ones go out now.
    pub fn publish(&mut self, app: &AppState, urgent: bool) {
        if !urgent && self.last_emit.elapsed() < Duration::from_secs(1) {
            return;
        }
        let state = self.view();
        if self.emitted.as_ref() == Some(&state) {
            return;
        }
        self.last_emit = Instant::now();
        self.emitted = Some(state.clone());
        *STATE.lock().unwrap() = Some(state.clone());
        app.emit(CoreEvent::PlaybackState(Some(Box::new(state))));
    }
}

// Connection ----------------------------------------------------------------------

/// How the session's server is reached: from Plex's connection list when it names the
/// address in use, else from the address itself.
pub fn connection_info(session: &Session) -> ConnectionInfo {
    let base = session.server.base_url();
    let host = base.host_str().unwrap_or("").to_string();
    let address = Some(match base.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.clone(),
    });
    let target = base.as_str().trim_end_matches('/');
    let connections = session.servers.iter().filter(|r| r.id() == session.server_id).flat_map(|r| r.connections.iter());
    let known = connections.clone().find(|c| c.uri.as_deref().is_some_and(|u| u.trim_end_matches('/') == target));
    let known = known.or_else(|| connections.into_iter().find(|c| c.address.as_deref() == Some(host.as_str()) && c.port == base.port().map(i64::from)));
    let kind = match known {
        Some(c) if c.relay => "relay",
        Some(c) if c.local => "local",
        Some(_) => "remote",
        None if is_local_host(&host) => "local",
        None => "remote",
    };
    ConnectionInfo { kind: kind.into(), address }
}

/// Whether an address is on this machine or its network.
pub fn is_local_host(host: &str) -> bool {
    use std::net::IpAddr;
    let host = host.trim_matches(['[', ']']).to_ascii_lowercase();
    let host = match host.strip_suffix(".plex.direct") {
        // `192-168-1-5.<id>.plex.direct` names the address in its first label.
        Some(name) => name.split('.').next().unwrap_or("").replace('-', "."),
        None => host,
    };
    if host == "localhost" || host.ends_with(".local") {
        return true;
    }
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => ip.is_private() || ip.is_loopback() || ip.is_link_local(),
        Ok(IpAddr::V6(ip)) => ip.is_loopback() || (ip.segments()[0] & 0xfe00) == 0xfc00 || (ip.segments()[0] & 0xffc0) == 0xfe80,
        // A bare machine name only resolves on the local network.
        Err(_) => !host.contains('.'),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_local_addresses() {
        for host in ["localhost", "127.0.0.1", "192.168.1.20", "10.0.0.2", "172.16.5.5", "nas.local", "nas", "[::1]", "192-168-1-20.abc123.plex.direct", "fd12::1"] {
            assert!(is_local_host(host), "{host}");
        }
        for host in ["8.8.8.8", "plex.example.com", "203-0-113-9.abc123.plex.direct", "2001:db8::1"] {
            assert!(!is_local_host(host), "{host}");
        }
    }

    #[test]
    fn repeat_cycles_through_every_mode() {
        assert_eq!(Repeat::Off.next(), Repeat::All);
        assert_eq!(Repeat::All.next(), Repeat::One);
        assert_eq!(Repeat::One.next(), Repeat::Off);
    }

    #[test]
    fn follows_mpv_properties_and_flags_urgent_ones() {
        let mut t = Tracker::new(ConnectionInfo { kind: "local".into(), address: None });
        assert!(t.on_property("volume", &json!(80.0)));
        assert!(t.on_property("mute", &json!(true)));
        assert!(!t.on_property("demuxer-cache-duration", &json!(12.34)));
        assert!(!t.on_property("frame-drop-count", &json!(3)));
        assert!(t.on_property("paused-for-cache", &json!(true)));
        assert!(t.on_property("hwdec-current", &json!("videotoolbox")));
        assert!(!t.on_property("time-pos", &json!(1.0)));
        let v = t.view();
        assert_eq!((v.volume, v.muted), (80.0, true));
        assert_eq!(v.diagnostics.buffered_secs, Some(12.3));
        assert_eq!(v.diagnostics.dropped_frames, 3);
        assert_eq!(v.diagnostics.hwdec.as_deref(), Some("videotoolbox"));
        assert!(v.diagnostics.buffering);
        t.new_item(&[]);
        assert_eq!(t.view().diagnostics.dropped_frames, 0);
        assert_eq!(t.view().diagnostics.connection.map(|c| c.kind), Some("local".into()));
    }

    #[test]
    fn chapters_come_from_mpv_or_else_plex() {
        let mut t = Tracker::new(ConnectionInfo::offline());
        let plex = [
            Chapter { tag: Some("Opening".into()), start_time_offset: Some(0), ..Default::default() },
            Chapter { tag: None, start_time_offset: Some(60_000), ..Default::default() },
        ];
        t.new_item(&plex);
        t.on_time(70_000);
        let v = t.view();
        assert_eq!(v.chapters.iter().map(|c| c.title.as_str()).collect::<Vec<_>>(), ["Opening", "Chapter 2"]);
        assert_eq!(v.chapter, Some(1));
        t.on_property("chapter-list", &json!([{"title": "A", "time": 0.0}, {"title": "", "time": 30.5}, {"title": "C", "time": 90.0}]));
        t.on_property("chapter", &json!(2));
        let v = t.view();
        assert_eq!(v.chapters.len(), 3);
        assert_eq!(v.chapters[1], ChapterView { title: "Chapter 2".into(), time_ms: 30_500 });
        assert_eq!(v.chapter, Some(2));
    }
}
