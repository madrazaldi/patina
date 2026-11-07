//! App-wide state the views render from: a mirror of the core's state, kept
//! current by its events, plus UI state shared between views (search text,
//! the open player panel).

use crate::prelude::*;
use std::collections::VecDeque;
use std::future::Future;

use futures::StreamExt as _;
use futures::channel::mpsc::UnboundedReceiver;
use patina_core::config::{PlayerSettings, Settings, UiSettings};
use patina_core::controls::PlaybackState;
use patina_core::downloads::Download;
use patina_core::failure::PlaybackFailure;
use patina_core::ops::setup::Info;
use patina_core::recovery::SavedQueueView;
use patina_core::state::{Changes, NowPlaying, QueueView, ServerActivity, SessionView, Stage};
use patina_core::{CoreEvent, ops};
use patina_plex::Directory;

use crate::core::{bg, blocking, core, message};

/// The panel above the now-playing bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    Queue,
    Lyrics,
    Player,
}

/// Something views are told about.
#[derive(Clone, Debug)]
pub enum StoreEvent {
    /// Show a toast; `undo` names the undo entry that reverses what it reports.
    Toast { text: String, error: bool, undo: Option<u64> },
    /// The account or server changed: history, editors and pages of the old one go.
    ScopeChanged,
    /// An undo entry was used: its toast goes.
    Undone(u64),
}

pub struct Store {
    pub connected: bool,
    pub stage: Stage,
    pub session: Option<SessionView>,
    pub sections: Vec<Directory>,
    pub now_playing: Option<NowPlaying>,
    pub queue: Option<QueueView>,
    /// Volume, speed, repeat, sleep timer, chapters and diagnostics of the running player.
    pub playback: Option<PlaybackState>,
    /// Why playback last failed, until it is retried or dismissed.
    pub failure: Option<PlaybackFailure>,
    /// The queue that was playing when Patina last stopped, offered as a Resume (never auto-played).
    pub saved_queue: Option<SavedQueueView>,
    /// Server background tasks, live.
    pub activities: Vec<ServerActivity>,
    pub downloads: Vec<Download>,
    /// Whether the server has a DVR (Live TV page).
    pub live_tv: bool,
    /// Whether a Seerr instance is set up (Requests page).
    pub seerr: bool,
    /// Bumped whenever watch state may have changed; views refetch.
    pub revision: u64,
    /// Bumped when playback sessions on the server change (Activity page refetches).
    pub sessions_tick: u64,
    /// Bumped on every downloads event (smart-download policies and the budget come with them).
    pub downloads_tick: u64,
    pub settings: Settings,
    pub info: Info,
    /// The top bar's search text.
    pub search: SharedString,
    /// Search every server of the account instead of the connected one.
    pub search_all: bool,
    pub panel: Option<Panel>,
    pending: Option<Changes>,
    log: VecDeque<(u64, Changes)>,
}

impl EventEmitter<StoreEvent> for Store {}

/// The store, reachable from anywhere with an `App`.
pub struct StoreHandle(pub Entity<Store>);

impl Global for StoreHandle {}

/// The app's store.
pub fn store(cx: &App) -> Entity<Store> {
    cx.global::<StoreHandle>().0.clone()
}

/// Create the store and apply core events to it as they arrive.
pub fn init(mut events: UnboundedReceiver<CoreEvent>, cx: &mut App) {
    let app = core(cx);
    let snap = app.snapshot();
    let entity = cx.new(|_| Store {
        connected: snap.connected,
        stage: snap.stage,
        session: snap.session,
        sections: snap.sections,
        now_playing: snap.now_playing,
        queue: snap.queue,
        playback: patina_core::controls::playback_state(),
        failure: patina_core::failure::current(),
        saved_queue: None,
        activities: snap.activities,
        downloads: patina_core::downloads::list(),
        live_tv: false,
        seerr: false,
        revision: snap.revision,
        sessions_tick: 0,
        downloads_tick: 0,
        settings: app.settings(),
        info: ops::setup::info(),
        search: SharedString::default(),
        search_all: false,
        panel: None,
        pending: None,
        log: VecDeque::new(),
    });
    cx.set_global(StoreHandle(entity.clone()));
    entity.update(cx, |s, cx| s.refresh_seerr(cx));
    cx.spawn(async move |cx| {
        while let Some(event) = events.next().await {
            entity.update(cx, |s, cx| s.apply(event, cx));
        }
    })
    .detach();
}

impl Store {
    /// The account and server, so late results for another one can be dropped.
    pub fn scope(&self) -> String {
        self.session.as_ref().map(|s| format!("{}:{}", s.account_id, s.server_id)).unwrap_or_default()
    }

    pub fn owned(&self) -> bool {
        self.session.as_ref().is_some_and(|s| s.owned)
    }

    pub fn has_tv(&self) -> bool {
        self.session.as_ref().is_some_and(|s| s.has_tv)
    }

    pub fn spoilers(&self) -> bool {
        self.settings.ui.hide_spoilers
    }

    /// The accent colour the user picked.
    pub fn tint(&self) -> Hsla {
        let id = self.settings.ui.accent.as_str();
        let hex = patina_core::config::ACCENTS.iter().find(|a| a.id == id).map_or("#e6e1d6", |a| a.color);
        crate::theme::hex(hex)
    }

    pub fn poster_width(&self) -> f32 {
        self.settings.ui.poster_size.clamp(100, 320) as f32
    }

    fn apply(&mut self, event: CoreEvent, cx: &mut Context<Self>) {
        match event {
            CoreEvent::Connection(on) => self.connected = on,
            CoreEvent::SettingsChanged => {
                self.settings = core(cx).settings();
                self.refresh_seerr(cx);
            }
            CoreEvent::Stage(stage) => self.stage = stage,
            CoreEvent::Session { session, sections } => {
                let before = self.scope();
                self.session = session;
                self.sections = sections;
                if before != self.scope() {
                    self.search = SharedString::default();
                    self.search_all = false;
                    self.saved_queue = None;
                    self.live_tv = false;
                    cx.emit(StoreEvent::ScopeChanged);
                    self.load_saved_queue(cx);
                    self.check_live_tv(cx);
                }
            }
            CoreEvent::NowPlaying(np) => {
                self.now_playing = np.map(|b| *b);
                // Once something plays the offer is moot (resuming, or the user chose otherwise).
                if self.now_playing.is_some() {
                    self.saved_queue = None;
                }
            }
            CoreEvent::Queue(q) => {
                // The queue only goes away when playback ends, unlike now-playing between tracks.
                if q.is_none() {
                    self.panel = None;
                }
                self.queue = q;
            }
            CoreEvent::Activities(list) => self.activities = list,
            CoreEvent::Revision(_) => self.bump(),
            CoreEvent::Changes(c) => {
                let p = self.pending.get_or_insert_with(Changes::default);
                p.all |= c.all;
                for s in c.sections {
                    if !p.sections.contains(&s) {
                        p.sections.push(s);
                    }
                }
                for i in c.items {
                    if !p.items.contains(&i) {
                        p.items.push(i);
                    }
                }
            }
            CoreEvent::Toast(t) => cx.emit(StoreEvent::Toast { text: t.text, error: t.error, undo: t.undo }),
            CoreEvent::Downloads(list) => {
                self.downloads = list;
                self.downloads_tick += 1;
            }
            CoreEvent::SessionsChanged => self.sessions_tick += 1,
            CoreEvent::PlaybackState(s) => self.playback = s.map(|b| *b),
            CoreEvent::PlaybackFailure(f) => self.failure = f.map(|b| *b),
        }
        cx.notify();
    }

    /// A revision without a preceding `Changes` could affect anything.
    fn bump(&mut self) {
        let c = self.pending.take().unwrap_or_else(Changes::everything);
        self.revision += 1;
        self.log.push_back((self.revision, c));
        if self.log.len() > 64 {
            self.log.pop_front();
        }
    }

    /// Refetch everything that depends on watch state.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.pending = None;
        self.bump();
        cx.notify();
    }

    /// Whether anything in library section `section` may have changed since `revision`.
    pub fn affects(&self, section: &str, since: u64) -> bool {
        if since >= self.revision {
            return false;
        }
        let Some((first, _)) = self.log.front() else { return true };
        if *first > since + 1 {
            return true;
        }
        self.log.iter().any(|(r, c)| *r > since && (c.all || c.sections.iter().any(|s| s == section)))
    }

    /// Whether an item may have changed since `revision`.
    pub fn affects_item(&self, rating_key: &str, since: u64) -> bool {
        if since >= self.revision {
            return false;
        }
        let Some((first, _)) = self.log.front() else { return true };
        if *first > since + 1 {
            return true;
        }
        self.log.iter().any(|(r, c)| *r > since && (c.all || !c.sections.is_empty() || c.items.iter().any(|i| i == rating_key)))
    }

    pub fn toast(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        cx.emit(StoreEvent::Toast { text: text.into(), error: false, undo: None });
    }

    pub fn error(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        cx.emit(StoreEvent::Toast { text: text.into(), error: true, undo: None });
    }

    /// Look for a saved queue to offer, unless something is already playing.
    pub fn load_saved_queue(&mut self, cx: &mut Context<Self>) {
        if self.now_playing.is_some() || self.session.is_none() {
            self.saved_queue = None;
            return;
        }
        let scope = self.scope();
        let app = core(cx);
        cx.spawn(async move |this, cx| {
            let saved = blocking(move || ops::playback::saved_queue(&app)).await;
            this.update(cx, |s, cx| {
                if s.scope() == scope && s.now_playing.is_none() {
                    s.saved_queue = saved;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Live TV shows in the sidebar when the server has a DVR.
    pub fn check_live_tv(&mut self, cx: &mut Context<Self>) {
        let scope = self.scope();
        let app = core(cx);
        if self.session.is_none() {
            return;
        }
        cx.spawn(async move |this, cx| {
            let on = bg(async move {
                match app.server() {
                    Ok(server) => patina_core::livetv::available(&server).await,
                    Err(_) => false,
                }
            })
            .await;
            this.update(cx, |s, cx| {
                if s.scope() == scope {
                    s.live_tv = on;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Whether a Seerr instance is set up (no network).
    pub fn refresh_seerr(&mut self, _cx: &mut Context<Self>) {
        self.seerr = !self.settings.seerr.url.trim().is_empty();
    }

    /// Change player settings now and save them in the background.
    pub fn update_player(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut PlayerSettings) + Send + Clone + 'static) {
        f.clone()(&mut self.settings.player);
        cx.notify();
        self.save(cx, move |s| f(&mut s.player));
    }

    /// Change interface settings now and save them in the background.
    pub fn update_ui(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut UiSettings) + Send + Clone + 'static) {
        f.clone()(&mut self.settings.ui);
        cx.notify();
        self.save(cx, move |s| f(&mut s.ui));
    }

    fn save(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Settings) + Send + 'static) {
        let app = core(cx);
        cx.spawn(async move |this, cx| {
            let result = blocking(move || app.try_update_settings(|s| {
                f(s);
                Ok(())
            }))
            .await;
            if let Err(e) = result {
                this.update(cx, |s, cx| s.error(format!("Could not save settings: {}", message(&e)), cx)).ok();
            }
        })
        .detach();
    }

}

/// Toast an error from anywhere.
pub fn error(cx: &mut App, text: impl Into<String>) {
    let text = text.into();
    store(cx).update(cx, |s, cx| s.error(text, cx));
}

/// Toast a message from anywhere.
pub fn toast(cx: &mut App, text: impl Into<String>) {
    let text = text.into();
    store(cx).update(cx, |s, cx| s.toast(text, cx));
}

/// Run a core operation in the background: toast `ok` when it succeeds (and
/// refresh the views if asked), or the error.
pub fn act<T: Send + 'static>(cx: &mut App, fut: impl Future<Output = anyhow::Result<T>> + Send + 'static, ok: Option<String>, refresh: bool) {
    let store = store(cx);
    cx.spawn(async move |cx| {
        let result = bg(fut).await;
        store
            .update(cx, |s, cx| match result {
                Ok(_) => {
                    if let Some(ok) = ok {
                        s.toast(ok, cx);
                    }
                    if refresh {
                        s.refresh(cx);
                    }
                }
                Err(e) => s.error(message(&e), cx),
            });
    })
    .detach();
}

/// Run a core operation in the background and hand its result to `then` on the UI thread
/// (errors are toasted instead).
pub fn act_then<T: Send + 'static>(
    cx: &mut App,
    fut: impl Future<Output = anyhow::Result<T>> + Send + 'static,
    then: impl FnOnce(T, &mut App) + 'static,
) {
    let store = store(cx);
    cx.spawn(async move |cx| {
        let result = bg(fut).await;
        cx.update(|cx| match result {
            Ok(v) => then(v, cx),
            Err(e) => store.update(cx, |s, cx| s.error(message(&e), cx)),
        });
    })
    .detach();
}
