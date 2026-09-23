use std::sync::atomic::{AtomicU64, Ordering};

use dioxus::core::spawn_forever;
use dioxus::prelude::*;
use patina_plex::{Hub, Metadata};

use super::icons::{Ic, Icon};
use super::Route;
use crate::fmt;
use crate::images;
use crate::player::{self, Control, PlayRequest};
use crate::state::{AppState, MenuState, use_app};

static IDS: AtomicU64 = AtomicU64::new(1);

pub fn unique_id(prefix: &str) -> String {
    format!("{prefix}-{}", IDS.fetch_add(1, Ordering::Relaxed))
}

/// Where clicking an item should navigate.
pub fn route_for(item: &Metadata) -> Route {
    match item.kind() {
        "playlist" => Route::PlaylistPage { id: item.rating_key().to_string() },
        _ => Route::ItemPage { rating_key: item.rating_key().to_string() },
    }
}

pub fn browse(key: &str, title: &str) -> Route {
    Route::BrowsePage { path: fmt::hex_encode(key), title: fmt::hex_encode(title) }
}

/// Image that fades in once loaded.
#[component]
pub fn Img(src: Option<String>, #[props(default)] class: String) -> Element {
    let mut loaded = use_signal(|| false);
    let Some(src) = src else { return rsx! {} };
    let cls = if loaded() { format!("{class} loaded") } else { class };
    rsx! {
        img {
            class: "{cls}",
            src: "{src}",
            loading: "lazy",
            decoding: "async",
            draggable: "false",
            onload: move |_| loaded.set(true),
        }
    }
}

#[component]
pub fn Spinner(#[props(default)] fill: bool) -> Element {
    rsx! {
        div { class: if fill { "center fill" } else { "center" }, div { class: "spinner" } }
    }
}

#[component]
pub fn ErrorView(message: String) -> Element {
    rsx! {
        div { class: "empty",
            Icon { name: Ic::Alert }
            h3 { "Something went wrong" }
            div { class: "small", "{message}" }
        }
    }
}

#[component]
pub fn Empty(icon: Ic, title: String, #[props(default)] text: String) -> Element {
    rsx! {
        div { class: "empty",
            Icon { name: icon }
            h3 { "{title}" }
            if !text.is_empty() { div { "{text}" } }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Shape {
    #[default]
    Poster,
    Landscape,
    Square,
}

impl Shape {
    pub fn for_item(item: &Metadata) -> Shape {
        match item.kind() {
            "episode" | "clip" => Shape::Landscape,
            "album" | "artist" | "track" | "playlist" | "photo" | "photoalbum" => Shape::Square,
            _ => Shape::Poster,
        }
    }

    fn class(self) -> &'static str {
        match self {
            Shape::Poster => "card",
            Shape::Landscape => "card landscape",
            Shape::Square => "card square",
        }
    }

    fn size(self) -> (u32, u32) {
        match self {
            Shape::Poster => (300, 450),
            Shape::Landscape => (560, 315),
            Shape::Square => (360, 360),
        }
    }
}

pub fn open_menu(app: AppState, item: &Metadata, x: f64, y: f64) {
    let mut menu = app.menu;
    menu.set(Some(MenuState { item: item.clone(), x, y }));
}

/// Poster / thumbnail card for any metadata item.
#[component]
pub fn MediaCard(
    item: Metadata,
    #[props(default)] shape: Option<Shape>,
    /// Use backdrop art for landscape cards ("Continue Watching" style).
    #[props(default)]
    use_art: bool,
) -> Element {
    let app = use_app();
    let nav = use_navigator();
    let shape = shape.unwrap_or_else(|| Shape::for_item(&item));
    let (w, h) = shape.size();
    let source = match shape {
        Shape::Landscape if use_art && item.kind() != "episode" => item.backdrop().or(item.thumb.as_deref()),
        Shape::Landscape => item.thumb.as_deref().or(item.backdrop()),
        _ => item.poster(),
    };
    let src = images::opt(source, w, h);
    let (title, sub) = if item.kind() == "episode" && use_art {
        (
            item.grandparent_title.clone().unwrap_or_default(),
            format!("{} · {}", item.episode_code(), item.title()),
        )
    } else {
        (fmt::card_title(&item), fmt::subtitle(&item))
    };
    let unwatched = if matches!(item.kind(), "show" | "season") { item.unwatched_leaves().filter(|n| *n > 0) } else { None };
    let watched = !matches!(item.kind(), "playlist" | "collection" | "artist" | "album" | "photo" | "photoalbum")
        && unwatched.is_none()
        && item.is_watched();
    let progress = item.progress();
    let can_play = item.is_playable() || item.is_container();
    let spoiler = app.settings.read().ui.hide_spoilers && item.kind() == "episode" && !item.is_watched();

    let target = route_for(&item);
    let for_play = item.clone();
    let for_menu = item.clone();
    let for_ctx = item.clone();
    let kind_icon = super::icons::section_icon(match item.kind() {
        "episode" | "season" => "show",
        "track" | "album" | "artist" => "artist",
        "photo" | "photoalbum" => "photo",
        _ => "movie",
    });

    rsx! {
        div {
            class: "{shape.class()}",
            onclick: move |_| { nav.push(target.clone()); },
            oncontextmenu: move |e| {
                e.prevent_default();
                let p = e.client_coordinates();
                open_menu(app, &for_ctx, p.x, p.y);
            },
            div { class: "card-art",
                if src.is_some() {
                    Img { src: src.clone(), class: if spoiler { "spoiler" } else { "" } }
                } else {
                    div { class: "placeholder", Icon { name: kind_icon } }
                }
                if let Some(n) = unwatched {
                    div { class: "card-badge", "{n}" }
                } else if watched {
                    div { class: "card-badge watched", Icon { name: Ic::Check } }
                }
                if let Some(p) = progress {
                    div { class: "progress", i { style: "width: {p * 100.0:.1}%" } }
                }
                if can_play {
                    button {
                        class: "card-play",
                        title: "Play",
                        onclick: move |e| {
                            e.stop_propagation();
                            player::play(app, PlayRequest::new(for_play.clone()));
                        },
                        Icon { name: Ic::Play }
                    }
                }
                button {
                    class: "card-more",
                    title: "More",
                    onclick: move |e| {
                        e.stop_propagation();
                        let p = e.client_coordinates();
                        open_menu(app, &for_menu, p.x, p.y);
                    },
                    Icon { name: Ic::More, class: "sm" }
                }
            }
            div { class: "card-title", title: "{title}", "{title}" }
            if !sub.is_empty() { div { class: "card-sub", "{sub}" } }
        }
    }
}

/// Horizontally scrolling row of cards with arrow buttons.
#[component]
pub fn Row(
    title: String,
    items: Vec<Metadata>,
    #[props(default)] shape: Option<Shape>,
    #[props(default)] use_art: bool,
    #[props(default)] more: Option<Route>,
) -> Element {
    let id = use_hook(|| unique_id("row"));
    let nav = use_navigator();
    if items.is_empty() {
        return rsx! {};
    }
    let row_id = id.clone();
    let scroll = move |dir: i32| {
        let id = row_id.clone();
        move |_| {
            document::eval(&format!(
                "document.getElementById('{id}').scrollBy({{left: {dir} * window.innerWidth * 0.62, behavior: 'smooth'}})"
            ));
        }
    };
    rsx! {
        div { class: "section-title",
            h2 { "{title}" }
            if let Some(route) = more {
                span { class: "more", onclick: move |_| { nav.push(route.clone()); }, "See all", Icon { name: Ic::ChevronRight, class: "sm" } }
            }
        }
        div { class: "row-scroller",
            button { class: "row-nav left", onclick: scroll(-1), Icon { name: Ic::ChevronLeft } }
            div { class: "row", id: "{id}",
                for item in items {
                    MediaCard { key: "{item.rating_key():?}{item.key:?}", item: item.clone(), shape, use_art }
                }
            }
            button { class: "row-nav right", onclick: scroll(1), Icon { name: Ic::ChevronRight } }
        }
    }
}

/// A Plex hub rendered as a row.
#[component]
pub fn HubRow(hub: Hub) -> Element {
    let ident = hub.hub_identifier.clone().unwrap_or_default().to_lowercase();
    let continue_style = ident.contains("continue") || ident.contains("ondeck") || ident.contains("inprogress");
    let shape = (continue_style || matches!(hub.kind.as_deref(), Some("clip" | "episode"))).then_some(Shape::Landscape);
    let title = hub.title.clone().unwrap_or_default();
    let more = (hub.more && hub.key.is_some()).then(|| browse(hub.key.as_deref().unwrap_or(""), &title));
    rsx! {
        Row { title, items: hub.metadata.clone(), shape, use_art: continue_style, more }
    }
}

/// Right-click / "…" menu for any item.
#[component]
pub fn ItemMenu() -> Element {
    let app = use_app();
    let nav = use_navigator();
    let mut menu = app.menu;
    let Some(state) = menu() else { return rsx! {} };
    let item = state.item.clone();

    let (win_w, win_h) = {
        let w = dioxus::desktop::window();
        let scale = w.window.scale_factor();
        let size = w.window.inner_size();
        (size.width as f64 / scale, size.height as f64 / scale)
    };
    let x = state.x.min(win_w - 250.0).max(8.0);
    let y = state.y.min(win_h - 380.0).max(8.0);

    let session = app.session.read().clone();
    let owned = session.as_ref().map(|s| s.owned).unwrap_or(false);
    let has_tv = session.as_ref().and_then(|s| s.tv.as_ref()).is_some();
    let rk = item.rating_key().to_string();
    let playable = item.is_playable() || item.is_container();
    let watchable = !matches!(item.kind(), "playlist" | "photo" | "photoalbum" | "artist" | "album" | "track");
    let watched = item.is_watched();

    let close = move || {
        let mut m = app.menu;
        m.set(None);
    };

    let play_item = item.clone();
    let resume_item = item.clone();
    let shuffle_item = item.clone();
    let show_key = item.grandparent_rating_key.clone();
    let season_key = item.parent_rating_key.clone();
    let playlist_item = item.clone();
    let watchlist_id = item.discover_id().map(str::to_string);
    let web_url = session.as_ref().and_then(|s| s.server.machine_identifier().map(|m| {
        format!("https://app.plex.tv/desktop#!/server/{m}/details?key=%2Flibrary%2Fmetadata%2F{rk}")
    }));
    let has_progress = item.progress().is_some();
    let is_container = item.is_container();
    let title = item.full_title();
    let sub = fmt::subtitle(&item);
    let kind = item.kind().to_string();

    rsx! {
        div { class: "scrim", onclick: move |_| close(), oncontextmenu: move |e| { e.prevent_default(); close(); } }
        div { class: "menu", style: "left: {x}px; top: {y}px;",
            div { class: "menu-head", b { "{title}" } span { "{sub}" } }
            if playable {
                div { class: "menu-item", onclick: move |_| {
                        menu.set(None);
                        player::play(app, PlayRequest::new(play_item.clone()));
                    },
                    Icon { name: Ic::Play, class: "sm" }
                    if has_progress { "Resume" } else { "Play" }
                }
                if has_progress {
                    div { class: "menu-item", onclick: move |_| {
                            menu.set(None);
                            player::play(app, PlayRequest { resume: false, ..PlayRequest::new(resume_item.clone()) });
                        },
                        Icon { name: Ic::Restart, class: "sm" } "Play from beginning"
                    }
                }
                if is_container {
                    div { class: "menu-item", onclick: move |_| {
                            menu.set(None);
                            player::play(app, PlayRequest { shuffle: true, ..PlayRequest::new(shuffle_item.clone()) });
                        },
                        Icon { name: Ic::Shuffle, class: "sm" } "Shuffle"
                    }
                }
                div { class: "menu-sep" }
            }
            if let Some(show) = show_key.clone() {
                if kind == "episode" || kind == "track" {
                    div { class: "menu-item", onclick: move |_| { menu.set(None); nav.push(Route::ItemPage { rating_key: show.clone() }); },
                        Icon { name: if kind == "track" { Ic::User } else { Ic::Tv }, class: "sm" }
                        if kind == "track" { "Go to artist" } else { "Go to show" }
                    }
                }
            }
            if let Some(season) = season_key.clone() {
                if kind == "episode" || kind == "track" || kind == "season" {
                    div { class: "menu-item", onclick: move |_| { menu.set(None); nav.push(Route::ItemPage { rating_key: season.clone() }); },
                        Icon { name: Ic::Layers, class: "sm" }
                        match kind.as_str() { "track" => "Go to album", "season" => "Go to show", _ => "Go to season" }
                    }
                }
            }
            if watchable {
                div { class: "menu-item", onclick: {
                        let rk = rk.clone();
                        move |_| {
                            menu.set(None);
                            let rk = rk.clone();
                            spawn_forever(async move {
                                let server = app.server();
                                let r = if watched { server.unscrobble(&rk).await } else { server.scrobble(&rk).await };
                                match r {
                                    Ok(()) => app.toast(if watched { "Marked as unwatched" } else { "Marked as watched" }),
                                    Err(e) => app.error(e),
                                }
                                app.bump();
                            });
                        }
                    },
                    Icon { name: if watched { Ic::Circle } else { Ic::CheckCircle }, class: "sm" }
                    if watched { "Mark as unwatched" } else { "Mark as watched" }
                }
            }
            if kind != "playlist" && kind != "collection" {
                div { class: "menu-item", onclick: move |_| {
                        menu.set(None);
                        let mut target = app.playlist_target;
                        target.set(Some(playlist_item.clone()));
                    },
                    Icon { name: Ic::List, class: "sm" } "Add to playlist…"
                }
            }
            if has_tv {
                if let Some(id) = watchlist_id.clone() {
                    div { class: "menu-item", onclick: move |_| {
                            menu.set(None);
                            let id = id.clone();
                            spawn_forever(async move {
                                let Some(tv) = app.tv() else { return };
                                match tv.add_to_watchlist(&id).await {
                                    Ok(()) => app.toast("Added to watchlist"),
                                    Err(e) => app.error(e),
                                }
                            });
                        },
                        Icon { name: Ic::Bookmark, class: "sm" } "Add to watchlist"
                    }
                }
            }
            if owned && kind != "playlist" {
                div { class: "menu-sep" }
                div { class: "menu-item", onclick: {
                        let rk = rk.clone();
                        move |_| {
                            menu.set(None);
                            let rk = rk.clone();
                            spawn_forever(async move {
                                match app.server().refresh_metadata(&rk, true).await {
                                    Ok(()) => app.toast("Refreshing metadata…"),
                                    Err(e) => app.error(e),
                                }
                            });
                        }
                    },
                    Icon { name: Ic::Refresh, class: "sm" } "Refresh metadata"
                }
                div { class: "menu-item", onclick: {
                        let rk = rk.clone();
                        move |_| {
                            menu.set(None);
                            let rk = rk.clone();
                            spawn_forever(async move {
                                match app.server().analyze_metadata(&rk).await {
                                    Ok(()) => app.toast("Analyzing…"),
                                    Err(e) => app.error(e),
                                }
                            });
                        }
                    },
                    Icon { name: Ic::Sparkles, class: "sm" } "Analyze"
                }
            }
            if let Some(url) = web_url {
                div { class: "menu-item", onclick: move |_| { menu.set(None); let _ = open::that(&url); },
                    Icon { name: Ic::ExternalLink, class: "sm" } "Open in Plex Web"
                }
            }
        }
    }
}

fn playlist_kind(item: &Metadata) -> &'static str {
    match item.kind() {
        "track" | "album" | "artist" => "audio",
        "photo" | "photoalbum" => "photo",
        _ => "video",
    }
}

fn add_to_playlist(app: AppState, rk: String, playlist: Option<(String, String)>, new_title: String, kind: &'static str) {
    let mut target = app.playlist_target;
    target.set(None);
    spawn_forever(async move {
        let server = app.server();
        let uri = server.item_uri(&[&rk]);
        let result = match &playlist {
            Some((id, _)) => server.add_to_playlist(id, &uri).await,
            None => server.create_playlist(&new_title, kind, &uri, false).await.map(|_| ()),
        };
        match (result, playlist) {
            (Ok(()), Some((_, name))) => app.toast(format!("Added to {name}")),
            (Ok(()), None) => app.toast(format!("Created playlist {new_title}")),
            (Err(e), _) => app.error(e),
        }
    });
}

/// "Add to playlist" dialog.
#[component]
pub fn PlaylistDialog() -> Element {
    let app = use_app();
    let mut target = app.playlist_target;
    let mut new_name = use_signal(String::new);
    let playlists = use_resource(move || {
        let item = target();
        async move {
            match item {
                Some(item) => app.server().playlists(Some(playlist_kind(&item)), Some(false)).await,
                None => Ok(Vec::new()),
            }
        }
    });
    let Some(item) = target() else { return rsx! {} };
    let kind = playlist_kind(&item);
    let rk = item.rating_key().to_string();
    let title = item.full_title();
    let rk_enter = rk.clone();
    let rk_click = rk.clone();

    rsx! {
        div { class: "modal-backdrop", onclick: move |_| target.set(None),
            div { class: "modal", onclick: move |e| e.stop_propagation(),
                h2 { "Add to playlist" }
                p { class: "desc", "{title}" }
                match &*playlists.read() {
                    None => rsx! { Spinner {} },
                    Some(Err(e)) => rsx! { div { class: "error-box", "{e}" } },
                    Some(Ok(list)) if list.is_empty() => rsx! { p { class: "muted", "No playlists yet — create one below." } },
                    Some(Ok(list)) => rsx! {
                        div { class: "list",
                            for p in list.clone() {
                                div {
                                    class: "list-row",
                                    key: "{p.rating_key():?}",
                                    onclick: {
                                        let rk = rk.clone();
                                        let pick = (p.rating_key().to_string(), p.title().to_string());
                                        move |_| add_to_playlist(app, rk.clone(), Some(pick.clone()), String::new(), kind)
                                    },
                                    div { class: "thumb", Img { src: images::opt(p.composite.as_deref().or(p.thumb.as_deref()), 120, 120) } }
                                    div { class: "grow", div { class: "t", "{p.title()}" } div { class: "s", "{fmt::subtitle(&p)}" } }
                                    div { class: "end", Icon { name: Ic::Plus, class: "sm" } }
                                }
                            }
                        }
                    },
                }
                div { class: "divider", "or" }
                div { class: "flex",
                    input {
                        class: "input grow",
                        placeholder: "New playlist name",
                        value: "{new_name}",
                        oninput: move |e| new_name.set(e.value()),
                        onkeydown: move |e: KeyboardEvent| {
                            let name = new_name().trim().to_string();
                            if e.key() == Key::Enter && !name.is_empty() {
                                add_to_playlist(app, rk_enter.clone(), None, name, kind);
                            }
                        },
                    }
                    button {
                        class: "btn primary",
                        disabled: new_name().trim().is_empty(),
                        onclick: move |_| add_to_playlist(app, rk_click.clone(), None, new_name().trim().to_string(), kind),
                        Icon { name: Ic::Plus, class: "sm" }
                        "Create"
                    }
                }
                div { class: "foot", button { class: "btn ghost", onclick: move |_| target.set(None), "Cancel" } }
            }
        }
    }
}

#[component]
pub fn Toasts() -> Element {
    let app = use_app();
    rsx! {
        div { class: "toasts",
            for t in app.toasts.read().iter() {
                div { key: "{t.id}", class: if t.error { "toast error" } else { "toast" },
                    Icon { name: if t.error { Ic::Alert } else { Ic::CheckCircle } }
                    span { "{t.text}" }
                }
            }
        }
    }
}

#[component]
pub fn NowPlayingBar() -> Element {
    let app = use_app();
    let nav = use_navigator();
    let Some(np) = app.now_playing.read().clone() else { return rsx! {} };
    let item = np.item.clone();
    let pct = if np.duration_ms > 0 { np.position_ms as f64 / np.duration_ms as f64 * 100.0 } else { 0.0 };
    let art = images::opt(if item.kind() == "episode" { item.thumb.as_deref() } else { item.poster() }, 120, 120);
    let (title, sub) = match item.kind() {
        "episode" => (item.grandparent_title.clone().unwrap_or_default(), format!("{} · {}", item.episode_code(), item.title())),
        "track" => (item.title().to_string(), item.grandparent_title.clone().unwrap_or_default()),
        _ => (item.title().to_string(), item.year.map(|y| y.to_string()).unwrap_or_default()),
    };
    let target = route_for(&item);
    rsx! {
        div { class: "now-playing",
            div { class: "np-progress", i { style: "width: {pct:.2}%" } }
            div { class: "np-info", onclick: move |_| { nav.push(target.clone()); },
                div { class: "art", Img { src: art } }
                div { class: "grow", b { "{title}" } span { "{sub}" } }
            }
            div { class: "np-controls",
                button { class: "icon-btn", title: "Previous", disabled: np.queue_pos == 0, onclick: |_| player::control(Control::Previous), Icon { name: Ic::SkipBack, class: "sm" } }
                button { class: "icon-btn", title: "Back 10s", onclick: |_| player::control(Control::SeekRelative(-10.0)), Icon { name: Ic::Restart, class: "sm" } }
                button { class: "icon-btn play", title: "Play/Pause", onclick: |_| player::control(Control::TogglePause),
                    Icon { name: if np.paused { Ic::Play } else { Ic::Pause } }
                }
                button { class: "icon-btn", title: "Stop", onclick: |_| player::control(Control::Stop), Icon { name: Ic::Square, class: "sm" } }
                button { class: "icon-btn", title: "Next", disabled: np.queue_pos + 1 >= np.queue_len, onclick: |_| player::control(Control::Next), Icon { name: Ic::SkipForward, class: "sm" } }
            }
            div { class: "np-right",
                if np.transcoding { span { class: "badge accent", "TRANSCODE" } } else { span { class: "badge", "DIRECT" } }
                if np.queue_len > 1 { span { "{np.queue_pos + 1} / {np.queue_len}" } }
                span { "{fmt::clock(np.position_ms)} / {fmt::clock(np.duration_ms)}" }
            }
        }
    }
}
