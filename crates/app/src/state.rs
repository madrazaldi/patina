//! Global application state shared through Dioxus context.

use dioxus::core::spawn_forever;
use dioxus::prelude::*;
use patina_plex::{ClientInfo, Directory, Metadata, PlexServer, PlexTv, Resource, User};

use crate::config::{self, Settings};
use crate::session::Stage;

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

impl PartialEq for Session {
    fn eq(&self, other: &Self) -> bool {
        self.server == other.server && self.user == other.user
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NowPlaying {
    pub item: Metadata,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub paused: bool,
    pub transcoding: bool,
    pub queue_len: usize,
    pub queue_pos: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    pub id: u64,
    pub text: String,
    pub error: bool,
}

/// Context menu anchored at screen coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuState {
    pub item: Metadata,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy)]
pub struct AppState {
    pub settings: Signal<Settings>,
    pub stage: Signal<Stage>,
    pub session: Signal<Option<Session>>,
    pub sections: Signal<Vec<Directory>>,
    pub now_playing: Signal<Option<NowPlaying>>,
    pub toasts: Signal<Vec<Toast>>,
    pub menu: Signal<Option<MenuState>>,
    /// Item waiting for the "add to playlist" dialog.
    pub playlist_target: Signal<Option<Metadata>>,
    /// Bumped whenever watch state may have changed; views refetch.
    pub revision: Signal<u64>,
    pub search: Signal<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            settings: Signal::new_in_scope(Settings::load(), ScopeId::ROOT),
            stage: Signal::new_in_scope(Stage::Booting, ScopeId::ROOT),
            session: Signal::new_in_scope(None, ScopeId::ROOT),
            sections: Signal::new_in_scope(Vec::new(), ScopeId::ROOT),
            now_playing: Signal::new_in_scope(None, ScopeId::ROOT),
            toasts: Signal::new_in_scope(Vec::new(), ScopeId::ROOT),
            menu: Signal::new_in_scope(None, ScopeId::ROOT),
            playlist_target: Signal::new_in_scope(None, ScopeId::ROOT),
            revision: Signal::new_in_scope(0, ScopeId::ROOT),
            search: Signal::new_in_scope(String::new(), ScopeId::ROOT),
        }
    }

    pub fn client_info(&self) -> ClientInfo {
        ClientInfo::new(self.settings.peek().client_id.clone(), config::PRODUCT, config::VERSION)
    }

    /// The connected server. Only call below the session gate.
    pub fn server(&self) -> PlexServer {
        self.session.peek().as_ref().expect("no session").server.clone()
    }

    pub fn tv(&self) -> Option<PlexTv> {
        self.session.peek().as_ref().and_then(|s| s.tv.clone())
    }

    pub fn update_settings(&self, f: impl FnOnce(&mut Settings)) {
        let mut settings = self.settings;
        let mut s = settings.write();
        f(&mut s);
        s.save();
    }

    pub fn bump(&self) {
        let mut r = self.revision;
        *r.write() += 1;
    }

    pub fn toast(&self, text: impl Into<String>) {
        self.push_toast(text.into(), false);
    }

    pub fn error(&self, text: impl std::fmt::Display) {
        self.push_toast(text.to_string(), true);
    }

    fn push_toast(&self, text: String, error: bool) {
        let mut toasts = self.toasts;
        let id = toasts.peek().iter().map(|t| t.id).max().unwrap_or(0) + 1;
        toasts.write().push(Toast { id, text, error });
        spawn_forever(async move {
            tokio::time::sleep(std::time::Duration::from_millis(if error { 6000 } else { 3200 })).await;
            toasts.write().retain(|t| t.id != id);
        });
    }
}

pub fn use_app() -> AppState {
    use_context::<AppState>()
}
