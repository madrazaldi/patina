use std::time::Duration;

use dioxus::prelude::*;
use patina_plex::{Hub, Metadata};

use crate::fmt;
use crate::images;
use crate::player::{self, PlayRequest};
use crate::state::use_app;
use crate::ui::components::{Empty, ErrorView, HubRow, Img, route_for};
use crate::ui::icons::{Ic, Icon};

#[component]
pub fn HomePage() -> Element {
    let app = use_app();
    let hubs = use_resource(move || {
        let _ = (app.revision)();
        async move { app.server().home_hubs(20).await }
    });

    let read = hubs.read();
    let hubs = match &*read {
        None => return rsx! { HomeSkeleton {} },
        Some(Err(e)) => return rsx! { div { class: "page", ErrorView { message: e.to_string() } } },
        Some(Ok(h)) => h.iter().filter(|h| !h.is_empty()).cloned().collect::<Vec<Hub>>(),
    };
    if hubs.is_empty() {
        return rsx! {
            div { class: "page",
                Empty { icon: Ic::Film, title: "Nothing here yet", text: "Your libraries are empty or still scanning." }
            }
        };
    }
    let featured = featured(&hubs);
    rsx! {
        div { class: "page flush",
            if !featured.is_empty() {
                Hero { items: featured }
            } else {
                div { style: "height: 60px" }
            }
            div { style: "padding: 0 36px",
                for hub in hubs {
                    HubRow { key: "{hub.hub_identifier:?}{hub.key:?}", hub: hub.clone() }
                }
            }
        }
    }
}

/// Pick hero items: recent movies & shows with backdrop art.
fn featured(hubs: &[Hub]) -> Vec<(Metadata, &'static str)> {
    let mut out: Vec<(Metadata, &'static str)> = Vec::new();
    let mut push = |m: &Metadata, label: &'static str| {
        if out.len() < 6 && m.art.is_some() && !out.iter().any(|(x, _)| x.rating_key == m.rating_key) {
            out.push((m.clone(), label));
        }
    };
    for hub in hubs {
        let ident = hub.hub_identifier.as_deref().unwrap_or("");
        let label = if ident.contains("continue") || ident.contains("ondeck") || ident.contains("onDeck") {
            "Continue watching"
        } else if ident.contains("recent") {
            "Recently added"
        } else {
            continue;
        };
        for m in &hub.metadata {
            if matches!(m.kind(), "movie" | "show") {
                push(m, label);
            }
        }
    }
    out
}

#[component]
fn Hero(items: Vec<(Metadata, &'static str)>) -> Element {
    let app = use_app();
    let nav = use_navigator();
    let mut index = use_signal(|| 0usize);
    let count = items.len();
    use_future(move || async move {
        loop {
            tokio::time::sleep(Duration::from_secs(9)).await;
            if count > 1 {
                index.set((index() + 1) % count);
            }
        }
    });
    let (item, label) = items[index() % count].clone();
    let bg = images::opt(item.art.as_deref(), 1920, 1080);
    let mut meta: Vec<String> = Vec::new();
    if let Some(y) = item.year {
        meta.push(y.to_string());
    }
    if let Some(c) = &item.content_rating {
        meta.push(c.clone());
    }
    match item.kind() {
        "show" => {
            if let Some(n) = item.child_count {
                meta.push(format!("{n} season{}", if n == 1 { "" } else { "s" }));
            }
        }
        _ => {
            if let Some(d) = item.duration {
                meta.push(fmt::duration(d));
            }
        }
    }
    let genres = item.genres.iter().take(3).map(|g| g.name().to_string()).collect::<Vec<_>>().join(" · ");
    let rating = item.audience_rating.or(item.rating);
    let resume = item.progress().is_some();
    let target = route_for(&item);
    let play_item = item.clone();

    rsx! {
        section { class: "hero",
            div { class: "hero-bg", key: "{item.rating_key():?}", Img { src: bg } }
            div { class: "hero-content", key: "{item.rating_key():?}-c",
                div { class: "hero-kicker", Icon { name: Ic::Sparkles, class: "sm" } "{label}" }
                h1 { "{item.title()}" }
                div { class: "meta",
                    if let Some(r) = rating {
                        span { class: "rating", Icon { name: Ic::Star }, "{r:.1}" }
                        span { class: "dot" }
                    }
                    for (i, m) in meta.iter().enumerate() {
                        if i > 0 { span { class: "dot" } }
                        span { "{m}" }
                    }
                    if !genres.is_empty() {
                        span { class: "dot" }
                        span { "{genres}" }
                    }
                }
                if let Some(s) = &item.summary { p { class: "summary", "{s}" } }
                div { class: "actions",
                    button { class: "btn light lg", onclick: move |_| player::play(app, PlayRequest::new(play_item.clone())),
                        Icon { name: Ic::Play }
                        if resume { "Resume" } else { "Play" }
                    }
                    button { class: "btn lg", onclick: move |_| { nav.push(target.clone()); },
                        Icon { name: Ic::Info } "More info"
                    }
                }
            }
            if count > 1 {
                div { class: "hero-dots",
                    for i in 0..count {
                        i { key: "{i}", class: if i == index() % count { "on" } else { "" }, onclick: move |_| index.set(i) }
                    }
                }
            }
        }
    }
}

#[component]
fn HomeSkeleton() -> Element {
    rsx! {
        div { class: "page flush",
            div { class: "skeleton", style: "height: 58vh; border-radius: 0; opacity: .5" }
            div { style: "padding: 0 36px",
                for r in 0..3 {
                    div { key: "{r}", class: "section-title", div { class: "skeleton", style: "width: 220px; height: 22px" } }
                    div { class: "row", style: "padding: 0; overflow: hidden",
                        for c in 0..8 {
                            div { key: "{c}", class: "card", div { class: "card-art skeleton" } }
                        }
                    }
                }
            }
        }
    }
}
