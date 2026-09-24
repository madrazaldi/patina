//! Application state and update logic.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use patina_core::config::{QUALITIES, TuiSettings};
use patina_core::downloads::{self, Download, State as DownloadState};
use patina_core::ops::edit::{self, TagEdit};
use patina_core::ops::lists;
use patina_core::player::{self, Control, PlayRequest, QueueCommand};
use patina_core::state::{NowPlaying, QueueView, ServerActivity, SessionView, Stage};
use patina_core::{AppState, CoreEvent, session};
use patina_plex::{HomeUser, Metadata, MetadataOptions, Params, PlexServer, stream_type};
use tokio::sync::mpsc::UnboundedSender;

use super::column::{Column, SortKey};
use super::modal::*;
use crate::fetch::{continue_watching, recently_added, search, watchlist_items};
use crate::item::{Item, Kind, Origin, resolve_key};
use crate::search::Fuzzy;

pub const CONTINUE: &str = "@continue";
pub const RECENT: &str = "@recent";
pub const SESSIONS: &str = "@sessions";
pub const HISTORY: &str = "@history";
pub const WATCHLIST: &str = "@watchlist";
pub const PLAYLISTS: &str = "/playlists";

const PAGE_SIZE: usize = 500;
const MAX_QUEUE: usize = 200;
const DETAIL_DELAY: Duration = Duration::from_millis(150);
const SEARCH_DELAY: Duration = Duration::from_millis(250);
const SESSIONS_REFRESH: Duration = Duration::from_secs(5);
const SEEK_SECONDS: f64 = 10.0;
/// Revisions arrive in bursts (stop, scrobble, notifications); reload once.
const REVISION_DELAY: Duration = Duration::from_millis(200);

// ---- messages from background tasks --------------------------------------

pub enum Msg {
    Listing {
        id: u64,
        generation: u64,
        items: Vec<Item>,
        append: bool,
        total: Option<usize>,
        done: bool,
    },
    ListingFailed {
        id: u64,
        generation: u64,
        error: String,
    },
    SectionCount(String, usize),
    Detail(String, Box<Item>),
    DetailFailed(String),
    SearchResults {
        query: String,
        items: Vec<Item>,
    },
    SearchFailed(String, String),
    OpDone {
        result: Result<Option<String>, String>,
        refresh: Refresh,
    },
    PlaylistRows(Result<Vec<PlaylistRow>, String>),
    PlaylistToggled {
        playlist_key: String,
        result: Result<Option<i64>, String>,
    },
    PlayOptionsItem(Result<Box<Item>, String>),
    /// GUIDs on the account's watchlist.
    Watchlisted(HashSet<String>),
    CollectionRows(Result<Vec<CollectionRow>, String>),
    CollectionToggled {
        collection_key: String,
        result: Result<bool, String>,
    },
    PickRows(Result<Vec<(PickEntry, String)>, String>),
    EditItem(Result<Box<Item>, String>),
    /// A plex.tv link code to show on the sign-in screen, or why there is none.
    Pin(Result<(String, String), String>),
    Core(CoreEvent),
}

/// What to reload after a mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refresh {
    Nothing,
    /// Watch state or ratings changed: every view refetches (via a revision).
    WatchState,
    /// Playlist contents or the playlist list changed.
    Playlists,
    /// Reload the columns showing these paths.
    Paths(Vec<String>),
    /// One item's metadata changed: refetch its details and small listings.
    Item(String),
    /// An item was deleted from the server.
    Removed(String),
    /// Watchlist membership changed.
    Watchlist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Success,
    Error,
}

pub struct Notice {
    pub text: String,
    pub level: Level,
    pub until: Option<Instant>,
}

pub struct SearchState {
    pub input: String,
    pub results: Vec<Item>,
    pub cursor: usize,
    pub scroll: usize,
    pub pending: bool,
    pub dirty_at: Option<Instant>,
    pub error: Option<String>,
}

/// The sign-in screens (everything before [`Stage::Ready`]).
#[derive(Default)]
pub struct Login {
    /// plex.tv/link code and the page to enter it on.
    pub code: Option<(String, String)>,
    pub requesting: bool,
    pub error: Option<String>,
    /// Cursor on the server list.
    pub cursor: usize,
}

pub struct App {
    pub core: AppState,
    tx: UnboundedSender<Msg>,
    pub stage: Stage,
    pub login: Login,
    pub session: Option<SessionView>,
    pub tui: TuiSettings,

    pub columns: Vec<Column>,
    next_id: u64,
    pub sections: Vec<Item>,
    /// Item counts per library section.
    pub counts: HashMap<String, usize>,
    fuzzy: Fuzzy,

    pub details: HashMap<String, Item>,
    detail_due: Option<(String, Instant)>,
    detail_inflight: HashSet<String>,
    revision_due: Option<Instant>,

    pub modal: Option<Modal>,
    pub search: Option<SearchState>,
    pub notice: Option<Notice>,
    pub pending: usize,
    pub now_playing: Option<NowPlaying>,
    /// Tracks to append once the play queue of `.0` (a rating key) is running.
    queue_after: Option<(String, Vec<Metadata>)>,
    pub downloads: Vec<Download>,

    pub show_inspector: bool,
    pub inspector_scroll: u16,
    last_selected: Option<String>,
    pub list_height: usize,
    pub spinner: usize,
    last_sessions_refresh: Instant,
    pub watchlist: HashSet<String>,
    pub activities: Vec<ServerActivity>,
    pub quit: bool,
}

fn detail_kind(item: &Item) -> bool {
    item.rating_key.is_some()
        && matches!(
            item.kind(),
            Kind::Movie
                | Kind::Show
                | Kind::Season
                | Kind::Episode
                | Kind::Artist
                | Kind::Album
                | Kind::Track
                | Kind::Clip
                | Kind::Collection
        )
}

fn is_paged(path: &str) -> bool {
    (path.starts_with("/library/sections/") && path != "/library/sections")
        || path.starts_with("/library/collections/")
        || (path.starts_with("/playlists/") && path.ends_with("/items"))
}

fn err(e: impl std::fmt::Display) -> String {
    format!("{e:#}")
}

impl App {
    pub fn new(core: AppState, tx: UnboundedSender<Msg>) -> Self {
        let tui = core.settings().tui;
        Self {
            show_inspector: tui.show_inspector,
            stage: core.snapshot().stage,
            core,
            tx,
            login: Login::default(),
            session: None,
            tui,
            columns: Vec::new(),
            next_id: 1,
            sections: Vec::new(),
            counts: HashMap::new(),
            fuzzy: Fuzzy::default(),
            details: HashMap::new(),
            detail_due: None,
            detail_inflight: HashSet::new(),
            revision_due: None,
            modal: None,
            search: None,
            notice: None,
            pending: 0,
            now_playing: None,
            queue_after: None,
            downloads: downloads::list(),
            inspector_scroll: 0,
            last_selected: None,
            list_height: 20,
            spinner: 0,
            last_sessions_refresh: Instant::now(),
            watchlist: HashSet::new(),
            activities: Vec::new(),
            quit: false,
        }
    }

    pub fn ready(&self) -> bool {
        matches!(self.stage, Stage::Ready) && !self.columns.is_empty()
    }

    pub fn server_name(&self) -> &str {
        self.session.as_ref().map(|s| s.server_name.as_str()).filter(|n| !n.is_empty()).unwrap_or("Plex")
    }

    pub fn username(&self) -> Option<&str> {
        self.session.as_ref()?.user.as_ref().map(|u| u.name.as_str())
    }

    fn server(&self) -> Result<PlexServer, String> {
        self.core.server().map_err(err)
    }

    fn new_column(&mut self, title: impl Into<String>, path: Option<String>) -> Column {
        let id = self.next_id;
        self.next_id += 1;
        let mut c = Column::new(id, title, path);
        c.hide_watched = self.tui.hide_watched;
        c
    }

    /// (Re)build the browser for the connected server.
    fn start(&mut self) {
        self.columns.clear();
        self.details.clear();
        self.detail_inflight.clear();
        self.search = None;
        self.modal = None;
        self.last_selected = None;
        let mut root = self.new_column(self.server_name().to_string(), None);
        root.items = self.root_items();
        root.loaded = true;
        root.rebuild(&mut self.fuzzy);
        self.columns.push(root);
        self.load_counts();
        self.load_watchlist_guids();
        self.after_selection_change();
    }

    fn load_watchlist_guids(&self) {
        let (core, tx) = (self.core.clone(), self.tx.clone());
        if core.tv().is_none() {
            return;
        }
        tokio::spawn(async move {
            match lists::watchlist(&core).await {
                Ok(items) => {
                    let guids = items.into_iter().filter_map(|i| i.guid).collect();
                    let _ = tx.send(Msg::Watchlisted(guids));
                }
                Err(e) => tracing::warn!("could not load the watchlist: {e:#}"),
            }
        });
    }

    /// Library sizes for the root column, one cheap request per section.
    fn load_counts(&self) {
        if !self.tui.show_library_counts {
            return;
        }
        let Ok(server) = self.core.server() else { return };
        for id in self.sections.iter().filter_map(|s| s.key.clone()) {
            let (server, tx) = (server.clone(), self.tx.clone());
            tokio::spawn(async move {
                let p = Params::new().set("X-Plex-Container-Start", 0).set("X-Plex-Container-Size", 0);
                if let Ok(mc) = server.fetch(&format!("/library/sections/{id}/all"), &p).await
                    && let Some(n) = mc.total_size.or(mc.size)
                {
                    let _ = tx.send(Msg::SectionCount(id, n.max(0) as usize));
                }
            });
        }
    }

    fn root_items(&self) -> Vec<Item> {
        let mut items = vec![
            Item::nav("Continue Watching", CONTINUE),
            Item::nav("Recently Added", RECENT),
            Item::nav("Watchlist", WATCHLIST),
            Item::nav("Playlists", PLAYLISTS),
            Item::nav("Now Playing", SESSIONS),
            Item::nav("History", HISTORY),
        ];
        items.extend(self.sections.iter().cloned());
        items
    }

    // ---- notices ---------------------------------------------------------

    pub fn info(&mut self, text: impl Into<String>) {
        self.notice = Some(Notice { text: text.into(), level: Level::Info, until: Some(Instant::now() + Duration::from_secs(4)) });
    }

    pub fn success(&mut self, text: impl Into<String>) {
        self.notice = Some(Notice { text: text.into(), level: Level::Success, until: Some(Instant::now() + Duration::from_secs(4)) });
    }

    pub fn error(&mut self, text: impl Into<String>) {
        self.notice = Some(Notice { text: text.into(), level: Level::Error, until: Some(Instant::now() + Duration::from_secs(8)) });
    }

    // ---- sign-in ----------------------------------------------------------

    fn on_stage(&mut self, stage: Stage) {
        match &stage {
            Stage::Login => {
                self.columns.clear();
                self.login = Login::default();
                self.request_pin();
            }
            Stage::PickServer { .. } => self.login.cursor = 0,
            Stage::Ready => {
                self.stage = stage;
                self.start();
                return;
            }
            _ => {}
        }
        self.stage = stage;
    }

    /// Ask plex.tv for a link code and wait for it to be claimed.
    fn request_pin(&mut self) {
        self.login.requesting = true;
        self.login.error = None;
        let (core, tx) = (self.core.clone(), self.tx.clone());
        tokio::spawn(async move {
            match session::create_pin(&core, false).await {
                Ok(pin) => {
                    let _ = tx.send(Msg::Pin(Ok((pin.code.clone(), pin.url.clone()))));
                    if let Err(e) = session::complete_pin(&core, pin.id, pin.attempt).await {
                        let _ = tx.send(Msg::Pin(Err(err(e))));
                    }
                }
                Err(e) => {
                    let _ = tx.send(Msg::Pin(Err(err(e))));
                }
            }
        });
    }

    fn sign_in_manual(&mut self, url: String, token: String) {
        session::cancel_pin(&self.core);
        self.login.code = None;
        self.login.error = None;
        self.login.requesting = true;
        let (core, tx) = (self.core.clone(), self.tx.clone());
        tokio::spawn(async move {
            let result = session::sign_in_manual(&core, &url, &token).await;
            if let Err(e) = result {
                let _ = tx.send(Msg::Pin(Err(format!("Could not connect: {}", err(e)))));
            }
        });
    }

    fn stage_key(&mut self, key: KeyEvent) {
        let core = self.core.clone();
        match (&self.stage, key.code) {
            (_, KeyCode::Char('q')) => self.quit = true,
            (Stage::Login, KeyCode::Char('r')) => {
                session::cancel_pin(&core);
                self.request_pin();
            }
            (Stage::Login, KeyCode::Char('m')) => {
                self.modal = Some(Modal::Input(Input {
                    title: "Connect to a server".into(),
                    hint: "Server URL (e.g. 192.168.1.10:32400)".into(),
                    value: String::new(),
                    purpose: InputPurpose::ServerUrl,
                }))
            }
            (Stage::Login, KeyCode::Char('o')) => {
                if let Some((_, url)) = &self.login.code
                    && let Err(e) = open::that_detached(url)
                {
                    self.login.error = Some(format!("Couldn't open a browser: {e}"));
                }
            }
            (Stage::Failed { .. }, KeyCode::Char('r') | KeyCode::Enter) => {
                tokio::spawn(async move { session::retry(&core).await });
            }
            (Stage::Failed { .. }, KeyCode::Char('L')) => session::sign_out(&core),
            (Stage::PickServer { servers, .. }, code) => {
                let n = servers.len();
                match code {
                    KeyCode::Char('j') | KeyCode::Down => self.login.cursor = (self.login.cursor + 1).min(n.saturating_sub(1)),
                    KeyCode::Char('k') | KeyCode::Up => self.login.cursor = self.login.cursor.saturating_sub(1),
                    KeyCode::Enter => {
                        if let Some(s) = servers.get(self.login.cursor) {
                            let id = s.id.clone();
                            tokio::spawn(async move { session::run(&core, session::pick_server(&core, &id)).await });
                        }
                    }
                    KeyCode::Char('L') | KeyCode::Esc => session::sign_out(&core),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    // ---- loading ---------------------------------------------------------

    /// Start (or restart) loading a column's listing.
    fn load(&mut self, idx: usize) {
        let Some(col) = self.columns.get_mut(idx) else { return };
        let Some(path) = col.path.clone() else { return };
        col.generation += 1;
        col.loading = true;
        col.error = None;
        let (id, generation) = (col.id, col.generation);
        let progressive = col.items.is_empty();
        self.spawn_listing(id, generation, path, progressive);
    }

    fn spawn_listing(&self, id: u64, generation: u64, path: String, progressive: bool) {
        let (core, tx) = (self.core.clone(), self.tx.clone());
        tokio::spawn(async move {
            let fail = |error: String| Msg::ListingFailed { id, generation, error };
            let server = match core.server() {
                Ok(s) => s,
                Err(e) => {
                    let _ = tx.send(fail(err(e)));
                    return;
                }
            };
            let result: anyhow::Result<Vec<Item>> = match path.as_str() {
                CONTINUE => continue_watching(&server).await,
                RECENT => recently_added(&server).await,
                SESSIONS => server.sessions().await.map(|v| v.into_iter().map(Item::from).collect()).map_err(Into::into),
                HISTORY => lists::history(&core, 0).await.map(|h| h.items.into_iter().map(Item::from).collect()),
                WATCHLIST => watchlist_items(&core, &server).await,
                p if is_paged(p) => {
                    let mut all: Vec<Item> = Vec::new();
                    let mut start = 0;
                    loop {
                        let params = Params::new()
                            .set("X-Plex-Container-Start", start)
                            .set("X-Plex-Container-Size", PAGE_SIZE)
                            .set("includeGuids", 1);
                        let mc = match server.fetch(p, &params).await {
                            Ok(mc) => mc,
                            Err(e) => {
                                let _ = tx.send(fail(err(e)));
                                return;
                            }
                        };
                        let total = mc.total_size.or(mc.size).map(|n| n.max(0) as usize);
                        let items = Item::listing(mc);
                        let n = items.len();
                        start += n;
                        let finished = n == 0 || total.map_or(n < PAGE_SIZE, |t| start >= t);
                        if progressive && !finished {
                            let _ = tx.send(Msg::Listing { id, generation, items: items.clone(), append: start > n, total, done: false });
                        }
                        all.extend(items);
                        if finished {
                            break;
                        }
                    }
                    Ok(all)
                }
                p => server.fetch(p, &Params::new()).await.map(Item::listing).map_err(Into::into),
            };
            let _ = tx.send(match result {
                Ok(items) => Msg::Listing { id, generation, items, append: false, total: None, done: true },
                Err(e) => fail(err(e)),
            });
        });
    }

    fn column_index(&self, id: u64) -> Option<usize> {
        self.columns.iter().position(|c| c.id == id)
    }

    fn open_column(&mut self, title: String, path: String) {
        let col = self.new_column(title, Some(path));
        self.columns.push(col);
        let idx = self.columns.len() - 1;
        self.load(idx);
    }

    fn drill(&mut self) {
        let Some(col) = self.columns.last() else { return };
        let Some(item) = col.selected().cloned() else { return };
        if item.kind() == Kind::Remote {
            self.info(format!("{} isn't on this server", item.title()));
            return;
        }
        let Some(key) = item.children_key() else { return };
        if item.is_playable() {
            return;
        }
        let path = match &col.path {
            Some(parent) if item.origin != Origin::Nav => resolve_key(parent, &key),
            _ => key,
        };
        let title = match item.kind() {
            Kind::Season | Kind::Album => item.full_title(),
            _ => item.title().to_string(),
        };
        self.open_column(title, path);
    }

    fn back(&mut self) {
        if self.columns.len() > 1 {
            self.columns.pop();
            self.inspector_scroll = 0;
        }
    }

    // ---- selection / details ---------------------------------------------

    pub fn selected(&self) -> Option<&Item> {
        if let Some(s) = &self.search {
            return s.results.get(s.cursor);
        }
        self.columns.last()?.selected()
    }

    /// The selected item with full metadata when it has been fetched.
    pub fn selected_detail(&self) -> Option<&Item> {
        let item = self.selected()?;
        item.rating_key
            .as_ref()
            .and_then(|k| self.details.get(k))
            .filter(|_| item.kind() != Kind::Playlist)
            .or(Some(item))
    }

    fn after_selection_change(&mut self) {
        let identity = self.selected().map(Column::identity);
        if identity != self.last_selected {
            self.last_selected = identity;
            self.inspector_scroll = 0;
            if let Some(item) = self.selected()
                && detail_kind(item)
            {
                let key = item.rating_key.clone().unwrap_or_default();
                if !self.details.contains_key(&key) && !self.detail_inflight.contains(&key) {
                    self.detail_due = Some((key, Instant::now() + DETAIL_DELAY));
                }
            }
        }
    }

    fn fetch_detail(&mut self, key: String) {
        self.detail_inflight.insert(key.clone());
        let (core, tx) = (self.core.clone(), self.tx.clone());
        tokio::spawn(async move {
            let opts = MetadataOptions { extras: true, markers: true, chapters: true, ..Default::default() };
            let result = match core.server() {
                Ok(server) => server.metadata(&key, opts).await.ok().flatten(),
                Err(_) => None,
            };
            let _ = tx.send(match result {
                Some(m) => Msg::Detail(key, Box::new(Item::from(m))),
                None => Msg::DetailFailed(key),
            });
        });
    }

    // ---- ticks -----------------------------------------------------------

    pub fn on_tick(&mut self) {
        self.spinner = self.spinner.wrapping_add(1);
        let now = Instant::now();
        if let Some(n) = &self.notice
            && n.until.is_some_and(|u| now >= u)
        {
            self.notice = None;
        }
        if self.revision_due.is_some_and(|d| now >= d) {
            self.on_revision();
        }
        if let Some((key, due)) = self.detail_due.clone()
            && now >= due
        {
            self.detail_due = None;
            if self.selected().and_then(|i| i.rating_key.as_deref()) == Some(key.as_str()) {
                self.fetch_detail(key);
            }
        }
        if let Some(s) = &mut self.search
            && s.dirty_at.is_some_and(|d| now.duration_since(d) >= SEARCH_DELAY)
        {
            s.dirty_at = None;
            let query = s.input.trim().to_string();
            if query.is_empty() {
                s.results.clear();
                s.pending = false;
            } else {
                s.pending = true;
                let (core, tx) = (self.core.clone(), self.tx.clone());
                tokio::spawn(async move {
                    let result = match core.server() {
                        Ok(server) => search(&server, &query, 30).await.map_err(err),
                        Err(e) => Err(err(e)),
                    };
                    let _ = tx.send(match result {
                        Ok(items) => Msg::SearchResults { query, items },
                        Err(e) => Msg::SearchFailed(query, e),
                    });
                });
            }
        }
        // Live notifications refresh Now Playing; poll too in case they are unavailable.
        if self.last_sessions_refresh.elapsed() >= SESSIONS_REFRESH && self.columns.last().is_some_and(|c| c.is_sessions() && !c.loading) {
            self.last_sessions_refresh = Instant::now();
            let idx = self.columns.len() - 1;
            self.load(idx);
        }
    }

    pub fn is_busy(&self) -> bool {
        self.pending > 0 || self.columns.iter().any(|c| c.loading)
    }

    // ---- messages --------------------------------------------------------

    pub fn on_msg(&mut self, msg: Msg) {
        match msg {
            Msg::Core(ev) => self.on_core(ev),
            Msg::Pin(result) => {
                self.login.requesting = false;
                match result {
                    Ok(code) => self.login.code = Some(code),
                    Err(e) => {
                        self.login.code = None;
                        self.login.error = Some(e);
                    }
                }
            }
            Msg::Listing { id, generation, items, append, total, done } => self.on_listing(id, generation, items, append, total, done),
            Msg::ListingFailed { id, generation, error } => {
                let Some(idx) = self.column_index(id) else { return };
                let col = &mut self.columns[idx];
                if col.generation != generation {
                    return;
                }
                col.loading = false;
                col.error = Some(error.clone());
                let title = col.title.clone();
                self.error(format!("Loading {title}: {error}"));
            }
            Msg::SectionCount(id, n) => {
                self.counts.insert(id, n);
            }
            Msg::Detail(key, item) => {
                self.detail_inflight.remove(&key);
                self.details.insert(key, *item);
            }
            Msg::DetailFailed(key) => {
                self.detail_inflight.remove(&key);
            }
            Msg::SearchResults { query, items } => {
                if let Some(s) = &mut self.search
                    && s.input.trim() == query
                {
                    s.results = items;
                    s.cursor = 0;
                    s.scroll = 0;
                    s.pending = false;
                    s.error = None;
                }
                self.after_selection_change();
            }
            Msg::SearchFailed(query, e) => {
                if let Some(s) = &mut self.search
                    && s.input.trim() == query
                {
                    s.pending = false;
                    s.error = Some(e);
                }
            }
            Msg::OpDone { result, refresh } => {
                self.pending = self.pending.saturating_sub(1);
                match result {
                    Ok(Some(text)) => self.success(text),
                    Ok(None) => {}
                    Err(e) => self.error(e),
                }
                self.apply_refresh(refresh);
            }
            Msg::PlaylistRows(result) => {
                if let Some(Modal::Playlists(p)) = &mut self.modal {
                    p.loading = false;
                    match result {
                        Ok(rows) => p.rows = rows,
                        Err(e) => p.error = Some(e),
                    }
                }
            }
            Msg::PlaylistToggled { playlist_key, result } => {
                let mut failed = None;
                if let Some(Modal::Playlists(p)) = &mut self.modal
                    && let Some(row) = p.rows.iter_mut().find(|r| r.playlist.rating_key.as_deref() == Some(playlist_key.as_str()))
                {
                    row.busy = false;
                    match result {
                        Ok(entry) => row.entry_id = entry,
                        Err(e) => failed = Some(e),
                    }
                }
                if let Some(e) = failed {
                    self.error(e);
                }
                self.apply_refresh(Refresh::Playlists);
            }
            Msg::PlayOptionsItem(result) => match result {
                Ok(item) => {
                    if let Some(Modal::PlayOptions(m)) = &mut self.modal {
                        m.item = Some(*item);
                        build_option_rows(m);
                    }
                }
                Err(e) => {
                    self.modal = None;
                    self.error(e);
                }
            },
            Msg::Watchlisted(guids) => self.watchlist = guids,
            Msg::CollectionRows(result) => {
                if let Some(Modal::Collections(c)) = &mut self.modal {
                    c.loading = false;
                    match result {
                        Ok(rows) => c.rows = rows,
                        Err(e) => c.error = Some(e),
                    }
                }
            }
            Msg::CollectionToggled { collection_key, result } => {
                let mut failed = None;
                let mut item_key = None;
                if let Some(Modal::Collections(c)) = &mut self.modal {
                    item_key = c.item.rating_key.clone();
                    if let Some(row) = c.rows.iter_mut().find(|r| r.collection.rating_key.as_deref() == Some(collection_key.as_str())) {
                        row.busy = false;
                        match result {
                            Ok(member) => row.member = member,
                            Err(e) => failed = Some(e),
                        }
                    }
                }
                if let Some(e) = failed {
                    self.error(e);
                }
                if let Some(k) = item_key {
                    self.details.remove(&k);
                }
                let path = format!("/library/collections/{collection_key}/children");
                self.apply_refresh(Refresh::Paths(vec![path]));
            }
            Msg::PickRows(result) => {
                if let Some(Modal::Pick(p)) = &mut self.modal {
                    p.loading = false;
                    match result {
                        Ok(rows) if rows.is_empty() => p.error = Some("Nothing found".into()),
                        Ok(rows) => p.rows = rows,
                        Err(e) => p.error = Some(e),
                    }
                }
            }
            Msg::EditItem(result) => match result {
                Ok(item) => {
                    if let Some(k) = item.rating_key.clone() {
                        self.details.insert(k, (*item).clone());
                    }
                    self.modal = Some(Modal::Edit(EditModal { item: *item, cursor: 0 }));
                }
                Err(e) => self.error(e),
            },
        }
    }

    fn on_core(&mut self, ev: CoreEvent) {
        match ev {
            CoreEvent::Stage(stage) => self.on_stage(stage),
            CoreEvent::Session { session, sections } => {
                self.session = session;
                self.sections = sections.into_iter().map(Item::section).collect();
                self.counts.retain(|k, _| self.sections.iter().any(|s| s.key.as_deref() == Some(k)));
                let root_items = self.root_items();
                let name = self.server_name().to_string();
                if let Some(root) = self.columns.first_mut() {
                    root.title = name;
                    root.set_items(root_items, &mut self.fuzzy);
                    self.load_counts();
                }
            }
            CoreEvent::NowPlaying(np) => self.now_playing = np.map(|b| *b),
            CoreEvent::Queue(q) => self.on_queue(q),
            CoreEvent::Activities(list) => self.activities = list,
            CoreEvent::Revision(_) => {
                self.revision_due.get_or_insert_with(|| Instant::now() + REVISION_DELAY);
            }
            CoreEvent::Toast(t) if t.error => self.error(t.text),
            CoreEvent::Toast(t) => self.info(t.text),
            CoreEvent::Downloads(list) => self.downloads = list,
            CoreEvent::SessionsChanged => {
                for idx in 1..self.columns.len() {
                    if self.columns[idx].is_sessions() && !self.columns[idx].loading {
                        self.last_sessions_refresh = Instant::now();
                        self.load(idx);
                    }
                }
            }
        }
    }

    /// Watch state may have changed: refetch the open listings and the details shown.
    fn on_revision(&mut self) {
        self.revision_due = None;
        if !self.ready() {
            return;
        }
        let keep = self.selected().and_then(|i| i.rating_key.clone());
        self.details.clear();
        self.detail_inflight.clear();
        for idx in 1..self.columns.len() {
            self.load(idx);
        }
        if let Some(k) = keep.filter(|_| self.selected().is_some_and(detail_kind)) {
            self.fetch_detail(k);
        }
    }

    /// Once playback of a track is running, queue the rest of its column.
    fn on_queue(&mut self, q: Option<QueueView>) {
        let Some(q) = q else { return };
        let current = q.items.iter().find(|i| Some(i.uid) == q.current).map(|i| i.item.rating_key().to_string());
        if self.queue_after.as_ref().is_some_and(|(first, _)| current.as_deref() == Some(first.as_str()))
            && let Some((_, rest)) = self.queue_after.take()
        {
            for item in rest {
                player::queue(self.core.clone(), QueueCommand::Add { item: Box::new(item), next: false });
            }
        }
    }

    fn on_listing(&mut self, id: u64, generation: u64, items: Vec<Item>, append: bool, total: Option<usize>, done: bool) {
        let Some(idx) = self.column_index(id) else { return };
        let col = &mut self.columns[idx];
        if col.generation != generation {
            return;
        }
        if done && col.path.as_deref() == Some(WATCHLIST) {
            self.watchlist = items.iter().filter_map(|i| i.guid.clone()).collect();
        }
        if append {
            col.append(items, &mut self.fuzzy);
        } else {
            col.set_items(items, &mut self.fuzzy);
        }
        col.total = total.or(col.total);
        col.loaded = true;
        if done {
            col.loading = false;
            col.total = None;
            col.error = None;
        }
        if idx + 1 == self.columns.len() {
            self.after_selection_change();
        }
    }

    fn apply_refresh(&mut self, refresh: Refresh) {
        match refresh {
            Refresh::Nothing => {}
            Refresh::WatchState => self.core.bump(),
            Refresh::Playlists => {
                let paths: Vec<String> = self.columns.iter().filter_map(|c| c.path.clone()).filter(|p| p.starts_with(PLAYLISTS)).collect();
                self.apply_refresh(Refresh::Paths(paths));
            }
            Refresh::Paths(paths) => {
                for idx in 0..self.columns.len() {
                    if self.columns[idx].path.as_ref().is_some_and(|p| paths.contains(p)) {
                        self.load(idx);
                    }
                }
            }
            Refresh::Item(key) => {
                self.details.remove(&key);
                self.fetch_detail(key.clone());
                for idx in 1..self.columns.len() {
                    let col = &self.columns[idx];
                    let shows_it = col.items.iter().any(|i| i.rating_key.as_deref() == Some(key.as_str()));
                    if shows_it && col.path.as_deref().is_some_and(|p| !is_paged(p)) {
                        self.load(idx);
                    }
                }
            }
            Refresh::Removed(key) => {
                self.details.remove(&key);
                for col in &mut self.columns {
                    let before = col.items.len();
                    col.items.retain(|i| i.rating_key.as_deref() != Some(key.as_str()));
                    if col.items.len() != before {
                        col.rebuild(&mut self.fuzzy);
                    }
                }
            }
            Refresh::Watchlist => {
                self.load_watchlist_guids();
                self.apply_refresh(Refresh::Paths(vec![WATCHLIST.into()]));
            }
        }
    }

    /// Apply a change to every copy of an item (columns, search, details).
    fn patch(&mut self, rating_key: &str, f: &mut dyn FnMut(&mut Item)) {
        for col in &mut self.columns {
            if col.patch(rating_key, f) {
                col.rebuild(&mut self.fuzzy);
            }
        }
        if let Some(s) = &mut self.search {
            for i in s.results.iter_mut().filter(|i| i.rating_key.as_deref() == Some(rating_key)) {
                f(i);
            }
        }
        if let Some(d) = self.details.get_mut(rating_key) {
            f(d);
        }
    }

    // ---- background operations ------------------------------------------

    fn spawn_op<F>(&mut self, refresh: Refresh, fut: F)
    where
        F: std::future::Future<Output = Result<Option<String>, String>> + Send + 'static,
    {
        self.pending += 1;
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = fut.await;
            let _ = tx.send(Msg::OpDone { result, refresh });
        });
    }

    /// Run a server call in the background, reporting `done` on success.
    fn server_op<F, Fut>(&mut self, refresh: Refresh, done: impl Into<String>, f: F)
    where
        F: FnOnce(PlexServer) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = patina_plex::Result<()>> + Send,
    {
        let server = self.server();
        let done = done.into();
        self.spawn_op(refresh, async move {
            f(server?).await.map_err(err)?;
            Ok(Some(done).filter(|d| !d.is_empty()))
        });
    }

    fn set_watched(&mut self, watched: bool) {
        let Some(item) = self.selected().cloned() else { return };
        if !item.is_watchable() {
            self.info("That can't be marked watched");
            return;
        }
        let key = item.rating_key.clone().unwrap_or_default();
        self.patch(&key, &mut |i| i.set_watched(watched));
        let title = item.title().to_string();
        let done = format!("{title} marked {}", if watched { "watched" } else { "unwatched" });
        self.server_op(Refresh::WatchState, done, move |s| async move {
            if watched { s.scrobble(&key).await } else { s.unscrobble(&key).await }
        });
    }

    fn submit_rating(&mut self, key: String, value: &str) {
        let rating = if value.trim().is_empty() {
            None
        } else {
            match value.trim().parse::<f64>() {
                Ok(v) if (0.0..=10.0).contains(&v) => Some(v),
                _ => {
                    self.error("Rating must be a number from 0 to 10");
                    return;
                }
            }
        };
        self.patch(&key, &mut |i| i.user_rating = rating);
        let done = rating.map_or("Rating cleared".into(), |r| format!("Rated {r}/10"));
        self.server_op(Refresh::Nothing, done, move |s| async move { s.rate(&key, rating).await });
    }

    // ---- playback ----------------------------------------------------------

    /// Hand a request to the core player. Shows and seasons are fetched with
    /// their on-deck episode first, so playback continues where you left off.
    fn start_play(&mut self, req: PlayRequest, queue_after: Vec<Metadata>) {
        self.info(format!("Starting {}…", Item::from(req.item.clone()).full_title()));
        self.queue_after = (!queue_after.is_empty()).then(|| (req.item.rating_key().to_string(), queue_after));
        let core = self.core.clone();
        self.spawn_op(Refresh::Nothing, async move {
            let mut req = req;
            if matches!(req.item.kind(), "show" | "season") && !req.shuffle && req.item.on_deck.is_none() {
                let opts = MetadataOptions { on_deck: true, ..Default::default() };
                let server = core.server().map_err(err)?;
                if let Some(full) = server.metadata(req.item.rating_key(), opts).await.map_err(err)? {
                    req.item = full;
                }
            }
            player::play(core, req);
            Ok(None)
        });
    }

    /// The tracks after the cursor, to queue behind the selected one.
    fn tracks_after_cursor(&self) -> Vec<Metadata> {
        match self.columns.last() {
            Some(col) if self.search.is_none() => {
                col.queue_from_cursor().into_iter().skip(1).filter(|i| i.kind() == Kind::Track).take(MAX_QUEUE).map(|i| i.meta).collect()
            }
            _ => Vec::new(),
        }
    }

    fn play_selected(&mut self, from_start: bool) {
        let Some(item) = self.selected().cloned() else { return };
        if !item.is_playable() {
            return self.play_container(item, false);
        }
        let after = if item.kind() == Kind::Track { self.tracks_after_cursor() } else { Vec::new() };
        self.start_play(PlayRequest { item: item.meta, resume: !from_start, ..Default::default() }, after);
    }

    /// Play every leaf of a container (from next-up for shows), optionally
    /// shuffled; on a playable item, play from there (shuffling its parent).
    fn play_container(&mut self, item: Item, shuffle: bool) {
        if item.is_playable() {
            if !shuffle {
                return self.play_selected(false);
            }
            let parent = match item.kind() {
                Kind::Episode => item.parent_rating_key.clone().map(|k| (k, "season")),
                Kind::Track => item.parent_rating_key.clone().map(|k| (k, "album")),
                _ => None,
            };
            let item = match parent {
                Some((rk, kind)) => Metadata { rating_key: Some(rk), kind: Some(kind.into()), ..Default::default() },
                None => item.meta,
            };
            return self.start_play(PlayRequest { item, shuffle: true, ..Default::default() }, Vec::new());
        }
        if item.rating_key.is_none() || !matches!(item.kind(), Kind::Show | Kind::Season | Kind::Artist | Kind::Album | Kind::Playlist | Kind::Collection | Kind::PhotoAlbum) {
            self.info("Nothing to play here");
            return;
        }
        self.start_play(PlayRequest { item: item.meta, shuffle, ..Default::default() }, Vec::new());
    }

    fn open_play_options(&mut self) {
        let Some(item) = self.selected().cloned() else { return };
        if !item.is_playable() {
            self.info("Play options are for a single movie, episode or track");
            return;
        }
        let Some(key) = item.rating_key.clone() else { return };
        let mut m = PlayOptionsModal {
            item: None,
            opts: PlayOptions::with_quality(self.core.settings().player.quality().id),
            cursor: 0,
            rows: Vec::new(),
        };
        build_option_rows(&mut m);
        self.modal = Some(Modal::PlayOptions(m));
        let (core, tx) = (self.core.clone(), self.tx.clone());
        tokio::spawn(async move {
            let r = async {
                let server = core.server().map_err(err)?;
                let m = server.metadata(&key, MetadataOptions::default()).await.map_err(err)?;
                m.map(|m| Box::new(Item::from(m))).ok_or_else(|| "Item not found".to_string())
            }
            .await;
            let _ = tx.send(Msg::PlayOptionsItem(r));
        });
    }

    /// Play with the dialog's choices. Track choices are saved to the server
    /// first: the player (and the transcoder) apply the server's selection.
    fn play_with_options(&mut self, m: PlayOptionsModal) {
        let Some(item) = m.item else { return };
        let opts = m.opts;
        let part = item.media.get(opts.media_index).and_then(|md| md.parts.first()).and_then(|p| p.id);
        let choose = opts.audio_stream_id.is_some() || opts.subtitle_stream_id.is_some();
        let req = PlayRequest {
            item: item.meta.clone(),
            resume: !opts.from_start,
            media_index: opts.media_index,
            quality: Some(QUALITIES[opts.quality.min(QUALITIES.len() - 1)].id.to_string()),
            ..Default::default()
        };
        let core = self.core.clone();
        self.info(format!("Starting {}…", item.full_title()));
        self.spawn_op(Refresh::Nothing, async move {
            if choose && let Some(part) = part {
                let server = core.server().map_err(err)?;
                if let Err(e) = server.select_streams(part, opts.audio_stream_id, opts.subtitle_stream_id, true).await {
                    tracing::warn!("failed to save stream selection: {e}");
                }
            }
            player::play(core, req);
            Ok(None)
        });
    }

    fn control(&mut self, cmd: Control) {
        if self.now_playing.is_none() {
            self.info("Nothing is playing");
            return;
        }
        player::control(cmd);
    }

    fn download_selected(&mut self) {
        let Some(item) = self.selected().cloned() else { return };
        if !(item.is_playable() || matches!(item.kind(), Kind::Show | Kind::Season | Kind::Album | Kind::Artist)) {
            self.info("Select something to download");
            return;
        }
        let Some(key) = item.rating_key.clone() else { return };
        let (core, title) = (self.core.clone(), item.full_title());
        self.spawn_op(Refresh::Nothing, async move {
            let n = downloads::enqueue(&core, &key).await.map_err(err)?;
            Ok(Some(match n {
                0 => format!("{title} is already downloaded"),
                1 => format!("Downloading {title} to {}", core.settings().player.downloads_dir().display()),
                n => format!("Downloading {n} items of {title}"),
            }))
        });
    }

    fn copy_url(&mut self) {
        let Some(item) = self.selected().cloned() else { return };
        let Some(key) = item.rating_key.clone().filter(|_| item.is_playable()) else {
            self.info("Select a movie, episode or track");
            return;
        };
        let server = self.server();
        self.spawn_op(Refresh::Nothing, async move {
            let server = server?;
            let full = server.metadata(&key, MetadataOptions::default()).await.map_err(err)?.ok_or("Item not found")?;
            let part = full.media.first().and_then(|m| m.parts.first()).and_then(|p| p.key.clone()).ok_or("no media file")?;
            let url = server.part_url(&part, false).map_err(err)?;
            copy_to_clipboard(url.as_str()).map_err(err)?;
            Ok(Some("Stream URL copied (it contains your token)".into()))
        });
    }

    fn open_web(&mut self) {
        let Some(key) = self.selected().and_then(|i| i.rating_key.clone()) else { return };
        let Some(machine) = self.session.as_ref().and_then(|s| s.machine_identifier.clone()) else {
            self.info("The server's identity is unknown");
            return;
        };
        match open::that_detached(web_url(&machine, &key)) {
            Ok(()) => self.success("Opened in Plex Web"),
            Err(e) => self.error(format!("Couldn't open a browser: {e}")),
        }
    }

    fn scan_library(&mut self, force: bool) {
        let section = self
            .selected()
            .filter(|i| i.kind() == Kind::Section)
            .and_then(|i| Some((i.key.clone()?, i.title().to_string())))
            .or_else(|| {
                let sid = self.columns.last()?.section_id()?.to_string();
                let title = self.sections.iter().find(|s| s.key.as_deref() == Some(&sid))?.title().to_string();
                Some((sid, title))
            });
        let Some((id, title)) = section else {
            self.info("Select a library (or open one) to scan it");
            return;
        };
        let done = format!("Scanning {title}{}", if force { " (full refresh)" } else { "" });
        self.server_op(Refresh::Nothing, done, move |s| async move { s.refresh_section(&id, force, None).await });
    }

    fn open_playlist_picker(&mut self) {
        let Some(item) = self.selected().cloned() else { return };
        if !(item.is_playable() || matches!(item.kind(), Kind::Show | Kind::Season | Kind::Album | Kind::Artist)) || item.rating_key.is_none() {
            self.info("Select something to add to a playlist");
            return;
        }
        let kind = item.playlist_kind();
        let key = item.rating_key.clone().unwrap_or_default();
        self.modal = Some(Modal::Playlists(PlaylistPicker { item, kind, rows: Vec::new(), cursor: 0, loading: true, error: None }));
        let (server, tx) = (self.server(), self.tx.clone());
        tokio::spawn(async move {
            let result = async {
                let server = server?;
                let playlists = server.playlists(Some(kind), None).await.map_err(err)?.into_iter().filter(|p| !p.smart);
                // Check membership concurrently; any failure makes the whole
                // view an error, so "unknown" is never shown as "absent".
                futures::stream::iter(playlists)
                    .map(|p| {
                        let (server, key) = (server.clone(), key.clone());
                        async move {
                            let entries = server.playlist_items(p.rating_key(), 0, 5000).await.map_err(err)?.metadata;
                            let entry_id = entries.iter().find(|e| e.rating_key.as_deref() == Some(key.as_str())).and_then(|e| e.playlist_item_id);
                            Ok::<_, String>(PlaylistRow { playlist: Item::from(p), entry_id, busy: false })
                        }
                    })
                    .buffered(4)
                    .collect::<Vec<_>>()
                    .await
                    .into_iter()
                    .collect::<Result<Vec<_>, _>>()
            }
            .await;
            let _ = tx.send(Msg::PlaylistRows(result));
        });
    }

    fn toggle_playlist_row(&mut self) {
        let Some(Modal::Playlists(p)) = &mut self.modal else { return };
        let item_key = p.item.rating_key.clone().unwrap_or_default();
        if p.cursor == p.rows.len() {
            let kind = p.kind;
            self.modal = Some(Modal::Input(Input {
                title: "New playlist".into(),
                hint: "Name".into(),
                value: String::new(),
                purpose: InputPurpose::NewPlaylist { rating_keys: vec![item_key], kind },
            }));
            return;
        }
        let Some(row) = p.rows.get_mut(p.cursor) else { return };
        if row.busy {
            return;
        }
        row.busy = true;
        let playlist_key = row.playlist.rating_key.clone().unwrap_or_default();
        let entry = row.entry_id;
        let (server, tx) = (self.server(), self.tx.clone());
        tokio::spawn(async move {
            let result = async {
                let server = server?;
                match entry {
                    Some(entry_id) => {
                        server.remove_from_playlist(&playlist_key, entry_id).await.map_err(err)?;
                        Ok(None)
                    }
                    None => {
                        server.add_to_playlist(&playlist_key, &server.item_uri(&[&item_key])).await.map_err(err)?;
                        let entries = server.playlist_items(&playlist_key, 0, 5000).await.map_err(err)?.metadata;
                        Ok(entries.iter().rev().find(|e| e.rating_key.as_deref() == Some(item_key.as_str())).and_then(|e| e.playlist_item_id).or(Some(0)))
                    }
                }
            }
            .await;
            let _ = tx.send(Msg::PlaylistToggled { playlist_key, result });
        });
    }

    fn submit_input(&mut self, input: Input) {
        let value = input.value.trim().to_string();
        match input.purpose {
            InputPurpose::Rate { rating_key } => self.submit_rating(rating_key, &value),
            InputPurpose::ServerUrl => {
                if value.is_empty() {
                    return;
                }
                self.modal = Some(Modal::Input(Input {
                    title: "Connect to a server".into(),
                    hint: "X-Plex-Token".into(),
                    value: String::new(),
                    purpose: InputPurpose::ServerToken { url: value },
                }));
            }
            InputPurpose::ServerToken { url } => {
                if !value.is_empty() {
                    self.sign_in_manual(url, value);
                }
            }
            InputPurpose::NewPlaylist { rating_keys, kind } => {
                if value.is_empty() {
                    return;
                }
                let server = self.server();
                self.spawn_op(Refresh::Playlists, async move {
                    let server = server?;
                    let keys: Vec<&str> = rating_keys.iter().map(String::as_str).collect();
                    let p = server.create_playlist(&value, kind, &server.item_uri(&keys), false).await.map_err(err)?;
                    Ok(Some(format!("Created playlist {}", p.as_ref().map_or(value.as_str(), |p| p.title()))))
                });
            }
            InputPurpose::NewCollection { rating_key } => {
                if value.is_empty() {
                    return;
                }
                let core = self.core.clone();
                self.spawn_op(Refresh::Item(rating_key.clone()), async move {
                    edit::add_to_collection(&core, &rating_key, None, Some(&value)).await.map_err(err)?;
                    Ok(Some(format!("Created collection {value}")))
                });
            }
            InputPurpose::SmartPlaylist { kind, filter_path } => {
                if value.is_empty() {
                    return;
                }
                let server = self.server();
                self.spawn_op(Refresh::Playlists, async move {
                    let server = server?;
                    server.create_playlist(&value, kind, &server.library_uri(&filter_path), true).await.map_err(err)?;
                    Ok(Some(format!("Created smart playlist {value}")))
                });
            }
            InputPurpose::EditField { item, field } => self.submit_edit(*item, field, value),
            InputPurpose::PosterUrl { rating_key } => {
                if !value.starts_with("http") {
                    self.error("Enter an http(s) image URL");
                    return;
                }
                self.server_op(Refresh::Item(rating_key.clone()), "Poster changed", move |s| async move { s.set_poster(&rating_key, &value).await });
            }
            InputPurpose::MatchQuery { rating_key, year } => self.search_matches(rating_key, value, year),
            InputPurpose::SubtitleLanguage { rating_key } => {
                let lang = if value.is_empty() { "en".to_string() } else { value };
                self.search_subtitles(rating_key, lang);
            }
            InputPurpose::UserPin { user } => self.switch_user(*user, Some(value)),
            InputPurpose::RenamePlaylist { playlist_id } => {
                if value.is_empty() {
                    return;
                }
                let done = format!("Renamed to {value}");
                self.server_op(Refresh::Playlists, done, move |s| async move { s.update_playlist(&playlist_id, Some(&value), None).await });
            }
        }
    }

    fn run_confirmed(&mut self, action: ConfirmAction) {
        match action {
            ConfirmAction::Logout => session::sign_out(&self.core),
            ConfirmAction::DeletePlaylist { playlist_id } => {
                self.server_op(Refresh::Playlists, "Playlist deleted", move |s| async move { s.delete_playlist(&playlist_id).await })
            }
            ConfirmAction::RemoveFromPlaylist { playlist_id, playlist_item_id } => self.server_op(Refresh::Playlists, "Removed from playlist", move |s| async move {
                s.remove_from_playlist(&playlist_id, playlist_item_id).await
            }),
            ConfirmAction::RemoveFromCollection { collection_id, rating_key } => {
                let path = format!("/library/collections/{collection_id}/children");
                self.server_op(Refresh::Paths(vec![path]), "Removed from collection", move |s| async move {
                    s.remove_from_collection(&collection_id, &rating_key).await
                })
            }
            ConfirmAction::TerminateSession { session_id } => self.server_op(Refresh::Paths(vec![SESSIONS.into()]), "Stream stopped", move |s| async move {
                s.terminate_session(&session_id, "Stopped by the server owner").await
            }),
            ConfirmAction::Unmatch { rating_key } => {
                self.server_op(Refresh::Item(rating_key.clone()), "Unmatched", move |s| async move { s.unmatch(&rating_key).await })
            }
            ConfirmAction::DeleteMedia { rating_key, title } => {
                let server = self.server();
                self.spawn_op(Refresh::Removed(rating_key.clone()), async move {
                    server?.delete_metadata(&rating_key).await.map_err(|e| match e.status() {
                        Some(401 | 403) => "The server doesn't allow media deletion (Settings → Library)".to_string(),
                        _ => err(e),
                    })?;
                    Ok(Some(format!("Deleted {title}")))
                });
            }
            ConfirmAction::EmptyTrash { section_id } => {
                self.server_op(Refresh::Nothing, "Trash emptied", move |s| async move { s.empty_trash(&section_id).await })
            }
        }
    }

    /// `x`: the contextual destructive action.
    fn delete_action(&mut self) {
        let Some(col) = self.columns.last() else { return };
        let Some(item) = col.selected().cloned() else { return };
        let action = if col.is_sessions() {
            item.session.as_ref().and_then(|s| s.id.clone()).map(|session_id| Confirm {
                message: format!(
                    "Stop {}'s stream of {}?",
                    item.user.as_ref().and_then(|u| u.title.clone()).unwrap_or_else(|| "this user".into()),
                    item.full_title()
                ),
                action: ConfirmAction::TerminateSession { session_id },
            })
        } else if let Some(pid) = col.is_playlist() {
            item.playlist_item_id.map(|entry| Confirm {
                message: format!("Remove {} from {}?", item.full_title(), col.title),
                action: ConfirmAction::RemoveFromPlaylist { playlist_id: pid.to_string(), playlist_item_id: entry },
            })
        } else if let Some(cid) = col.is_collection() {
            item.rating_key.clone().map(|rk| Confirm {
                message: format!("Remove {} from the {} collection?", item.title(), col.title),
                action: ConfirmAction::RemoveFromCollection { collection_id: cid.to_string(), rating_key: rk },
            })
        } else if item.kind() == Kind::Playlist {
            item.rating_key.clone().map(|id| Confirm {
                message: format!("Delete playlist {}?", item.title()),
                action: ConfirmAction::DeletePlaylist { playlist_id: id },
            })
        } else {
            None
        };
        match action {
            Some(c) => self.modal = Some(Modal::Confirm(c)),
            None => self.info("Nothing to delete here"),
        }
    }

    fn move_in_playlist(&mut self, down: bool) {
        let Some(col) = self.columns.last_mut() else { return };
        let Some(pid) = col.is_playlist().map(str::to_string) else {
            self.info("Reordering works inside a playlist");
            return;
        };
        if !col.filter.is_empty() || col.sort != SortKey::Default || col.hide_watched {
            self.info("Clear the filter, sort and hide-watched to reorder");
            return;
        }
        let cur = col.cursor;
        let target = if down { cur + 1 } else { cur.wrapping_sub(1) };
        if target >= col.items.len() {
            return;
        }
        col.items.swap(cur, target);
        col.rebuild(&mut self.fuzzy);
        col.cursor = target;
        let entry = col.items[target].playlist_item_id;
        let after = target.checked_sub(1).and_then(|i| col.items[i].playlist_item_id);
        let Some(entry) = entry else { return };
        let path = col.path.clone().unwrap_or_default();
        let server = self.server();
        self.pending += 1;
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = match server {
                Ok(s) => s.move_playlist_item(&pid, entry, after).await.map(|_| None).map_err(|e| format!("Reorder failed: {e}")),
                Err(e) => Err(e),
            };
            let refresh = if result.is_err() { Refresh::Paths(vec![path]) } else { Refresh::Nothing };
            let _ = tx.send(Msg::OpDone { result, refresh });
        });
    }

    // ---- watchlist ---------------------------------------------------------

    fn toggle_watchlist(&mut self) {
        let Some(item) = self.selected_detail().cloned() else { return };
        if !matches!(item.kind(), Kind::Movie | Kind::Show | Kind::Remote) {
            self.info("Only movies and shows go on the watchlist");
            return;
        }
        let (Some(guid), Some(id)) = (item.guid.clone(), item.discover_id().map(str::to_string)) else {
            self.info(format!("{} isn't matched with the Plex agent, so it can't be watchlisted", item.title()));
            return;
        };
        let tv = match self.core.plex_tv() {
            Ok(tv) => tv,
            Err(e) => return self.error(err(e)),
        };
        let on = !self.watchlist.contains(&guid);
        if on {
            self.watchlist.insert(guid);
        } else {
            self.watchlist.remove(&guid);
        }
        let title = item.title().to_string();
        self.spawn_op(Refresh::Watchlist, async move {
            if on { tv.add_to_watchlist(&id).await } else { tv.remove_from_watchlist(&id).await }.map_err(err)?;
            Ok(Some(if on { format!("Added {title} to your watchlist") } else { format!("Removed {title} from your watchlist") }))
        });
    }

    // ---- collections -------------------------------------------------------

    fn open_collections(&mut self) {
        let Some(item) = self.selected_detail().cloned() else { return };
        let section_id = item.section_id().or_else(|| self.columns.last()?.section_id().map(str::to_string));
        let (Some(key), Some(section_id)) = (item.rating_key.clone(), section_id) else {
            self.info("Select a library item to manage its collections");
            return;
        };
        if !matches!(item.kind(), Kind::Movie | Kind::Show | Kind::Artist | Kind::Album) {
            self.info("Collections hold movies, shows, artists and albums");
            return;
        }
        self.modal = Some(Modal::Collections(CollectionPicker {
            item,
            rows: Vec::new(),
            cursor: 0,
            loading: true,
            error: None,
        }));
        let (server, tx) = (self.server(), self.tx.clone());
        tokio::spawn(async move {
            let result = async {
                let server = server?;
                // Fresh metadata for the item's current collection tags.
                let full = server.metadata(&key, MetadataOptions::default()).await.map_err(err)?.ok_or("Item not found")?;
                let rows = server
                    .collections(&section_id)
                    .await
                    .map_err(err)?
                    .into_iter()
                    .filter(|c| !c.smart)
                    .map(|c| CollectionRow {
                        member: full.collections.iter().any(|t| t.name() == c.title()),
                        collection: Item::from(c),
                        busy: false,
                    })
                    .collect();
                Ok::<_, String>(rows)
            }
            .await;
            let _ = tx.send(Msg::CollectionRows(result));
        });
    }

    fn toggle_collection_row(&mut self) {
        let Some(Modal::Collections(c)) = &mut self.modal else { return };
        let item_key = c.item.rating_key.clone().unwrap_or_default();
        if c.cursor == c.rows.len() {
            self.modal = Some(Modal::Input(Input {
                title: "New collection".into(),
                hint: "Name".into(),
                value: String::new(),
                purpose: InputPurpose::NewCollection { rating_key: item_key },
            }));
            return;
        }
        let Some(row) = c.rows.get_mut(c.cursor) else { return };
        if row.busy {
            return;
        }
        row.busy = true;
        let collection_key = row.collection.rating_key.clone().unwrap_or_default();
        let member = row.member;
        let (server, tx) = (self.server(), self.tx.clone());
        tokio::spawn(async move {
            let result = async {
                let server = server?;
                if member {
                    server.remove_from_collection(&collection_key, &item_key).await
                } else {
                    server.add_to_collection(&collection_key, &server.item_uri(&[&item_key])).await
                }
                .map_err(err)?;
                Ok(!member)
            }
            .await;
            let _ = tx.send(Msg::CollectionToggled { collection_key, result });
        });
    }

    fn open_smart_playlist(&mut self) {
        let Some(col) = self.columns.last() else { return };
        match smart_playlist_path(col, &self.sections) {
            Some((kind, filter_path)) => {
                self.modal = Some(Modal::Input(Input {
                    title: "New smart playlist".into(),
                    hint: format!("Name (items matching {})", col.title),
                    value: col.title.clone(),
                    purpose: InputPurpose::SmartPlaylist { kind, filter_path },
                }));
            }
            None => self.info("Open a library listing (or a browse-by view) to save it"),
        }
    }

    // ---- metadata editing ------------------------------------------------

    fn editable(&mut self) -> Option<Item> {
        let item = self.selected_detail().cloned()?;
        if item.rating_key.is_none() || item.type_number().is_none() || !detail_kind(&item) {
            self.info("Select a library item");
            return None;
        }
        Some(item)
    }

    fn open_edit(&mut self) {
        let Some(item) = self.editable() else { return };
        let key = item.rating_key.clone().unwrap_or_default();
        // Listings lack labels and lock flags; edit from full metadata.
        let (server, tx) = (self.server(), self.tx.clone());
        tokio::spawn(async move {
            let r = async {
                let m = server?.metadata(&key, MetadataOptions::default()).await.map_err(err)?;
                m.map(|m| Box::new(Item::from(m))).ok_or_else(|| "Item not found".to_string())
            }
            .await;
            let _ = tx.send(Msg::EditItem(r));
        });
    }

    fn submit_edit(&mut self, item: Item, field: &'static str, value: String) {
        let key = item.rating_key.clone().unwrap_or_default();
        if field != "labels" {
            self.patch(&key, &mut |i| set_field_value(i, field, &value));
        }
        let core = self.core.clone();
        self.spawn_op(Refresh::Item(key.clone()), async move {
            if field == "labels" {
                let labels: Vec<String> = value.split(',').map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
                let current: Vec<String> = item.labels.iter().map(|t| t.name().to_string()).filter(|l| !l.is_empty()).collect();
                let tag = TagEdit {
                    tag: "label".into(),
                    add: labels.iter().filter(|l| !current.contains(l)).cloned().collect(),
                    remove: current.iter().filter(|l| !labels.contains(l)).cloned().collect(),
                };
                edit::edit_metadata(&core, &key, &[], &[tag]).await
            } else {
                edit::edit_metadata(&core, &key, &[(field.to_string(), value)], &[]).await
            }
            .map_err(err)?;
            Ok(Some("Saved (field locked against agent updates)".into()))
        });
    }

    fn toggle_field_lock(&mut self, item: &Item, field: &'static str) {
        let key = item.rating_key.clone().unwrap_or_default();
        let lock = !item.is_field_locked(field);
        let (section, kind) = (item.library_section_id, item.type_number().unwrap_or(1));
        let done = if lock { "Field locked" } else { "Field unlocked" };
        self.server_op(Refresh::Item(key.clone()), done, move |s| async move { s.lock_fields(&key, section, kind, &[(field, lock)]).await });
    }

    /// Open a picker whose rows load in the background.
    fn open_picker<F>(&mut self, title: String, kind: PickKind, hint: &'static str, load: F)
    where
        F: std::future::Future<Output = Result<Vec<(PickEntry, String)>, String>> + Send + 'static,
    {
        self.modal = Some(Modal::Pick(Picker { title, kind, rows: Vec::new(), cursor: 0, loading: true, error: None, hint }));
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(Msg::PickRows(load.await));
        });
    }

    fn open_posters(&mut self) {
        let Some(item) = self.editable() else { return };
        let key = item.rating_key.clone().unwrap_or_default();
        let server = self.server();
        let k = key.clone();
        self.open_picker(format!("Poster · {}", item.title()), PickKind::Posters { rating_key: key }, " enter use · v view · esc close ", async move {
            let posters = server?.posters(&k).await.map_err(err)?;
            let mut rows = vec![(PickEntry::PosterFromUrl, "+ Use an image URL…".to_string())];
            for (i, p) in posters.into_iter().enumerate() {
                let selected = p.extra.get("selected").is_some_and(|v| v.as_bool().unwrap_or(v.as_i64() == Some(1)));
                let provider = p.extra.get("provider").and_then(|v| v.as_str()).unwrap_or("uploaded").to_string();
                let label = format!("{} {provider} #{}", if selected { "●" } else { " " }, i + 1);
                rows.push((PickEntry::Poster(Box::new(Item::from(p))), label));
            }
            Ok(rows)
        });
    }

    fn search_matches(&mut self, rating_key: String, query: String, year: Option<i64>) {
        let server = self.server();
        let k = rating_key.clone();
        self.open_picker(format!("Match · {query}"), PickKind::Matches { rating_key }, " enter apply · esc close ", async move {
            let title = Some(query.as_str()).filter(|q| !q.is_empty());
            let results = server?.matches(&k, title, year, None).await.map_err(err)?;
            Ok(results
                .into_iter()
                .map(|m| {
                    let mut label = m.name.clone().unwrap_or_default();
                    if let Some(y) = m.year {
                        label.push_str(&format!(" ({y})"));
                    }
                    if let Some(sc) = m.score {
                        label.push_str(&format!(" · {sc}%"));
                    }
                    (PickEntry::Match(Box::new(m)), label)
                })
                .collect())
        });
    }

    fn search_subtitles(&mut self, rating_key: String, language: String) {
        let server = self.server();
        let k = rating_key.clone();
        self.open_picker(format!("Subtitles · {language}"), PickKind::Subtitles { rating_key }, " enter download · esc close ", async move {
            let results = server?.search_subtitles(&k, &language, false, false).await.map_err(err)?;
            Ok(results
                .into_iter()
                .map(|st| {
                    let mut label = st.title.clone().or_else(|| st.display_title.clone()).unwrap_or_else(|| st.label());
                    if st.hearing_impaired {
                        label.push_str(" · SDH");
                    }
                    if let Some(p) = &st.provider_title {
                        label.push_str(&format!(" · {p}"));
                    }
                    if let Some(sc) = st.score {
                        label.push_str(&format!(" · score {sc}"));
                    }
                    (PickEntry::Subtitle(Box::new(st)), label)
                })
                .collect())
        });
    }

    fn open_users(&mut self) {
        let tv = match self.core.plex_tv() {
            Ok(tv) => tv,
            Err(e) => return self.error(err(e)),
        };
        let current = self.username().unwrap_or_default().to_string();
        self.open_picker("Switch user".into(), PickKind::Users, " enter switch · esc close ", async move {
            let users = tv.home_users().await.map_err(err)?;
            Ok(users
                .into_iter()
                .map(|u| {
                    let mut label = format!("{} {}", if u.name() == current { "●" } else { " " }, u.name());
                    if u.admin {
                        label.push_str(" · admin");
                    } else if u.restricted {
                        label.push_str(" · managed");
                    }
                    if u.protected {
                        label.push_str(" · PIN");
                    }
                    (PickEntry::User(Box::new(u)), label)
                })
                .collect())
        });
    }

    /// Switch Plex Home user; the session reconnects and the browser is rebuilt.
    fn switch_user(&mut self, user: HomeUser, pin: Option<String>) {
        let Some(uuid) = user.uuid.clone() else { return };
        self.info(format!("Switching to {}…", user.name()));
        let core = self.core.clone();
        self.spawn_op(Refresh::Nothing, async move {
            session::switch_user(&core, &uuid, pin.as_deref()).await.map_err(err)?;
            Ok(None)
        });
    }

    fn pick_selected(&mut self, p: Picker) {
        let Some((entry, _)) = p.rows.into_iter().nth(p.cursor) else { return };
        match (p.kind, entry) {
            (PickKind::Posters { rating_key }, PickEntry::PosterFromUrl) => {
                self.modal = Some(Modal::Input(Input {
                    title: "Poster from URL".into(),
                    hint: "Image URL (https://…)".into(),
                    value: String::new(),
                    purpose: InputPurpose::PosterUrl { rating_key },
                }));
            }
            (PickKind::Posters { rating_key }, PickEntry::Poster(poster)) => {
                let Some(url) = poster.rating_key.clone().or_else(|| poster.key.clone()) else { return };
                self.server_op(Refresh::Item(rating_key.clone()), "Poster changed", move |s| async move { s.set_poster(&rating_key, &url).await });
            }
            (PickKind::Matches { rating_key }, PickEntry::Match(m)) => {
                let Some(guid) = m.guid.clone() else { return };
                let done = format!("Matched to {}; refreshing metadata", m.name.as_deref().unwrap_or("the result"));
                self.server_op(Refresh::Item(rating_key.clone()), done, move |s| async move { s.apply_match(&rating_key, &guid, m.name.as_deref()).await });
            }
            (PickKind::Subtitles { rating_key }, PickEntry::Subtitle(st)) => {
                let Some(key) = st.key.clone() else { return };
                self.server_op(Refresh::Item(rating_key.clone()), "Subtitle downloaded", move |s| async move { s.download_subtitle(&rating_key, &key).await });
            }
            (PickKind::Users, PickEntry::User(u)) => {
                if Some(u.name()) == self.username() {
                    self.info(format!("Already signed in as {}", u.name()));
                } else if u.protected {
                    self.modal = Some(Modal::Input(Input {
                        title: format!("PIN for {}", u.name()),
                        hint: "4-digit PIN".into(),
                        value: String::new(),
                        purpose: InputPurpose::UserPin { user: u },
                    }));
                } else {
                    self.switch_user(*u, None);
                }
            }
            _ => {}
        }
    }

    fn open_menu(&mut self) {
        let Some(col) = self.columns.last() else { return };
        let Some(item) = self.selected().cloned() else { return };
        use MenuAction::*;
        let mut rows: Vec<(MenuAction, &'static str, &'static str)> = Vec::new();
        let kind = item.kind();
        if item.is_playable() {
            rows.push((Play, "Play / resume", "enter"));
            rows.push((PlayFromStart, "Play from start", "p"));
            rows.push((PlayOptions, "Play with options…", "o"));
            rows.push((Shuffle, "Shuffle from here", "z"));
        } else if item.rating_key.is_some() && matches!(kind, Kind::Show | Kind::Season | Kind::Artist | Kind::Album | Kind::Playlist | Kind::Collection) {
            rows.push((PlayAll, "Play all", "a"));
            rows.push((Shuffle, "Shuffle", "z"));
        }
        if item.is_watchable() {
            rows.push((MarkWatched, "Mark watched", "w"));
            rows.push((MarkUnwatched, "Mark unwatched", "u"));
        }
        if detail_kind(&item) {
            rows.push((Rate, "Rate…", "*"));
        }
        if matches!(kind, Kind::Movie | Kind::Show | Kind::Remote) && item.guid.is_some() && self.core.tv().is_some() {
            let on = item.guid.as_ref().is_some_and(|g| self.watchlist.contains(g));
            rows.push((Watchlist, if on { "Remove from watchlist" } else { "Add to watchlist" }, "+"));
        }
        if item.rating_key.is_some() && (item.is_playable() || matches!(kind, Kind::Show | Kind::Season | Kind::Album | Kind::Artist)) {
            rows.push((AddToPlaylist, "Playlists…", "space"));
        }
        if item.rating_key.is_some() && matches!(kind, Kind::Movie | Kind::Show | Kind::Artist | Kind::Album) {
            rows.push((Collections, "Collections…", "C"));
        }
        let detail = self.selected_detail();
        if let Some(n) = detail.and_then(|d| d.extras.as_ref()).and_then(|e| e.size).filter(|n| *n > 0) {
            rows.push((Extras, if n == 1 { "Extras (1)" } else { "Extras (trailers, featurettes…)" }, ""));
        }
        if detail_kind(&item) {
            rows.push((Related, "Related items", ""));
            rows.push((OpenWeb, "Open in Plex Web", "O"));
        }
        if item.is_playable() {
            rows.push((CopyUrl, "Copy stream URL", "y"));
        }
        if item.rating_key.is_some() && (item.is_playable() || matches!(kind, Kind::Show | Kind::Season | Kind::Album | Kind::Artist)) {
            rows.push((Download, "Download for offline", "D"));
        }
        if matches!(kind, Kind::Movie | Kind::Episode) {
            rows.push((SearchSubtitles, "Search subtitles online…", ""));
        }
        if detail_kind(&item) && item.type_number().is_some() {
            rows.push((EditDetails, "Edit details…", "e"));
            rows.push((ChangePoster, "Change poster…", ""));
        }
        if matches!(kind, Kind::Movie | Kind::Show | Kind::Artist | Kind::Album) {
            rows.push((FixMatch, "Fix match…", ""));
            rows.push((Unmatch, "Unmatch", ""));
        }
        if detail_kind(&item) {
            rows.push((RefreshMetadata, "Refresh metadata", ""));
            rows.push((Analyze, "Analyze media", ""));
        }
        if detail_kind(&item) && kind != Kind::Collection {
            rows.push((DeleteMedia, "Delete from disk…", ""));
        }
        if smart_playlist_path(col, &self.sections).is_some() {
            rows.push((SmartPlaylist, "Save view as smart playlist…", ""));
        }
        if kind == Kind::Section || col.section_id().is_some() {
            rows.push((BrowseBy, "Browse by…", "v"));
            rows.push((ScanLibrary, "Scan library files", "S"));
            rows.push((ForceScanLibrary, "Refresh all library metadata", ""));
            rows.push((EmptyTrash, "Empty library trash", ""));
        }
        if kind == Kind::Playlist {
            rows.push((RenamePlaylist, "Rename playlist…", "e"));
            rows.push((DeletePlaylist, "Delete playlist", "x"));
        }
        if col.is_playlist().is_some() && item.playlist_item_id.is_some() {
            rows.push((RemoveFromPlaylist, "Remove from playlist", "x"));
        }
        if col.is_collection().is_some() {
            rows.push((RemoveFromCollection, "Remove from collection", "x"));
        }
        if col.is_sessions() && item.session.is_some() {
            rows.push((TerminateSession, "Stop this stream", "x"));
        }
        if self.core.tv().is_some() {
            rows.push((SwitchUser, "Switch Plex Home user…", "U"));
        }
        self.modal = Some(Modal::Menu(Menu { title: item.full_title(), rows, cursor: 0 }));
    }

    fn run_menu(&mut self, action: MenuAction) {
        use MenuAction::*;
        let item = self.selected().cloned();
        match action {
            Play => self.play_selected(false),
            PlayFromStart => self.play_selected(true),
            PlayAll => {
                if let Some(i) = item {
                    self.play_container(i, false)
                }
            }
            Shuffle => {
                if let Some(i) = item {
                    self.play_container(i, true)
                }
            }
            PlayOptions => self.open_play_options(),
            MarkWatched => self.set_watched(true),
            MarkUnwatched => self.set_watched(false),
            Rate => self.open_rate(),
            AddToPlaylist => self.open_playlist_picker(),
            Related => {
                if let Some(i) = item
                    && let Some(k) = i.rating_key.clone()
                {
                    self.search = None;
                    self.open_column(format!("Related to {}", i.title()), format!("/library/metadata/{k}/related"));
                }
            }
            OpenWeb => self.open_web(),
            CopyUrl => self.copy_url(),
            Download => self.download_selected(),
            RefreshMetadata | Analyze => {
                let Some(i) = item else { return };
                let Some(k) = i.rating_key.clone() else { return };
                let refresh = action == RefreshMetadata;
                self.details.remove(&k);
                let done = format!("{} {}", if refresh { "Refreshing metadata for" } else { "Analyzing" }, i.title());
                self.server_op(Refresh::Nothing, done, move |s| async move {
                    if refresh { s.refresh_metadata(&k, false).await } else { s.analyze_metadata(&k).await }
                });
            }
            ScanLibrary => self.scan_library(false),
            ForceScanLibrary => self.scan_library(true),
            EmptyTrash => {
                let sid = item
                    .as_ref()
                    .filter(|i| i.kind() == Kind::Section)
                    .and_then(|i| i.key.clone())
                    .or_else(|| self.columns.last()?.section_id().map(str::to_string));
                if let Some(section_id) = sid {
                    self.modal = Some(Modal::Confirm(Confirm {
                        message: "Permanently remove this library's missing items?".into(),
                        action: ConfirmAction::EmptyTrash { section_id },
                    }));
                }
            }
            BrowseBy => self.open_browse(),
            RenamePlaylist => self.open_rename(),
            Watchlist => self.toggle_watchlist(),
            Collections => self.open_collections(),
            SmartPlaylist => self.open_smart_playlist(),
            Extras => {
                if let Some(i) = item
                    && let Some(k) = i.rating_key.clone()
                {
                    self.search = None;
                    self.open_column(format!("Extras · {}", i.title()), format!("/library/metadata/{k}/extras"));
                }
            }
            EditDetails => self.open_edit(),
            ChangePoster => self.open_posters(),
            FixMatch => {
                if let Some(i) = self.editable() {
                    self.modal = Some(Modal::Input(Input {
                        title: format!("Fix match · {}", i.title()),
                        hint: "Search title".into(),
                        value: i.title().to_string(),
                        purpose: InputPurpose::MatchQuery { rating_key: i.rating_key.clone().unwrap_or_default(), year: i.year },
                    }));
                }
            }
            Unmatch => {
                if let Some(i) = self.editable() {
                    self.modal = Some(Modal::Confirm(Confirm {
                        message: format!("Unmatch {}? Its metadata will be cleared.", i.title()),
                        action: ConfirmAction::Unmatch { rating_key: i.rating_key.clone().unwrap_or_default() },
                    }));
                }
            }
            DeleteMedia => {
                if let Some(i) = self.editable() {
                    self.modal = Some(Modal::Confirm(Confirm {
                        message: format!("Delete {} and its files from disk? This can't be undone.", i.full_title()),
                        action: ConfirmAction::DeleteMedia { rating_key: i.rating_key.clone().unwrap_or_default(), title: i.full_title() },
                    }));
                }
            }
            SearchSubtitles => {
                if let Some(i) = item.filter(|i| matches!(i.kind(), Kind::Movie | Kind::Episode)) {
                    self.modal = Some(Modal::Input(Input {
                        title: format!("Subtitles · {}", i.full_title()),
                        hint: "Language code (en, es, fr, …)".into(),
                        value: "en".into(),
                        purpose: InputPurpose::SubtitleLanguage { rating_key: i.rating_key.clone().unwrap_or_default() },
                    }));
                }
            }
            SwitchUser => self.open_users(),
            DeletePlaylist | RemoveFromPlaylist | RemoveFromCollection | TerminateSession => self.delete_action(),
        }
    }

    fn open_rate(&mut self) {
        let Some(item) = self.selected_detail().cloned() else { return };
        if !detail_kind(&item) {
            self.info("That can't be rated");
            return;
        }
        self.modal = Some(Modal::Input(Input {
            title: format!("Rate {}", item.title()),
            hint: "0–10 (empty clears)".into(),
            value: item.user_rating.map(|r| format!("{r}")).unwrap_or_default(),
            purpose: InputPurpose::Rate { rating_key: item.rating_key.clone().unwrap_or_default() },
        }));
    }

    fn open_rename(&mut self) {
        let Some(item) = self.selected().cloned() else { return };
        if item.kind() != Kind::Playlist {
            self.info("Select a playlist to rename it");
            return;
        }
        self.modal = Some(Modal::Input(Input {
            title: "Rename playlist".into(),
            hint: "Name".into(),
            value: item.title().to_string(),
            purpose: InputPurpose::RenamePlaylist { playlist_id: item.rating_key.clone().unwrap_or_default() },
        }));
    }

    fn open_browse(&mut self) {
        let sid = self
            .selected()
            .filter(|i| i.kind() == Kind::Section)
            .and_then(|i| i.key.clone())
            .or_else(|| self.columns.last()?.section_id().map(str::to_string));
        let Some(section_id) = sid else {
            self.info("Browse-by works in a library");
            return;
        };
        let section_title = self.sections.iter().find(|s| s.key.as_deref() == Some(&section_id)).map(|s| s.title().to_string()).unwrap_or_default();
        self.modal = Some(Modal::Browse { section_id, section_title, cursor: 0 });
    }

    fn browse_to(&mut self, section_id: &str, section_title: &str, view: usize) {
        let (label, sub) = BROWSE_VIEWS[view];
        let path = format!("/library/sections/{section_id}/{sub}");
        let title = if view == 0 { section_title.to_string() } else { format!("{section_title} · {label}") };
        // Replace the library's column (or open one from the root).
        let keep = if self.columns.last().and_then(|c| c.section_id()) == Some(section_id) {
            self.columns.iter().position(|c| c.section_id() == Some(section_id)).unwrap_or(1)
        } else {
            self.columns.len()
        };
        self.columns.truncate(keep.max(1));
        self.open_column(title, path);
    }

    fn refresh_all(&mut self) {
        self.details.clear();
        let core = self.core.clone();
        tokio::spawn(async move {
            let _ = session::reload_sections(&core).await;
        });
        for idx in 1..self.columns.len() {
            self.load(idx);
        }
        self.info("Refreshing everything…");
    }

    // ---- search ----------------------------------------------------------

    fn open_search(&mut self) {
        self.search = Some(SearchState {
            input: String::new(),
            results: Vec::new(),
            cursor: 0,
            scroll: 0,
            pending: false,
            dirty_at: None,
            error: None,
        });
    }

    fn search_changed(&mut self) {
        let Some(s) = &mut self.search else { return };
        s.error = None;
        s.dirty_at = Some(Instant::now());
        s.pending = !s.input.trim().is_empty();
        self.after_selection_change();
    }

    /// Jump from a search result into the browser.
    fn search_accept(&mut self) {
        let Some(s) = self.search.take() else { return };
        let Some(item) = s.results.get(s.cursor).cloned() else { return };
        self.columns.truncate(1);
        let mut col = self.new_column(format!("Search: {}", s.input.trim()), None);
        col.items = s.results;
        col.loaded = true;
        col.rebuild(&mut self.fuzzy);
        col.cursor = s.cursor.min(col.view.len().saturating_sub(1));
        self.columns.push(col);
        if item.is_container() {
            self.drill();
        }
        self.after_selection_change();
    }

    fn search_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let Some(s) = &mut self.search else { return };
        match key.code {
            KeyCode::Esc => self.search = None,
            KeyCode::Enter => self.search_accept(),
            KeyCode::Down => s.cursor = (s.cursor + 1).min(s.results.len().saturating_sub(1)),
            KeyCode::Char('n' | 'j') if ctrl => s.cursor = (s.cursor + 1).min(s.results.len().saturating_sub(1)),
            KeyCode::Up => s.cursor = s.cursor.saturating_sub(1),
            KeyCode::Char('p' | 'k') if ctrl => s.cursor = s.cursor.saturating_sub(1),
            KeyCode::Char('u') if ctrl => {
                s.input.clear();
                self.search_changed();
            }
            KeyCode::Backspace => {
                s.input.pop();
                self.search_changed();
            }
            KeyCode::Char(c) if !ctrl => {
                s.input.push(c);
                self.search_changed();
            }
            _ => {}
        }
        self.after_selection_change();
    }

    // ---- input -----------------------------------------------------------

    pub fn on_event(&mut self, ev: Event) {
        match ev {
            Event::Key(k) if k.kind != KeyEventKind::Release => self.on_key(k),
            Event::Paste(text) => {
                if let Some(s) = &mut self.search {
                    s.input.push_str(text.trim());
                    self.search_changed();
                } else if let Some(Modal::Input(i)) = &mut self.modal {
                    i.value.push_str(text.trim());
                } else if let Some(col) = self.columns.last_mut().filter(|c| c.filtering) {
                    col.filter.push_str(text.trim());
                    col.refilter(&mut self.fuzzy);
                }
            }
            _ => {}
        }
    }

    fn on_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.quit = true;
            return;
        }
        if self.modal.is_some() {
            self.modal_key(key);
        } else if !self.ready() {
            self.stage_key(key);
            return;
        } else if self.search.is_some() {
            self.search_key(key);
        } else if self.columns.last().is_some_and(|c| c.filtering) {
            self.filter_key(key);
        } else {
            self.normal_key(key);
        }
        self.after_selection_change();
    }

    fn filter_key(&mut self, key: KeyEvent) {
        let Some(col) = self.columns.last_mut() else { return };
        match key.code {
            KeyCode::Esc => {
                col.filtering = false;
                col.filter.clear();
                col.refilter(&mut self.fuzzy);
            }
            KeyCode::Enter => col.filtering = false,
            KeyCode::Backspace => {
                if col.filter.pop().is_none() {
                    col.filtering = false;
                }
                col.refilter(&mut self.fuzzy);
            }
            KeyCode::Down => col.move_by(1),
            KeyCode::Up => col.move_by(-1),
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                col.filter.push(c);
                col.refilter(&mut self.fuzzy);
            }
            _ => {}
        }
    }

    fn normal_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let page = self.list_height.max(2) as isize;
        let Some(col) = self.columns.last_mut() else { return };
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.modal = Some(Modal::Help { scroll: 0 }),
            KeyCode::Char('j') | KeyCode::Down => col.move_by(1),
            KeyCode::Char('k') | KeyCode::Up => col.move_by(-1),
            KeyCode::Char('d') if ctrl => col.move_by(page / 2),
            KeyCode::Char('u') if ctrl => col.move_by(-page / 2),
            KeyCode::Char('f') if ctrl => col.move_by(page),
            KeyCode::Char('b') if ctrl => col.move_by(-page),
            KeyCode::PageDown => col.move_by(page),
            KeyCode::PageUp => col.move_by(-page),
            KeyCode::Char('g') | KeyCode::Home => col.move_to(0),
            KeyCode::Char('G') | KeyCode::End => col.move_to(usize::MAX),
            KeyCode::Char('l') | KeyCode::Right => self.drill(),
            KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => self.back(),
            KeyCode::Esc => {
                if !col.filter.is_empty() {
                    col.filter.clear();
                    col.refilter(&mut self.fuzzy);
                } else if self.notice.is_some() {
                    self.notice = None;
                }
            }
            KeyCode::Enter => {
                if col.selected().is_some_and(Item::is_playable) {
                    self.play_selected(false);
                } else {
                    self.drill();
                }
            }
            KeyCode::Char('p') => {
                if col.selected().is_some_and(Item::is_playable) {
                    self.play_selected(true);
                }
            }
            KeyCode::Char('a') => {
                if let Some(i) = col.selected().cloned() {
                    self.play_container(i, false);
                }
            }
            KeyCode::Char('z') => {
                if let Some(i) = col.selected().cloned() {
                    self.play_container(i, true);
                }
            }
            KeyCode::Char('o') => self.open_play_options(),
            KeyCode::Char('P') => self.control(Control::TogglePause),
            KeyCode::Char('>') => self.control(Control::Next),
            KeyCode::Char('<') => self.control(Control::Previous),
            KeyCode::Char('=') => self.control(Control::SeekRelative(SEEK_SECONDS)),
            KeyCode::Char('-') => self.control(Control::SeekRelative(-SEEK_SECONDS)),
            KeyCode::Char('X') => self.control(Control::Stop),
            KeyCode::Char('w') => self.set_watched(true),
            KeyCode::Char('u') => self.set_watched(false),
            KeyCode::Char('*') => self.open_rate(),
            KeyCode::Char(' ') => self.open_playlist_picker(),
            KeyCode::Char('x') | KeyCode::Delete => self.delete_action(),
            KeyCode::Char('e') => {
                if col.selected().is_some_and(|i| i.kind() == Kind::Playlist) {
                    self.open_rename();
                } else {
                    self.open_edit();
                }
            }
            KeyCode::Char('+') => self.toggle_watchlist(),
            KeyCode::Char('C') => self.open_collections(),
            KeyCode::Char('U') => self.open_users(),
            KeyCode::Char('J') => self.move_in_playlist(true),
            KeyCode::Char('K') => self.move_in_playlist(false),
            KeyCode::Char('f') => self.open_search(),
            KeyCode::Char('/') => col.filtering = true,
            KeyCode::Char('s') => {
                let cursor = SortKey::ALL.iter().position(|k| *k == col.sort).unwrap_or(0);
                self.modal = Some(Modal::Sort { cursor });
            }
            KeyCode::Char('v') => self.open_browse(),
            KeyCode::Char('H') => {
                col.hide_watched = !col.hide_watched;
                let on = col.hide_watched;
                col.rebuild(&mut self.fuzzy);
                self.info(if on { "Hiding watched items" } else { "Showing watched items" });
            }
            KeyCode::Char('i') => self.show_inspector = !self.show_inspector,
            KeyCode::Char(']') => self.inspector_scroll = self.inspector_scroll.saturating_add(3),
            KeyCode::Char('[') => self.inspector_scroll = self.inspector_scroll.saturating_sub(3),
            KeyCode::Char('r') => {
                let idx = self.columns.len() - 1;
                if idx == 0 {
                    let core = self.core.clone();
                    tokio::spawn(async move {
                        let _ = session::reload_sections(&core).await;
                    });
                } else {
                    if let Some(k) = self.selected().and_then(|i| i.rating_key.clone()) {
                        self.details.remove(&k);
                    }
                    self.load(idx);
                }
            }
            KeyCode::Char('R') => self.refresh_all(),
            KeyCode::Char('S') => self.scan_library(false),
            KeyCode::Char('m' | '.') => self.open_menu(),
            KeyCode::Char('O') => self.open_web(),
            KeyCode::Char('y') => self.copy_url(),
            KeyCode::Char('D') => self.download_selected(),
            KeyCode::Char('L') => {
                self.modal = Some(Modal::Confirm(Confirm { message: "Sign out of Patina on this device?".into(), action: ConfirmAction::Logout }))
            }
            _ => {}
        }
    }

    fn modal_key(&mut self, key: KeyEvent) {
        let Some(modal) = self.modal.take() else { return };
        let down = matches!(key.code, KeyCode::Char('j') | KeyCode::Down);
        let up = matches!(key.code, KeyCode::Char('k') | KeyCode::Up);
        let esc = matches!(key.code, KeyCode::Esc | KeyCode::Char('q'));
        let enter = key.code == KeyCode::Enter;

        let next: Option<Modal> = match modal {
            Modal::Help { scroll } => {
                if down {
                    Some(Modal::Help { scroll: scroll.saturating_add(1) })
                } else if up {
                    Some(Modal::Help { scroll: scroll.saturating_sub(1) })
                } else {
                    None
                }
            }
            Modal::Sort { mut cursor } => {
                let n = SortKey::ALL.len();
                if down {
                    cursor = (cursor + 1).min(n - 1);
                    Some(Modal::Sort { cursor })
                } else if up {
                    cursor = cursor.saturating_sub(1);
                    Some(Modal::Sort { cursor })
                } else if enter || key.code == KeyCode::Char(' ') {
                    let k = SortKey::ALL[cursor];
                    if let Some(col) = self.columns.last_mut() {
                        if col.sort == k {
                            col.sort_desc = !col.sort_desc;
                        } else {
                            col.sort = k;
                            col.sort_desc = k.natural_desc();
                        }
                        col.rebuild(&mut self.fuzzy);
                        col.move_to(0);
                    }
                    None
                } else if esc {
                    None
                } else {
                    Some(Modal::Sort { cursor })
                }
            }
            Modal::Browse { section_id, section_title, mut cursor } => {
                if down {
                    cursor = (cursor + 1).min(BROWSE_VIEWS.len() - 1);
                    Some(Modal::Browse { section_id, section_title, cursor })
                } else if up {
                    cursor = cursor.saturating_sub(1);
                    Some(Modal::Browse { section_id, section_title, cursor })
                } else if enter {
                    self.browse_to(&section_id, &section_title, cursor);
                    None
                } else if esc {
                    None
                } else {
                    Some(Modal::Browse { section_id, section_title, cursor })
                }
            }
            Modal::Menu(mut m) => {
                if down {
                    m.cursor = (m.cursor + 1).min(m.rows.len().saturating_sub(1));
                    Some(Modal::Menu(m))
                } else if up {
                    m.cursor = m.cursor.saturating_sub(1);
                    Some(Modal::Menu(m))
                } else if enter {
                    if let Some(row) = m.rows.get(m.cursor) {
                        self.run_menu(row.0);
                    }
                    return;
                } else if esc {
                    None
                } else {
                    Some(Modal::Menu(m))
                }
            }
            Modal::Input(mut i) => match key.code {
                KeyCode::Esc => None,
                KeyCode::Enter => {
                    self.submit_input(i);
                    return;
                }
                KeyCode::Backspace => {
                    i.value.pop();
                    Some(Modal::Input(i))
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    i.value.clear();
                    Some(Modal::Input(i))
                }
                KeyCode::Char(c) => {
                    i.value.push(c);
                    Some(Modal::Input(i))
                }
                _ => Some(Modal::Input(i)),
            },
            Modal::Confirm(c) => match key.code {
                KeyCode::Char('y' | 'Y') | KeyCode::Enter => {
                    self.run_confirmed(c.action);
                    None
                }
                KeyCode::Char('n' | 'N') | KeyCode::Esc | KeyCode::Char('q') => None,
                _ => Some(Modal::Confirm(c)),
            },
            Modal::Playlists(mut p) => {
                let n = p.rows.len() + 1;
                if down {
                    p.cursor = (p.cursor + 1).min(n - 1);
                    Some(Modal::Playlists(p))
                } else if up {
                    p.cursor = p.cursor.saturating_sub(1);
                    Some(Modal::Playlists(p))
                } else if key.code == KeyCode::Char('n') {
                    p.cursor = p.rows.len();
                    self.modal = Some(Modal::Playlists(p));
                    self.toggle_playlist_row();
                    return;
                } else if (enter || key.code == KeyCode::Char(' ')) && !p.loading && p.error.is_none() {
                    self.modal = Some(Modal::Playlists(p));
                    self.toggle_playlist_row();
                    return;
                } else if esc {
                    None
                } else {
                    Some(Modal::Playlists(p))
                }
            }
            Modal::PlayOptions(mut m) => {
                let n = m.rows.len();
                if down {
                    let mut c = m.cursor + 1;
                    while c < n && !m.selectable(c) {
                        c += 1;
                    }
                    if c < n {
                        m.cursor = c;
                    }
                    Some(Modal::PlayOptions(m))
                } else if up {
                    let mut c = m.cursor;
                    while c > 0 {
                        c -= 1;
                        if m.selectable(c) {
                            m.cursor = c;
                            break;
                        }
                    }
                    Some(Modal::PlayOptions(m))
                } else if enter || key.code == KeyCode::Char(' ') {
                    let row = m.rows.get(m.cursor).map(|r| r.0.clone());
                    match row {
                        Some(OptRow::Play) => {
                            self.play_with_options(m);
                            None
                        }
                        Some(OptRow::Version(i)) => {
                            m.opts.media_index = i;
                            m.opts.audio_stream_id = None;
                            m.opts.subtitle_stream_id = None;
                            build_option_rows(&mut m);
                            Some(Modal::PlayOptions(m))
                        }
                        Some(OptRow::Audio(id)) => {
                            m.opts.audio_stream_id = Some(id);
                            Some(Modal::PlayOptions(m))
                        }
                        Some(OptRow::Subtitle(id)) => {
                            m.opts.subtitle_stream_id = Some(id.unwrap_or(0));
                            Some(Modal::PlayOptions(m))
                        }
                        Some(OptRow::Resume(from_start)) => {
                            m.opts.from_start = from_start;
                            Some(Modal::PlayOptions(m))
                        }
                        Some(OptRow::Quality(q)) => {
                            m.opts.quality = q;
                            Some(Modal::PlayOptions(m))
                        }
                        _ => Some(Modal::PlayOptions(m)),
                    }
                } else if esc {
                    None
                } else {
                    Some(Modal::PlayOptions(m))
                }
            }
            Modal::Collections(mut c) => {
                let n = c.rows.len() + 1;
                if down {
                    c.cursor = (c.cursor + 1).min(n - 1);
                    Some(Modal::Collections(c))
                } else if up {
                    c.cursor = c.cursor.saturating_sub(1);
                    Some(Modal::Collections(c))
                } else if key.code == KeyCode::Char('n') && !c.loading {
                    c.cursor = c.rows.len();
                    self.modal = Some(Modal::Collections(c));
                    self.toggle_collection_row();
                    return;
                } else if (enter || key.code == KeyCode::Char(' ')) && !c.loading && c.error.is_none() {
                    self.modal = Some(Modal::Collections(c));
                    self.toggle_collection_row();
                    return;
                } else if esc {
                    None
                } else {
                    Some(Modal::Collections(c))
                }
            }
            Modal::Edit(mut m) => {
                let n = EDIT_FIELDS.len();
                if down {
                    m.cursor = (m.cursor + 1).min(n - 1);
                    Some(Modal::Edit(m))
                } else if up {
                    m.cursor = m.cursor.saturating_sub(1);
                    Some(Modal::Edit(m))
                } else if enter {
                    let (field, label) = EDIT_FIELDS[m.cursor];
                    self.modal = Some(Modal::Input(Input {
                        title: format!("{label} · {}", m.item.title()),
                        hint: match field {
                            "labels" => "Comma-separated".into(),
                            "originallyAvailableAt" => "YYYY-MM-DD".into(),
                            _ => label.into(),
                        },
                        value: field_value(&m.item, field),
                        purpose: InputPurpose::EditField { item: Box::new(m.item), field },
                    }));
                    return;
                } else if key.code == KeyCode::Char('l') {
                    let (field, _) = EDIT_FIELDS[m.cursor];
                    let field = if field == "labels" { "label" } else { field };
                    self.toggle_field_lock(&m.item, field);
                    None
                } else if esc {
                    None
                } else {
                    Some(Modal::Edit(m))
                }
            }
            Modal::Pick(mut p) => {
                if down {
                    p.cursor = (p.cursor + 1).min(p.rows.len().saturating_sub(1));
                    Some(Modal::Pick(p))
                } else if up {
                    p.cursor = p.cursor.saturating_sub(1);
                    Some(Modal::Pick(p))
                } else if key.code == KeyCode::Char('v') {
                    // Preview a poster candidate; the picker stays open.
                    if let Some((PickEntry::Poster(poster), _)) = p.rows.get(p.cursor)
                        && let Some(k) = &poster.key
                        && let Ok(server) = self.core.server()
                        && let Ok(url) = server.authed_url(k, &Params::new())
                        && let Err(e) = open::that_detached(url.as_str())
                    {
                        self.error(format!("Couldn't open the image: {e}"));
                    }
                    Some(Modal::Pick(p))
                } else if enter && !p.rows.is_empty() {
                    self.pick_selected(p);
                    return;
                } else if esc {
                    None
                } else {
                    Some(Modal::Pick(p))
                }
            }
        };
        if self.modal.is_none() {
            self.modal = next;
        }
    }
}

/// Lay out the play-options rows for the loaded item.
pub fn build_option_rows(m: &mut PlayOptionsModal) {
    let mut rows: Vec<(OptRow, String)> = Vec::new();
    if let Some(item) = &m.item {
        if item.media.len() > 1 {
            rows.push((OptRow::Header("Version"), String::new()));
            for (i, media) in item.media.iter().enumerate() {
                let label = [
                    crate::format::resolution(media.height, media.video_resolution.as_deref()),
                    media.video_codec.as_deref().map(crate::format::codec),
                    media.bitrate.map(crate::format::bitrate_kbps),
                    media.title.clone(),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" · ");
                rows.push((OptRow::Version(i), label));
            }
        }
        let part = item.media.get(m.opts.media_index).and_then(|md| md.parts.first());
        let streams = part.map(|p| p.streams.as_slice()).unwrap_or(&[]);
        let audio: Vec<_> = streams.iter().filter(|s| s.stream_type == Some(stream_type::AUDIO)).collect();
        if audio.len() > 1 {
            rows.push((OptRow::Header("Audio"), String::new()));
            for s in audio {
                rows.push((OptRow::Audio(s.id.unwrap_or_default()), s.label()));
            }
        }
        let subs: Vec<_> = streams.iter().filter(|s| s.stream_type == Some(stream_type::SUBTITLE)).collect();
        if !subs.is_empty() {
            rows.push((OptRow::Header("Subtitles"), String::new()));
            rows.push((OptRow::Subtitle(None), "None".into()));
            for s in subs {
                rows.push((OptRow::Subtitle(s.id), s.label()));
            }
        }
        if matches!(item.kind(), Kind::Movie | Kind::Episode | Kind::Clip) {
            rows.push((OptRow::Header("Quality"), String::new()));
            for (i, q) in QUALITIES.iter().enumerate() {
                let label = if q.kbps.is_some() { format!("Transcode · {}", q.label) } else { q.label.to_string() };
                rows.push((OptRow::Quality(i), label));
            }
        }
        if let Some(offset) = item.view_offset.filter(|o| *o > 0) {
            rows.push((OptRow::Header("Start"), String::new()));
            rows.push((OptRow::Resume(false), format!("Resume from {}", crate::format::clock_ms(offset))));
            rows.push((OptRow::Resume(true), "From the beginning".into()));
        }
        rows.push((OptRow::Header(""), String::new()));
    }
    rows.push((OptRow::Play, "▶ Play".into()));
    m.cursor = rows.iter().position(|(r, _)| !matches!(r, OptRow::Header(_))).unwrap_or(0);
    m.rows = rows;
}

/// Plex Web deep link for an item.
pub fn web_url(machine: &str, rating_key: &str) -> String {
    format!("https://app.plex.tv/desktop/#!/server/{machine}/details?key=%2Flibrary%2Fmetadata%2F{rating_key}")
}

/// For a library listing column (`/library/sections/<id>/all?…`), the
/// playlist type and filter path of an equivalent smart playlist.
pub fn smart_playlist_path(col: &Column, sections: &[Item]) -> Option<(&'static str, String)> {
    let path = col.path.as_deref()?;
    let sid = col.section_id()?;
    let rest = path.strip_prefix(&format!("/library/sections/{sid}/"))?;
    let (view, query) = rest.split_once('?').unwrap_or((rest, ""));
    if view != "all" {
        return None;
    }
    let section_kind = sections.iter().find(|s| s.key.as_deref() == Some(sid))?.meta.kind.clone()?;
    // Playlists hold leaves, so show and music filters apply to the parent.
    let (type_num, prefix, kind) = match section_kind.as_str() {
        "movie" => (1, None, "video"),
        "show" => (4, Some("show"), "video"),
        "artist" => (10, Some("artist"), "audio"),
        _ => return None,
    };
    let mut params = vec![format!("type={type_num}")];
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        if k == "type" {
            continue;
        }
        let k = match prefix {
            Some(p) if !k.contains('.') && !matches!(k, "unwatched" | "sort") => format!("{p}.{k}"),
            _ => k.to_string(),
        };
        params.push(format!("{k}={v}"));
    }
    Some((kind, format!("/library/sections/{sid}/all?{}", params.join("&"))))
}

/// The running download, as `(title, received, size)`.
pub fn active_download(list: &[Download]) -> Option<(String, i64, i64)> {
    let d = list.iter().find(|d| d.state == DownloadState::Downloading)?;
    Some((Item::from(d.item.clone()).full_title(), d.received, d.size))
}

fn which(binary: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else { return false };
    std::env::split_paths(&path).any(|dir| dir.join(binary).is_file())
}

/// Copy via the platform clipboard tool, falling back to OSC 52 (which
/// most modern terminals, including over SSH, understand).
fn copy_to_clipboard(text: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let tools: &[(&str, &[&str])] = &[
        ("pbcopy", &[]),
        ("wl-copy", &[]),
        ("xclip", &["-selection", "clipboard"]),
        ("xsel", &["--clipboard", "--input"]),
        ("clip.exe", &[]),
    ];
    for (cmd, args) in tools {
        if !which(cmd) {
            continue;
        }
        if let Ok(mut child) = Command::new(cmd).args(*args).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(text.as_bytes())?;
            }
            if child.wait().map(|s| s.success()).unwrap_or(false) {
                return Ok(());
            }
        }
    }
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    let mut out = std::io::stdout();
    write!(out, "\x1b]52;c;{encoded}\x07")?;
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use patina_plex::Directory;

    #[test]
    fn paged_paths() {
        assert!(is_paged("/library/sections/1/all"));
        assert!(is_paged("/library/sections/1/all?genre=3"));
        assert!(!is_paged("/library/sections"));
        assert!(is_paged("/playlists/9/items"));
        assert!(!is_paged("/playlists"));
        assert!(!is_paged("/library/metadata/4/children"));
    }

    #[test]
    fn smart_playlist_paths() {
        let section = |key: &str, kind: &str| Item::section(Directory { key: Some(key.into()), kind: Some(kind.into()), ..Default::default() });
        let sections = vec![section("1", "movie"), section("2", "show")];
        let col = |p: &str| Column::new(9, "x", Some(p.into()));
        assert_eq!(
            smart_playlist_path(&col("/library/sections/1/all?genre=18"), &sections),
            Some(("video", "/library/sections/1/all?type=1&genre=18".into()))
        );
        assert_eq!(
            smart_playlist_path(&col("/library/sections/2/all?unwatched=1&genre=5"), &sections).map(|(_, p)| p),
            Some("/library/sections/2/all?type=4&unwatched=1&show.genre=5".into())
        );
        assert!(smart_playlist_path(&col("/library/sections/1/genre"), &sections).is_none());
        assert!(smart_playlist_path(&col("/playlists/3/items"), &sections).is_none());
    }

    #[test]
    fn web_links() {
        assert_eq!(web_url("abc", "7"), "https://app.plex.tv/desktop/#!/server/abc/details?key=%2Flibrary%2Fmetadata%2F7");
    }

    #[test]
    fn option_rows_for_a_movie() {
        let mut item = crate::item::test_item("movie", "Heat");
        item.view_offset = Some(60_000);
        let mut m = PlayOptionsModal { item: Some(item), opts: PlayOptions::with_quality("original"), cursor: 0, rows: Vec::new() };
        build_option_rows(&mut m);
        assert!(m.rows.iter().any(|(r, _)| *r == OptRow::Quality(0)));
        assert!(m.is_checked(&OptRow::Quality(0)));
        assert!(m.is_checked(&OptRow::Resume(false)));
        assert_eq!(m.rows.last().map(|r| &r.0), Some(&OptRow::Play));
        assert!(m.selectable(m.cursor));
    }
}
