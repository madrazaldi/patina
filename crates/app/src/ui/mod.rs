pub mod components;
pub mod icons;
mod pages;
pub mod tracks;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::config;
use crate::fmt;
use crate::session::{self, Stage};
use crate::state::{AppState, use_app};
use components::{Img, ItemMenu, NowPlayingBar, PlaylistDialog, Spinner, Toasts};
use icons::{Ic, Icon, section_icon};
use pages::*;

#[derive(Routable, Clone, PartialEq, Debug)]
#[rustfmt::skip]
pub enum Route {
    #[layout(Shell)]
        #[route("/")]
        HomePage {},
        #[route("/library/:id")]
        LibraryPage { id: String },
        #[route("/item/:rating_key")]
        ItemPage { rating_key: String },
        #[route("/browse/:path/:title")]
        BrowsePage { path: String, title: String },
        #[route("/search")]
        SearchPage {},
        #[route("/playlists")]
        PlaylistsPage {},
        #[route("/playlist/:id")]
        PlaylistPage { id: String },
        #[route("/watchlist")]
        WatchlistPage {},
        #[route("/activity")]
        ActivityPage {},
        #[route("/server")]
        ServerPage {},
        #[route("/settings")]
        SettingsPage {},
}

#[component]
pub fn App() -> Element {
    let app = use_context_provider(AppState::new);
    use_hook(move || spawn_forever(session::restore(app)));

    let settings = app.settings.read();
    let accent = config::accent(&settings.ui.accent);
    let style = format!(
        "--accent: {}; --accent-rgb: {}; --poster-w: {}px;",
        accent.color, accent.rgb, settings.ui.poster_size
    );
    drop(settings);

    let stage = app.stage.read().clone();
    rsx! {
        div { style: "{style}",
            match stage {
                Stage::Ready => rsx! { Router::<Route> {} },
                Stage::Booting => rsx! { Splash { text: "Starting…" } },
                Stage::Connecting(name) => rsx! { Splash { text: "Connecting to {name}…" } },
                Stage::Login => rsx! { pages::login::Login {} },
                Stage::PickServer { servers, user } => rsx! { pages::login::PickServer { servers, user: *user } },
                Stage::Failed(message) => rsx! { pages::login::Failed { message } },
            }
            Toasts {}
        }
    }
}

#[component]
fn Splash(text: String) -> Element {
    rsx! {
        div { class: "login",
            div { class: "login-bg" }
            div { class: "login-card",
                div { class: "brand", div { class: "brand-mark" } "patina" }
                div { class: "center", style: "min-height: 120px", div { class: "spinner" } }
                p { "{text}" }
            }
        }
    }
}

#[component]
fn Shell() -> Element {
    let app = use_app();
    let route = use_route::<Route>();
    let nav = use_navigator();
    let mut scrolled = use_signal(|| false);
    let mut user_menu = use_signal(|| false);
    let scroll = use_context_provider(|| ScrollState(Signal::new((0.0, 0.0, 0.0))));

    // Debug builds: PATINA_ROUTE=/item/100 opens a page, PATINA_PLAY=<ratingKey> starts playback.
    #[cfg(debug_assertions)]
    use_hook(move || {
        if let Some(route) = std::env::var("PATINA_ROUTE").ok().and_then(|r| r.parse::<Route>().ok()) {
            nav.push(route);
        }
        if let Ok(rk) = std::env::var("PATINA_PLAY") {
            spawn(async move {
                if let Ok(Some(item)) = app.server().metadata(&rk, patina_plex::MetadataOptions::full()).await {
                    crate::player::play(app, crate::player::PlayRequest::new(item));
                }
            });
        }
    });

    // Scroll to top whenever the page changes.
    use_effect(use_reactive((&route,), |_| {
        document::eval("const c = document.getElementById('content'); if (c) { c.style.scrollBehavior = 'auto'; c.scrollTop = 0; c.style.scrollBehavior = ''; }");
    }));

    let Some(session) = app.session.read().clone() else {
        return rsx! { Spinner { fill: true } };
    };
    let sections = app.sections.read().clone();
    let user_name = session.user.as_ref().map(|u| u.display_name().to_string()).unwrap_or_else(|| "Server owner".into());
    let user_thumb = session.user.as_ref().and_then(|u| u.thumb.clone());
    let is = |r: &Route| &route == r;
    let lib_active = |id: &str| matches!(&route, Route::LibraryPage { id: x } if x == id);
    let has_tv = session.tv.is_some();

    rsx! {
        div { class: "app",
            nav { class: "sidebar",
                div { class: "brand", div { class: "brand-mark" } div { "patina" small { "for Plex" } } }
                NavLink { to: Route::HomePage {}, icon: Ic::Home, label: "Home", active: is(&Route::HomePage {}) }
                NavLink { to: Route::SearchPage {}, icon: Ic::Search, label: "Search", active: is(&Route::SearchPage {}) }
                if has_tv {
                    NavLink { to: Route::WatchlistPage {}, icon: Ic::Bookmark, label: "Watchlist", active: is(&Route::WatchlistPage {}) }
                }
                NavLink { to: Route::PlaylistsPage {}, icon: Ic::List, label: "Playlists", active: is(&Route::PlaylistsPage {}) }

                div { class: "nav-label", "Libraries" }
                for s in sections {
                    NavLink {
                        key: "{s.key:?}",
                        to: Route::LibraryPage { id: s.key.clone().unwrap_or_default() },
                        icon: section_icon(s.kind.as_deref().unwrap_or("")),
                        label: s.title().to_string(),
                        active: lib_active(s.key.as_deref().unwrap_or("")),
                    }
                }

                div { class: "nav-spacer" }
                NavLink { to: Route::ActivityPage {}, icon: Ic::Activity, label: "Activity", active: is(&Route::ActivityPage {}) }
                if session.owned {
                    NavLink { to: Route::ServerPage {}, icon: Ic::Server, label: "Server", active: is(&Route::ServerPage {}) }
                }
                NavLink { to: Route::SettingsPage {}, icon: Ic::Sliders, label: "Settings", active: is(&Route::SettingsPage {}) }

                div { class: "user-chip", onclick: move |_| user_menu.toggle(),
                    div { class: "avatar",
                        if user_thumb.is_some() { Img { src: user_thumb.clone() } } else { "{fmt::initials(&user_name)}" }
                    }
                    div { class: "who", b { "{user_name}" } span { "{session.server_name}" } }
                    Icon { name: Ic::ChevronDown, class: "sm" }
                }
            }

            main { class: "main",
                div { class: if scrolled() { "topbar scrolled" } else { "topbar" },
                    button { class: "icon-btn", title: "Back", disabled: !nav.can_go_back(), onclick: move |_| nav.go_back(), Icon { name: Ic::ChevronLeft } }
                    button { class: "icon-btn", title: "Forward", disabled: !nav.can_go_forward(), onclick: move |_| nav.go_forward(), Icon { name: Ic::ChevronRight } }
                    SearchBox {}
                }
                div {
                    id: "content",
                    class: "content",
                    onscroll: move |e| {
                        let top = e.data().scroll_top() > 12.0;
                        if top != *scrolled.peek() { scrolled.set(top); }
                        let mut s = scroll.0;
                        s.set((e.data().scroll_top(), e.data().scroll_height() as f64, e.data().client_height() as f64));
                    },
                    Outlet::<Route> {}
                }
            }
            NowPlayingBar {}
        }
        if user_menu() {
            UserMenu { on_close: move |_| user_menu.set(false) }
        }
        ItemMenu {}
        PlaylistDialog {}
        tracks::TrackDialog {}
    }
}

/// Scroll position of the main content area: (top, scroll height, client height).
#[derive(Clone, Copy)]
pub struct ScrollState(pub Signal<(f64, f64, f64)>);

pub fn use_scroll() -> Signal<(f64, f64, f64)> {
    use_context::<ScrollState>().0
}

#[component]
fn NavLink(to: Route, icon: Ic, label: String, active: bool) -> Element {
    let nav = use_navigator();
    rsx! {
        div { class: if active { "nav-item active" } else { "nav-item" }, title: "{label}",
            onclick: move |_| { nav.push(to.clone()); },
            Icon { name: icon }
            span { "{label}" }
        }
    }
}

#[component]
fn SearchBox() -> Element {
    let app = use_app();
    let nav = use_navigator();
    let route = use_route::<Route>();
    let mut search = app.search;
    let on_search = matches!(route, Route::SearchPage {});
    rsx! {
        label { class: "search-box",
            Icon { name: Ic::Search, class: "sm" }
            input {
                id: "search-input",
                placeholder: "Search movies, shows, people…",
                value: "{search}",
                oninput: move |e| {
                    search.set(e.value());
                    if !on_search { nav.push(Route::SearchPage {}); }
                },
                onkeydown: move |e: KeyboardEvent| {
                    if e.key() == Key::Escape { search.set(String::new()); }
                },
            }
            kbd { "/" }
        }
    }
}

#[component]
fn UserMenu(on_close: EventHandler<()>) -> Element {
    let app = use_app();
    let nav = use_navigator();
    let session = app.session.read().clone();
    let servers = session.as_ref().map(|s| s.servers.clone()).unwrap_or_default();
    let current = session.as_ref().map(|s| s.server_id.clone()).unwrap_or_default();
    rsx! {
        div { class: "scrim", onclick: move |_| on_close.call(()) }
        div { class: "menu", style: "left: 14px; bottom: 84px; width: 216px;",
            if servers.len() > 1 {
                div { class: "menu-head", span { "Switch server" } }
                for s in servers {
                    div { class: "menu-item", key: "{s.id()}",
                        onclick: {
                            let s = s.clone();
                            move |_| {
                                on_close.call(());
                                nav.push(Route::HomePage {});
                                spawn_forever(session::switch_server(app, s.clone()));
                            }
                        },
                        Icon { name: if s.id() == current { Ic::CheckCircle } else { Ic::Server }, class: "sm" }
                        "{s.name()}"
                    }
                }
                div { class: "menu-sep" }
            }
            div { class: "menu-item", onclick: move |_| { on_close.call(()); nav.push(Route::SettingsPage {}); },
                Icon { name: Ic::Users, class: "sm" } "Switch user…"
            }
            div { class: "menu-item danger", onclick: move |_| { on_close.call(()); session::sign_out(app); },
                Icon { name: Ic::LogOut, class: "sm" } "Sign out"
            }
        }
    }
}

pub fn head() -> String {
    format!(
        r#"<meta name="viewport" content="width=device-width, initial-scale=1">
<style>{}</style>
<script>
document.addEventListener('keydown', (e) => {{
  const t = e.target;
  const typing = t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT');
  if ((e.key === '/' && !typing) || ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') || ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'f')) {{
    const s = document.getElementById('search-input');
    if (s) {{ e.preventDefault(); s.focus(); s.select(); }}
  }}
}});
</script>"#,
        include_str!("../../assets/style.css")
    )
}
