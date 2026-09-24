//! What's playing as Discord Rich Presence. Front ends feed it every
//! now-playing change; it skips updates that change nothing visible.

use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{SystemTime, UNIX_EPOCH};

use discord_rich_presence::activity::{Activity, ActivityType, StatusDisplayType, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use patina_plex::Metadata;

use crate::config::PlayerSettings;
use crate::state::NowPlaying;

/// What was last shown, to skip redundant updates.
#[derive(Clone, PartialEq)]
pub struct Shown {
    pub key: String,
    pub paused: bool,
    /// Where playback should be now if it kept going (unix ms minus position).
    pub anchor_ms: i64,
}

impl Shown {
    pub fn of(np: &NowPlaying) -> Self {
        Self { key: format!("{}:{}", np.item.rating_key(), np.uid), paused: np.paused, anchor_ms: now_ms() - np.position_ms }
    }

    /// Whether going from `a` to `b` changes nothing visible. A drift of
    /// more than a few seconds means a seek.
    pub fn same(a: Option<&Shown>, b: Option<&Shown>) -> bool {
        match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => a.key == b.key && a.paused == b.paused && (a.anchor_ms - b.anchor_ms).abs() < 3000,
            _ => false,
        }
    }
}

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Title, artist-like and album-like lines for an item.
pub fn lines(item: &Metadata) -> (String, Option<String>, Option<String>) {
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

static LAST: Mutex<Option<Shown>> = Mutex::new(None);
static DISCORD: Mutex<Option<Sender<Option<Presence>>>> = Mutex::new(None);

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

/// Follow the now-playing state. Cheap when nothing changed.
pub fn update(np: Option<&NowPlaying>, settings: &PlayerSettings) {
    let shown = np.map(Shown::of);
    {
        let mut last = LAST.lock().unwrap();
        if Shown::same(last.as_ref(), shown.as_ref()) {
            return;
        }
        *last = shown;
    }
    let app_id = settings.discord_app_id.trim();
    let presence = np.filter(|_| settings.discord && !app_id.is_empty()).map(|np| {
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
