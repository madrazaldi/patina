//! What's playing, shown outside the app: the OS media controls (macOS Now
//! Playing, MPRIS on Linux, SMTC on Windows) and Discord Rich Presence.
//! Both follow [`AppState::set_now_playing`]; media keys and the OS widgets
//! drive the player through [`player::control`].

use std::cell::RefCell;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use discord_rich_presence::activity::{Activity, ActivityType, StatusDisplayType, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use patina_plex::Metadata;
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig, SeekDirection};
use tauri::Manager;

use crate::player::{self, Control};
use crate::state::{AppState, NowPlaying};

thread_local! {
    // Created and used on the main thread only.
    static CONTROLS: RefCell<Option<MediaControls>> = const { RefCell::new(None) };
}

/// What was last shown, to skip redundant updates.
#[derive(Clone, PartialEq)]
struct Shown {
    key: String,
    paused: bool,
    /// Where playback should be now if it kept going (unix ms minus position).
    anchor_ms: i64,
}

static LAST: Mutex<Option<Shown>> = Mutex::new(None);
static DISCORD: Mutex<Option<Sender<Option<Presence>>>> = Mutex::new(None);

/// Set up the media controls. Call on the main thread during setup.
pub fn init(app: &AppState) {
    #[cfg(target_os = "windows")]
    let hwnd = app
        .handle()
        .get_webview_window("main")
        .and_then(|w| w.hwnd().ok())
        .map(|h| h.0 as *mut std::ffi::c_void);
    #[cfg(not(target_os = "windows"))]
    let hwnd = None;

    let config = PlatformConfig { display_name: "Patina", dbus_name: "patina", hwnd };
    let mut controls = match MediaControls::new(config) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("media controls unavailable: {e:?}");
            return;
        }
    };
    let handle = app.handle().clone();
    let attached = controls.attach(move |event| match event {
        MediaControlEvent::Play => player::control(Control::Play),
        MediaControlEvent::Pause => player::control(Control::Pause),
        MediaControlEvent::Toggle => player::control(Control::TogglePause),
        MediaControlEvent::Next => player::control(Control::Next),
        MediaControlEvent::Previous => player::control(Control::Previous),
        MediaControlEvent::Stop | MediaControlEvent::Quit => player::control(Control::Stop),
        MediaControlEvent::Seek(dir) => player::control(Control::SeekRelative(signed(dir, 10.0))),
        MediaControlEvent::SeekBy(dir, by) => player::control(Control::SeekRelative(signed(dir, by.as_secs_f64()))),
        MediaControlEvent::SetPosition(MediaPosition(at)) => player::control(Control::SeekTo(at.as_secs_f64())),
        MediaControlEvent::Raise => {
            if let Some(w) = handle.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }
        _ => {}
    });
    if let Err(e) = attached {
        tracing::warn!("media controls unavailable: {e:?}");
        return;
    }
    CONTROLS.with(|c| *c.borrow_mut() = Some(controls));
}

fn signed(dir: SeekDirection, secs: f64) -> f64 {
    if dir == SeekDirection::Forward { secs } else { -secs }
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Follow the now-playing state. Cheap when nothing changed.
pub fn update(app: &AppState, np: Option<&NowPlaying>) {
    let shown = np.map(|np| Shown {
        key: format!("{}:{}", np.item.rating_key(), np.uid),
        paused: np.paused,
        anchor_ms: now_ms() - np.position_ms,
    });
    {
        let mut last = LAST.lock().unwrap();
        let same = match (&*last, &shown) {
            (None, None) => true,
            // A drift of more than a few seconds means a seek.
            (Some(a), Some(b)) => a.key == b.key && a.paused == b.paused && (a.anchor_ms - b.anchor_ms).abs() < 3000,
            _ => false,
        };
        if same {
            return;
        }
        let new_item = last.as_ref().map(|l| &l.key) != shown.as_ref().map(|s| &s.key);
        *last = shown;
        let settings = app.settings().player;
        update_controls(app, np, new_item, settings.media_controls);
        update_discord(np, &settings.discord_app_id, settings.discord);
    }
}

/// Title, artist-like and album-like lines for an item.
fn lines(item: &Metadata) -> (String, Option<String>, Option<String>) {
    match item.kind() {
        "episode" => (
            item.title().to_string(),
            item.grandparent_title.clone(),
            Some(format!("{} · {}", item.parent_title.as_deref().unwrap_or(""), item.episode_code())),
        ),
        "track" => (item.title().to_string(), item.grandparent_title.clone(), item.parent_title.clone()),
        _ => (item.title().to_string(), item.year.map(|y| y.to_string()), None),
    }
}

/// Media control metadata, owned so it can cross to the main thread.
struct ControlsMeta {
    title: String,
    artist: Option<String>,
    album: Option<String>,
    duration: Option<Duration>,
    thumb: Option<String>,
}

fn update_controls(app: &AppState, np: Option<&NowPlaying>, new_item: bool, enabled: bool) {
    let playback = match np {
        Some(np) if enabled => {
            let progress = Some(MediaPosition(Duration::from_millis(np.position_ms.max(0) as u64)));
            if np.paused { MediaPlayback::Paused { progress } } else { MediaPlayback::Playing { progress } }
        }
        _ => MediaPlayback::Stopped,
    };
    let meta = np.filter(|_| enabled && new_item).map(|np| {
        let (title, artist, album) = lines(&np.item);
        let duration = (np.duration_ms > 0).then(|| Duration::from_millis(np.duration_ms as u64));
        let thumb = match np.item.kind() {
            "episode" => np.item.grandparent_thumb.clone().or(np.item.thumb.clone()),
            "track" => np.item.parent_thumb.clone().or(np.item.thumb.clone()),
            _ => np.item.thumb.clone(),
        };
        ControlsMeta { title, artist, album, duration, thumb }
    });
    let handle = app.handle().clone();
    let set = move |meta: Option<ControlsMeta>, cover: Option<String>| {
        let playback = playback.clone();
        let _ = handle.run_on_main_thread(move || {
            CONTROLS.with(|c| {
                let mut c = c.borrow_mut();
                let Some(controls) = c.as_mut() else { return };
                if let Some(m) = &meta {
                    let _ = controls.set_metadata(MediaMetadata {
                        title: Some(&m.title),
                        artist: m.artist.as_deref(),
                        album: m.album.as_deref(),
                        cover_url: cover.as_deref(),
                        duration: m.duration,
                    });
                }
                let _ = controls.set_playback(playback);
            });
        });
    };
    match meta {
        Some(meta) => {
            let local = np.is_some_and(|n| n.local);
            tauri::async_runtime::spawn(async move {
                // The OS loads artwork itself, so hand it a cached file rather than a tokened URL.
                let cover = match (&meta.thumb, local) {
                    (Some(thumb), false) => crate::images::cached_file(thumb, 600, 600).await,
                    _ => None,
                };
                set(Some(meta), cover.map(|p| format!("file://{}", p.display())));
            });
        }
        None => set(None, None),
    }
}

/// Rich presence contents.
#[derive(Clone, Debug)]
struct Presence {
    app_id: String,
    details: String,
    state: Option<String>,
    listening: bool,
    /// Playing: when it started and will end (unix ms). Paused: `None`.
    span: Option<(i64, i64)>,
}

fn update_discord(np: Option<&NowPlaying>, app_id: &str, enabled: bool) {
    let app_id = app_id.trim();
    let presence = np.filter(|_| enabled && !app_id.is_empty()).map(|np| {
        let (title, artist, album) = lines(&np.item);
        let (details, state) = match np.item.kind() {
            "episode" => (artist.unwrap_or_default(), Some(format!("{} · {title}", np.item.episode_code()))),
            "track" => (title, artist.map(|a| match album {
                Some(al) => format!("{a} — {al}"),
                None => a,
            })),
            _ => (title, artist),
        };
        let start = now_ms() - np.position_ms;
        Presence {
            app_id: app_id.to_string(),
            details,
            state: if np.paused { Some("Paused".into()) } else { state },
            listening: np.item.kind() == "track",
            span: (!np.paused && np.duration_ms > 0).then(|| (start, start + np.duration_ms)),
        }
    });
    let mut tx = DISCORD.lock().unwrap();
    if tx.is_none() {
        if presence.is_none() {
            return;
        }
        let (sender, receiver) = channel();
        std::thread::Builder::new()
            .name("discord".into())
            .spawn(move || discord_thread(receiver))
            .ok();
        *tx = Some(sender);
    }
    if let Some(sender) = tx.as_ref()
        && sender.send(presence).is_err()
    {
        *tx = None;
    }
}

/// Owns the Discord IPC connection; reconnects as needed. Discord may not be running.
fn discord_thread(rx: Receiver<Option<Presence>>) {
    let mut client: Option<(String, DiscordIpcClient)> = None;
    while let Ok(mut presence) = rx.recv() {
        // Only the latest state matters.
        while let Ok(next) = rx.try_recv() {
            presence = next;
        }
        let Some(p) = presence else {
            if let Some((_, c)) = client.as_mut() {
                let _ = c.clear_activity();
            }
            continue;
        };
        if client.as_ref().is_none_or(|(id, _)| *id != p.app_id) {
            if let Some((_, mut old)) = client.take() {
                let _ = old.close();
            }
            let mut c = DiscordIpcClient::new(&p.app_id);
            if c.connect().is_err() {
                continue;
            }
            client = Some((p.app_id.clone(), c));
        }
        let Some((_, c)) = client.as_mut() else { continue };
        let mut activity = Activity::new()
            .details(p.details.clone())
            .activity_type(if p.listening { ActivityType::Listening } else { ActivityType::Watching })
            .status_display_type(StatusDisplayType::Details);
        if let Some(state) = &p.state {
            activity = activity.state(state.clone());
        }
        if let Some((start, end)) = p.span {
            activity = activity.timestamps(Timestamps::new().start(start).end(end));
        }
        if c.set_activity(activity).is_err() {
            // Discord restarted or closed; try again on the next update.
            if let Some((_, mut old)) = client.take() {
                let _ = old.close();
            }
        }
    }
}
