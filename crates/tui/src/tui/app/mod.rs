//! Application state and update logic, split by concern: `nav` (columns
//! and loading), `keys` (input), `menu` (the actions menu), `playback`,
//! `actions` (item and library operations), `edit` (metadata and pickers),
//! `search` and `login`.

mod actions;
mod edit;
mod keys;
mod login;
mod menu;
mod nav;
mod playback;
mod search;

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use patina_core::config::TuiSettings;
use patina_core::downloads::{self, Download};
use patina_core::ops::lists;
use patina_core::player::{self, QueueCommand};
use patina_core::state::{NowPlaying, QueueView, ServerActivity, SessionView, Stage};
use patina_core::{AppState, CoreEvent};
use patina_plex::{Metadata, MetadataOptions, Params, PlexServer};
use tokio::sync::mpsc::UnboundedSender;

use super::column::Column;
use super::modal::*;
use super::source::Source;
use crate::item::{Item, Kind};
use crate::search::Fuzzy;

pub use playback::{active_download, build_option_rows};

pub const PLAYLISTS: &str = "/playlists";

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
#[derive(Debug, Clone, PartialEq)]
pub enum Refresh {
    Nothing,
    /// Watch state or ratings changed: every view refetches (via a revision).
    WatchState,
    /// Playlist contents or the playlist list changed.
    Playlists,
    /// Reload the columns showing these paths.
    Paths(Vec<String>),
    /// Reload the columns showing this source.
    Source(Source),
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

    fn new_column(&mut self, title: impl Into<String>, source: Source) -> Column {
        let id = self.next_id;
        self.next_id += 1;
        let mut c = Column::new(id, title, source);
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
        let mut root = self.new_column(self.server_name().to_string(), Source::Root);
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
            Item::nav("Continue Watching", Source::Continue),
            Item::nav("Recently Added", Source::Recent),
            Item::nav("Watchlist", Source::Watchlist),
            Item::nav("Playlists", Source::Path(PLAYLISTS.into())),
            Item::nav("Now Playing", Source::Sessions),
            Item::nav("History", Source::History),
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
        self.search_tick(now);
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
}
