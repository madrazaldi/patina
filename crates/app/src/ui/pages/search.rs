use std::time::Duration;

use dioxus::prelude::*;
use patina_plex::{Directory, Hub};

use crate::fmt;
use crate::images;
use crate::state::use_app;
use crate::ui::components::{Empty, ErrorView, Img, MediaCard, Shape, Spinner, browse};
use crate::ui::icons::{Ic, Icon};

#[component]
pub fn SearchPage() -> Element {
    let app = use_app();
    use_effect(|| {
        document::eval("setTimeout(() => document.getElementById('search-input')?.focus(), 30)");
    });
    let results = use_resource(move || {
        let q = (app.search)().trim().to_string();
        async move {
            if q.len() < 2 {
                return (q, Ok(Vec::new()));
            }
            // Debounce: a newer keystroke restarts (and cancels) this future.
            tokio::time::sleep(Duration::from_millis(220)).await;
            let r = app.server().search_hubs(&q, 24, None).await;
            (q, r)
        }
    });
    let query = (app.search)().trim().to_string();

    rsx! {
        div { class: "page",
            div { class: "page-head",
                div {
                    h1 { if query.is_empty() { "Search" } else { "Results for “{query}”" } }
                    div { class: "sub", "Search across every library on this server. Press / anywhere to jump here." }
                }
            }
            if query.len() < 2 {
                Empty { icon: Ic::Search, title: "Find something to watch", text: "Movies, shows, episodes, music, actors, directors, genres…" }
            } else {
                match &*results.read() {
                    Some((q, Ok(hubs))) if *q == query => rsx! { Results { hubs: hubs.clone() } },
                    Some((q, Err(e))) if *q == query => rsx! { ErrorView { message: e.to_string() } },
                    _ => rsx! { Spinner {} },
                }
            }
        }
    }
}

#[component]
fn Results(hubs: Vec<Hub>) -> Element {
    let hubs: Vec<Hub> = hubs.into_iter().filter(|h| !h.is_empty()).collect();
    if hubs.is_empty() {
        return rsx! { Empty { icon: Ic::Search, title: "No results", text: "Check the spelling or try another word." } };
    }
    rsx! {
        for hub in hubs {
            div { key: "{hub.hub_identifier:?}",
                div { class: "section-title", h2 { "{hub.title.clone().unwrap_or_default()}" } }
                if !hub.metadata.is_empty() {
                    {
                        let landscape = matches!(hub.kind.as_deref(), Some("episode" | "clip"));
                        rsx! {
                            div { class: if landscape { "grid wide" } else { "grid" },
                                for m in hub.metadata.iter() {
                                    MediaCard { key: "{m.rating_key():?}", item: m.clone(), shape: landscape.then_some(Shape::Landscape) }
                                }
                            }
                        }
                    }
                }
                if !hub.directory.is_empty() {
                    div { class: "row", style: "padding: 6px 0 16px; flex-wrap: wrap",
                        for d in hub.directory.iter() {
                            TagCard { key: "{d.key:?}{d.tag:?}", dir: d.clone() }
                        }
                    }
                }
            }
        }
    }
}

/// People, genres and other tags from search results.
#[component]
fn TagCard(dir: Directory) -> Element {
    let nav = use_navigator();
    let name = dir.title().to_string();
    let face = images::opt(dir.thumb.as_deref(), 220, 220);
    let target = dir.key.clone().map(|k| browse(&k, &name));
    let person = matches!(dir.kind.as_deref(), Some("actor" | "director" | "writer" | "producer" | "person" | "tag"));
    if person {
        return rsx! {
            div { class: "person", onclick: move |_| if let Some(t) = &target { nav.push(t.clone()); },
                div { class: "face", if face.is_some() { Img { src: face } } else { "{fmt::initials(&name)}" } }
                b { "{name}" }
                if let Some(c) = dir.count { span { "{c} titles" } }
            }
        };
    }
    rsx! {
        div { class: "chip", onclick: move |_| if let Some(t) = &target { nav.push(t.clone()); },
            Icon { name: Ic::Layers, class: "sm" } "{name}"
        }
    }
}
