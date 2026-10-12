//! Application state shared by the front end, the session flow and the player.
//! Changes are pushed to the front end as [`CoreEvent`]s; it asks for a
//! [`Snapshot`] once on startup and follows the events from then on.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use patina_plex::{ClientInfo, Directory, Metadata, PlexServer, PlexTv, Resource, User};
use serde::Serialize;
use serde_json::{Value, json};

use crate::config::{self, Settings};
use crate::downloads::Download;

/// Where the app is in the sign-in / connect flow.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Stage {
    Booting,
    Login,
    PickServer { servers: Vec<ServerView>, user: UserView },
    Connecting { name: String },
    Ready,
    Failed { message: String },
}

/// A connected server plus the account that reached it.
#[derive(Clone)]
pub struct Session {
    /// plex.tv client; `None` for manually configured servers.
    pub tv: Option<PlexTv>,
    pub server: PlexServer,
    pub server_name: String,
    pub server_id: String,
    pub user: Option<User>,
    /// Whether the signed-in user owns (administers) this server.
    pub owned: bool,
    pub servers: Vec<Resource>,
}

/// A plex.tv account as front ends see it (no tokens).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserView {
    pub uuid: Option<String>,
    pub name: String,
    pub email: Option<String>,
    pub thumb: Option<String>,
    pub plex_pass: bool,
}

impl From<&User> for UserView {
    fn from(u: &User) -> Self {
        Self {
            uuid: u.uuid.clone(),
            name: u.display_name().to_string(),
            email: u.email.clone(),
            thumb: u.thumb.clone(),
            plex_pass: u.subscription.as_ref().is_some_and(|s| s.active),
        }
    }
}

/// A server resource as front ends see it (no tokens or connections).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerView {
    pub id: String,
    pub name: String,
    pub owned: bool,
    pub source_title: Option<String>,
    pub platform: Option<String>,
    pub version: Option<String>,
    pub presence: bool,
}

impl From<&Resource> for ServerView {
    fn from(r: &Resource) -> Self {
        Self {
            id: r.id().to_string(),
            name: r.name().to_string(),
            owned: r.owned,
            source_title: r.source_title.clone(),
            platform: r.platform.clone(),
            version: r.product_version.clone(),
            presence: r.presence,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub account_id: String,
    pub server_name: String,
    pub server_id: String,
    pub machine_identifier: Option<String>,
    pub base_url: String,
    pub owned: bool,
    pub has_tv: bool,
    pub user: Option<UserView>,
    pub servers: Vec<ServerView>,
}

impl From<&Session> for SessionView {
    fn from(s: &Session) -> Self {
        Self {
            account_id: crate::offline::account(s),
            server_name: s.server_name.clone(),
            server_id: s.server_id.clone(),
            machine_identifier: s.server.machine_identifier().map(str::to_string),
            base_url: s.server.base_url().to_string(),
            owned: s.owned,
            has_tv: s.tv.is_some(),
            user: s.user.as_ref().map(UserView::from),
            servers: s.servers.iter().map(ServerView::from).collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NowPlaying {
    pub item: Metadata,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub paused: bool,
    pub transcoding: bool,
    pub queue_len: usize,
    pub queue_pos: usize,
    /// Queue entry id of the item (see [`QueueView`]).
    pub uid: u64,
    /// In the closing credits or last seconds with something queued after: show "Up next".
    pub ending: bool,
    /// Playback stops when this item ends.
    pub stop_after: bool,
    pub live: bool,
    /// A downloaded file played offline.
    pub local: bool,
}

/// The running play queue, in play order.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueView {
    pub items: Vec<QueueItem>,
    pub current: Option<u64>,
    pub shuffled: bool,
    pub stop_after: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct QueueItem {
    pub uid: u64,
    pub item: Metadata,
}

/// A background task on the server (scan, analysis, ...), from live notifications.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerActivity {
    pub uuid: String,
    pub kind: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub progress: i64,
    pub cancellable: bool,
    pub section_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Toast {
    pub text: String,
    pub error: bool,
    /// Id of an undo entry (see [`crate::undo`]) that reverses what the toast reports.
    /// Absent for ordinary toasts, so their payload is unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub undo: Option<u64>,
}

/// Everything a front end needs to render its first frame.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub connected: bool,
    pub stage: Stage,
    pub session: Option<SessionView>,
    pub sections: Vec<Directory>,
    pub now_playing: Option<NowPlaying>,
    pub queue: Option<QueueView>,
    pub activities: Vec<ServerActivity>,
    pub revision: u64,
}

/// What the next [`CoreEvent::Revision`] covers, so views can refetch only what changed.
/// `all` means the change could not be narrowed down (watch state, unknown sections).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Changes {
    pub all: bool,
    /// Library section ids that changed.
    pub sections: Vec<String>,
    /// Rating keys that changed, when the server said which.
    pub items: Vec<String>,
}

impl Changes {
    pub fn everything() -> Self {
        Self { all: true, ..Self::default() }
    }

    /// A change in one library section, optionally to one item.
    pub fn section(section: Option<&str>, item: Option<&str>) -> Self {
        let mut c = Self::default();
        c.add(section, item);
        c
    }

    /// Fold another change in. Without a section the change could be anywhere.
    pub fn add(&mut self, section: Option<&str>, item: Option<&str>) {
        match section {
            Some(s) => {
                if !self.sections.iter().any(|x| x == s) {
                    self.sections.push(s.to_string());
                }
            }
            None => self.all = true,
        }
        if let Some(i) = item
            && !self.items.iter().any(|x| x == i)
        {
            self.items.push(i.to_string());
        }
    }
}

/// A state change for the front end.
#[derive(Clone, Debug)]
pub enum CoreEvent {
    Connection(bool),
    SettingsChanged,
    Stage(Stage),
    Session { session: Option<SessionView>, sections: Vec<Directory> },
    NowPlaying(Option<Box<NowPlaying>>),
    Queue(Option<QueueView>),
    Activities(Vec<ServerActivity>),
    /// Watch state may have changed; views refetch.
    Revision(u64),
    /// What the revision that follows covers (sent just before it). A revision with
    /// no preceding `Changes` may affect anything.
    Changes(Changes),
    Toast(Toast),
    Downloads(Vec<Download>),
    /// Playback sessions on the server changed.
    SessionsChanged,
}

impl CoreEvent {
    /// The event name the GUI's webview listens for.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Connection(_) => "connection",
            Self::SettingsChanged => "settings-changed",
            Self::Stage(_) => "stage",
            Self::Session { .. } => "session",
            Self::NowPlaying(_) => "now-playing",
            Self::Queue(_) => "queue",
            Self::Activities(_) => "activities",
            Self::Revision(_) => "revision",
            Self::Changes(_) => "library-changed",
            Self::Toast(_) => "toast",
            Self::Downloads(_) => "downloads",
            Self::SessionsChanged => "sessions-changed",
        }
    }

    /// The event payload as the webview receives it.
    pub fn payload(&self) -> Value {
        let v = match self {
            Self::Connection(online) => Ok(json!(online)),
            Self::SettingsChanged => Ok(Value::Null),
            Self::Stage(s) => serde_json::to_value(s),
            Self::Session { session, sections } => Ok(json!({ "session": session, "sections": sections })),
            Self::NowPlaying(np) => serde_json::to_value(np),
            Self::Queue(q) => serde_json::to_value(q),
            Self::Activities(a) => serde_json::to_value(a),
            Self::Revision(r) => Ok(json!(r)),
            Self::Changes(c) => serde_json::to_value(c),
            Self::Toast(t) => serde_json::to_value(t),
            Self::Downloads(d) => serde_json::to_value(d),
            Self::SessionsChanged => Ok(Value::Null),
        };
        v.unwrap_or_else(|e| {
            tracing::warn!("could not encode {}: {e}", self.name());
            Value::Null
        })
    }
}

/// Receives every [`CoreEvent`]; called from any thread.
pub trait EventSink: Send + Sync + 'static {
    fn emit(&self, event: CoreEvent);
}

pub struct Inner {
    sink: Arc<dyn EventSink>,
    runtime: tokio::runtime::Handle,
    settings: RwLock<Settings>,
    stage: RwLock<Stage>,
    session: RwLock<Option<Session>>,
    sections: RwLock<Vec<Directory>>,
    now_playing: RwLock<Option<NowPlaying>>,
    queue: RwLock<Option<QueueView>>,
    activities: RwLock<Vec<ServerActivity>>,
    revision: AtomicU64,
    connected: AtomicBool,
    /// Bumped whenever the session changes, which stops the previous
    /// server's notification listener.
    pub live_generation: AtomicU64,
    /// Bumped to abandon an in-flight PIN sign-in.
    pub login_attempt: AtomicU64,
    /// Servers offered on the "choose a server" screen.
    pub pending: Mutex<Option<(PlexTv, User, Vec<Resource>)>>,
    /// Episodes loaded by the bulk audio/subtitle dialog, keyed by show.
    pub tracks: Mutex<Option<(String, Arc<Vec<Metadata>>)>>,
    /// Reversible actions of the current account and server (see [`crate::undo`]).
    pub undo: Mutex<crate::undo::Stack>,
}

/// Cheap handle to the shared state.
#[derive(Clone)]
pub struct AppState(Arc<Inner>);

impl std::ops::Deref for AppState {
    type Target = Inner;
    fn deref(&self) -> &Inner {
        &self.0
    }
}

impl AppState {
    /// Load the settings; background work runs on `runtime`.
    pub fn new(sink: Arc<dyn EventSink>, runtime: tokio::runtime::Handle) -> Self {
        let _ = crate::RUNTIME.set(runtime.clone());
        let app = Self(Arc::new(Inner {
            sink,
            runtime,
            settings: RwLock::new(Settings::load()),
            stage: RwLock::new(Stage::Booting),
            session: RwLock::new(None),
            sections: RwLock::new(Vec::new()),
            now_playing: RwLock::new(None),
            queue: RwLock::new(None),
            activities: RwLock::new(Vec::new()),
            revision: AtomicU64::new(0),
            connected: AtomicBool::new(true),
            live_generation: AtomicU64::new(0),
            login_attempt: AtomicU64::new(0),
            pending: Mutex::new(None),
            tracks: Mutex::new(None),
            undo: Mutex::new(Default::default()),
        }));
        let weak = Arc::downgrade(&app.0);
        app.spawn(async move {
            let mut downloads = String::new();
            let mut settings = String::new();
            let mut ticks = 0;
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                let Some(inner) = weak.upgrade() else { break };
                let app = Self(inner);
                let list = crate::downloads::list();
                let next = serde_json::to_string(&list).unwrap_or_default();
                if next != downloads { downloads = next; app.emit(CoreEvent::Downloads(list.clone())); }
                if list.iter().any(|d| matches!(d.state, crate::downloads::State::Queued | crate::downloads::State::Downloading)) {
                    crate::downloads::resume(&app);
                }
                let s = app.settings();
                let next = serde_json::to_string(&(&s.player, &s.ui, &s.tui)).unwrap_or_default();
                if next != settings { settings = next; app.emit(CoreEvent::SettingsChanged); }
                ticks += 1;
                if ticks % 15 == 1 {
                    let syncing = app.clone();
                    app.spawn(async move {
                        if let Err(e) = crate::offline::sync(&syncing).await {
                            tracing::debug!("offline progress remains queued: {e:#}");
                        }
                    });
                }
            }
        });
        app
    }

    pub fn emit(&self, event: CoreEvent) {
        self.sink.emit(event);
    }

    pub fn spawn<F>(&self, fut: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.runtime.spawn(fut)
    }

    pub fn runtime(&self) -> &tokio::runtime::Handle {
        &self.runtime
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            connected: self.connected.load(Ordering::Relaxed),
            stage: self.stage.read().unwrap().clone(),
            session: self.session.read().unwrap().as_ref().map(SessionView::from),
            sections: self.sections.read().unwrap().clone(),
            now_playing: self.now_playing.read().unwrap().clone(),
            queue: self.queue.read().unwrap().clone(),
            activities: self.activities.read().unwrap().clone(),
            revision: self.revision.load(Ordering::Relaxed),
        }
    }

    pub fn client_info(&self) -> ClientInfo {
        ClientInfo::new(self.settings.read().unwrap().client_id.clone(), config::PRODUCT, config::VERSION)
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap().reload()
    }

    /// Change and save the settings. The file is re-read first so changes
    /// made by another Patina process are kept.
    pub fn update_settings(&self, f: impl FnOnce(&mut Settings)) {
        if let Err(e) = self.try_update_settings(|s| { f(s); Ok(()) }) {
            self.error(format!("Could not save settings: {e:#}"));
        }
    }

    pub fn try_update_settings(&self, f: impl FnOnce(&mut Settings) -> anyhow::Result<()>) -> anyhow::Result<()> {
        let mut s = self.settings.write().unwrap();
        *s = s.update(f)?;
        Ok(())
    }

    pub fn session(&self) -> Option<Session> {
        self.session.read().unwrap().clone()
    }

    /// The connected server.
    pub fn server(&self) -> anyhow::Result<PlexServer> {
        self.session().map(|s| s.server).ok_or_else(|| anyhow::anyhow!("Not connected to a server."))
    }

    pub fn tv(&self) -> Option<PlexTv> {
        self.session.read().unwrap().as_ref().and_then(|s| s.tv.clone())
    }

    /// The plex.tv client, or an error when not signed in to plex.tv.
    pub fn plex_tv(&self) -> anyhow::Result<PlexTv> {
        self.tv().ok_or_else(|| anyhow::anyhow!("Not signed in to plex.tv."))
    }

    pub fn set_connected(&self, online: bool) {
        let was = self.connected.swap(online, Ordering::Relaxed);
        if was != online {
            self.emit(CoreEvent::Connection(online));
            if online { self.bump(); }
        }
    }

    pub fn set_stage(&self, stage: Stage) {
        *self.stage.write().unwrap() = stage.clone();
        self.emit(CoreEvent::Stage(stage));
    }

    pub fn set_session(&self, session: Option<Session>, sections: Vec<Directory>) {
        let view = session.as_ref().map(SessionView::from);
        crate::undo::session_changed(self, session.as_ref());
        *self.session.write().unwrap() = session;
        *self.sections.write().unwrap() = sections.clone();
        self.emit(CoreEvent::Session { session: view, sections });
    }

    pub fn sections(&self) -> Vec<Directory> {
        self.sections.read().unwrap().clone()
    }

    pub fn set_sections(&self, sections: Vec<Directory>) {
        let session = self.session.read().unwrap().as_ref().map(SessionView::from);
        *self.sections.write().unwrap() = sections.clone();
        self.emit(CoreEvent::Session { session, sections });
    }

    pub fn now_playing(&self) -> Option<NowPlaying> {
        self.now_playing.read().unwrap().clone()
    }

    pub fn set_now_playing(&self, np: Option<NowPlaying>) {
        *self.now_playing.write().unwrap() = np.clone();
        self.emit(CoreEvent::NowPlaying(np.map(Box::new)));
    }

    pub fn set_queue(&self, queue: Option<QueueView>) {
        *self.queue.write().unwrap() = queue.clone();
        self.emit(CoreEvent::Queue(queue));
    }

    pub fn set_activities(&self, list: Vec<ServerActivity>) {
        *self.activities.write().unwrap() = list.clone();
        self.emit(CoreEvent::Activities(list));
    }

    pub fn update_activities(&self, f: impl FnOnce(&mut Vec<ServerActivity>)) {
        let list = {
            let mut a = self.activities.write().unwrap();
            f(&mut a);
            a.clone()
        };
        self.emit(CoreEvent::Activities(list));
    }

    /// Watch state may have changed; views refetch.
    pub fn bump(&self) {
        let r = self.revision.fetch_add(1, Ordering::Relaxed) + 1;
        self.emit(CoreEvent::Revision(r));
    }

    /// Like [`bump`](Self::bump), telling front ends which sections and items changed.
    pub fn bump_with(&self, changes: Changes) {
        self.emit(CoreEvent::Changes(changes));
        self.bump();
    }

    pub fn toast(&self, text: impl Into<String>) {
        self.emit(CoreEvent::Toast(Toast { text: text.into(), error: false, undo: None }));
    }

    /// A toast that offers to undo what it reports.
    pub fn toast_undo(&self, text: impl Into<String>, undo: u64) {
        self.emit(CoreEvent::Toast(Toast { text: text.into(), error: false, undo: Some(undo) }));
    }

    pub fn error(&self, text: impl std::fmt::Display) {
        self.emit(CoreEvent::Toast(Toast { text: text.to_string(), error: true, undo: None }));
    }
}
