//! Application state shared by commands, the session flow and the player.
//! Changes are pushed to the webview as events; the webview asks for a
//! [`Snapshot`] once on startup and follows the events from then on.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use patina_plex::{ClientInfo, Directory, Metadata, PlexServer, PlexTv, Resource, User};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::config::{self, Settings};

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

/// A plex.tv account as the webview sees it (no tokens).
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

/// A server resource as the webview sees it (no tokens or connections).
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
}

#[derive(Clone, Debug, Serialize)]
pub struct Toast {
    pub text: String,
    pub error: bool,
}

/// Everything the webview needs to render its first frame.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub stage: Stage,
    pub session: Option<SessionView>,
    pub sections: Vec<Directory>,
    pub now_playing: Option<NowPlaying>,
    pub revision: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionEvent {
    session: Option<SessionView>,
    sections: Vec<Directory>,
}

pub struct Inner {
    handle: AppHandle,
    settings: RwLock<Settings>,
    stage: RwLock<Stage>,
    session: RwLock<Option<Session>>,
    sections: RwLock<Vec<Directory>>,
    now_playing: RwLock<Option<NowPlaying>>,
    revision: AtomicU64,
    /// Bumped to abandon an in-flight PIN sign-in.
    pub login_attempt: AtomicU64,
    /// Servers offered on the "choose a server" screen.
    pub pending: Mutex<Option<(PlexTv, User, Vec<Resource>)>>,
    /// Episodes loaded by the bulk audio/subtitle dialog, keyed by show.
    pub tracks: Mutex<Option<(String, Arc<Vec<Metadata>>)>>,
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
    pub fn new(handle: AppHandle) -> Self {
        Self(Arc::new(Inner {
            handle,
            settings: RwLock::new(Settings::load()),
            stage: RwLock::new(Stage::Booting),
            session: RwLock::new(None),
            sections: RwLock::new(Vec::new()),
            now_playing: RwLock::new(None),
            revision: AtomicU64::new(0),
            login_attempt: AtomicU64::new(0),
            pending: Mutex::new(None),
            tracks: Mutex::new(None),
        }))
    }

    fn emit<S: Serialize + Clone>(&self, event: &str, payload: S) {
        if let Err(e) = self.handle.emit(event, payload) {
            tracing::warn!("could not emit {event}: {e}");
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            stage: self.stage.read().unwrap().clone(),
            session: self.session.read().unwrap().as_ref().map(SessionView::from),
            sections: self.sections.read().unwrap().clone(),
            now_playing: self.now_playing.read().unwrap().clone(),
            revision: self.revision.load(Ordering::Relaxed),
        }
    }

    pub fn client_info(&self) -> ClientInfo {
        ClientInfo::new(self.settings.read().unwrap().client_id.clone(), config::PRODUCT, config::VERSION)
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap().clone()
    }

    pub fn update_settings(&self, f: impl FnOnce(&mut Settings)) {
        let mut s = self.settings.write().unwrap();
        f(&mut s);
        s.save();
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

    pub fn set_stage(&self, stage: Stage) {
        *self.stage.write().unwrap() = stage.clone();
        self.emit("stage", stage);
    }

    pub fn set_session(&self, session: Option<Session>, sections: Vec<Directory>) {
        let view = session.as_ref().map(SessionView::from);
        *self.session.write().unwrap() = session;
        *self.sections.write().unwrap() = sections.clone();
        self.emit("session", SessionEvent { session: view, sections });
    }

    pub fn set_sections(&self, sections: Vec<Directory>) {
        let session = self.session.read().unwrap().as_ref().map(SessionView::from);
        *self.sections.write().unwrap() = sections.clone();
        self.emit("session", SessionEvent { session, sections });
    }

    pub fn set_now_playing(&self, np: Option<NowPlaying>) {
        *self.now_playing.write().unwrap() = np.clone();
        self.emit("now-playing", np);
    }

    /// Watch state may have changed; views refetch.
    pub fn bump(&self) {
        let r = self.revision.fetch_add(1, Ordering::Relaxed) + 1;
        self.emit("revision", r);
    }

    pub fn toast(&self, text: impl Into<String>) {
        self.emit("toast", Toast { text: text.into(), error: false });
    }

    pub fn error(&self, text: impl std::fmt::Display) {
        self.emit("toast", Toast { text: text.to_string(), error: true });
    }
}
