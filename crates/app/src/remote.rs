//! What's playing, shown outside the app: the OS media controls (macOS Now
//! Playing, MPRIS on Linux, SMTC on Windows) and Discord Rich Presence
//! (in core). Both follow `NowPlaying` events; media keys and the OS widgets
//! drive the player through [`player::control`].

use std::cell::RefCell;
use std::sync::Mutex;
use std::time::Duration;

use patina_core::config::PlayerSettings;
use patina_core::player::{self, Control};
use patina_core::presence::{self, Shown, lines};
use patina_core::state::NowPlaying;
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig, SeekDirection};
use tauri::{AppHandle, Manager};

thread_local! {
    // Created and used on the main thread only.
    static CONTROLS: RefCell<Option<MediaControls>> = const { RefCell::new(None) };
}

/// What the media controls last showed, to skip redundant updates.
static LAST: Mutex<Option<Shown>> = Mutex::new(None);

/// Set up the media controls. Call on the main thread during setup.
pub fn init(app: &AppHandle) {
    #[cfg(target_os = "windows")]
    let hwnd = app
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
    let handle = app.clone();
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

/// Follow the now-playing state. Cheap when nothing changed.
pub fn update(app: &AppHandle, np: Option<&NowPlaying>, settings: &PlayerSettings) {
    presence::update(np, settings);
    let shown = np.map(Shown::of);
    let mut last = LAST.lock().unwrap();
    if Shown::same(last.as_ref(), shown.as_ref()) {
        return;
    }
    let new_item = last.as_ref().map(|l| &l.key) != shown.as_ref().map(|s| &s.key);
    *last = shown;
    update_controls(app, np, new_item, settings.media_controls);
}

/// Media control metadata, owned so it can cross to the main thread.
struct ControlsMeta {
    title: String,
    artist: Option<String>,
    album: Option<String>,
    duration: Option<Duration>,
    thumb: Option<String>,
}

fn update_controls(app: &AppHandle, np: Option<&NowPlaying>, new_item: bool, enabled: bool) {
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
    let handle = app.clone();
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
                    (Some(thumb), false) => patina_core::images::cached_file(thumb, 600, 600).await,
                    _ => None,
                };
                set(Some(meta), cover.map(|p| format!("file://{}", p.display())));
            });
        }
        None => set(None, None),
    }
}
