//! In-app navigation with back/forward history. Each history entry keeps its
//! page view alive (up to [`KEEP_VIEWS`] of them), so going back lands on the
//! page as it was left: same scroll position, filters and loaded data.

use gpui_kit::*;
use patina_plex::Metadata;

/// Page views kept alive in the history; older entries are rebuilt when revisited.
const KEEP_VIEWS: usize = 24;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    Home,
    Library { id: String },
    /// `server` is the machine id of another server of the account the item lives on.
    Item { rating_key: String, server: Option<String> },
    /// A server listing, e.g. a genre or an actor's filmography.
    Browse { path: String, title: String },
    Search,
    Pick,
    Playlists,
    Playlist { id: String },
    Watchlist,
    Downloads,
    LiveTv,
    Activity,
    Server,
    Settings,
    Requests,
    /// A title on Seerr; `media_type` is `movie` or `tv`.
    Seerr { media_type: String, tmdb_id: i64 },
}

impl Route {
    /// Where clicking an item should go.
    pub fn for_item(item: &Metadata) -> Self {
        if item.kind() == "playlist" {
            Self::Playlist { id: item.rating_key().to_string() }
        } else {
            Self::Item { rating_key: item.rating_key().to_string(), server: item.server_id().map(str::to_string) }
        }
    }

    pub fn browse(path: impl Into<String>, title: impl Into<String>) -> Self {
        Self::Browse { path: path.into(), title: title.into() }
    }

    /// "/item/100" style paths (`PATINA_ROUTE` in debug builds).
    pub fn parse(path: &str) -> Option<Self> {
        let parts: Vec<&str> = path.split('/').collect();
        let a = parts.get(1).copied().unwrap_or("");
        let b = parts.get(2).copied().filter(|s| !s.is_empty()).map(str::to_string);
        Some(match a {
            "" => Self::Home,
            "library" => Self::Library { id: b? },
            "item" => Self::Item { rating_key: b?, server: None },
            "playlist" => Self::Playlist { id: b? },
            "seerr" => {
                let media_type = b.filter(|t| t == "movie" || t == "tv")?;
                let tmdb_id = parts.get(3)?.parse().ok().filter(|n| *n > 0)?;
                Self::Seerr { media_type, tmdb_id }
            }
            "search" => Self::Search,
            "pick" => Self::Pick,
            "playlists" => Self::Playlists,
            "watchlist" => Self::Watchlist,
            "downloads" => Self::Downloads,
            "livetv" => Self::LiveTv,
            "activity" => Self::Activity,
            "server" => Self::Server,
            "settings" => Self::Settings,
            "requests" => Self::Requests,
            _ => return None,
        })
    }
}

pub struct Entry {
    pub route: Route,
    /// The page, once built; dropped for old entries and rebuilt on demand.
    pub view: Option<AnyView>,
}

pub struct Nav {
    stack: Vec<Entry>,
    index: usize,
}

pub struct NavHandle(pub Entity<Nav>);

impl Global for NavHandle {}

pub fn init(cx: &mut App) {
    let nav = cx.new(|_| Nav { stack: vec![Entry { route: Route::Home, view: None }], index: 0 });
    cx.set_global(NavHandle(nav));
}

pub fn nav(cx: &App) -> Entity<Nav> {
    cx.global::<NavHandle>().0.clone()
}

/// Go to a page.
pub fn push(route: Route, cx: &mut App) {
    nav(cx).update(cx, |n, cx| n.push(route, cx));
}

pub fn back(cx: &mut App) {
    nav(cx).update(cx, |n, cx| n.back(cx));
}

pub fn forward(cx: &mut App) {
    nav(cx).update(cx, |n, cx| n.forward(cx));
}

/// Whether `route` is the current page.
pub fn is(route: &Route, cx: &App) -> bool {
    nav(cx).read(cx).route() == route
}

impl Nav {
    pub fn route(&self) -> &Route {
        &self.stack[self.index].route
    }

    pub fn current(&mut self) -> &mut Entry {
        &mut self.stack[self.index]
    }

    pub fn current_ref(&self) -> &Entry {
        &self.stack[self.index]
    }

    pub fn can_back(&self) -> bool {
        self.index > 0
    }

    pub fn can_forward(&self) -> bool {
        self.index + 1 < self.stack.len()
    }

    pub fn push(&mut self, route: Route, cx: &mut Context<Self>) {
        if *self.route() == route {
            return;
        }
        self.stack.truncate(self.index + 1);
        self.stack.push(Entry { route, view: None });
        self.index = self.stack.len() - 1;
        self.trim();
        cx.notify();
    }

    /// Replace the current page (e.g. a redirect).
    pub fn replace(&mut self, route: Route, cx: &mut Context<Self>) {
        self.stack[self.index] = Entry { route, view: None };
        cx.notify();
    }

    pub fn back(&mut self, cx: &mut Context<Self>) {
        if self.can_back() {
            self.index -= 1;
            cx.notify();
        }
    }

    pub fn forward(&mut self, cx: &mut Context<Self>) {
        if self.can_forward() {
            self.index += 1;
            cx.notify();
        }
    }

    /// Forget history (e.g. after switching servers).
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.stack = vec![Entry { route: Route::Home, view: None }];
        self.index = 0;
        cx.notify();
    }

    fn trim(&mut self) {
        let keep_from = self.stack.len().saturating_sub(KEEP_VIEWS);
        for e in &mut self.stack[..keep_from] {
            e.view = None;
        }
        if self.stack.len() > 200 {
            let drop = self.stack.len() - 200;
            self.stack.drain(..drop);
            self.index -= drop;
        }
    }
}
