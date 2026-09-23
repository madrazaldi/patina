use dioxus::prelude::*;
use patina_plex::Metadata;

use crate::fmt;
use crate::images;
use crate::player::{self, PlayRequest};
use crate::state::use_app;
use crate::ui::Route;
use crate::ui::components::{Empty, ErrorView, Img, MediaCard, Shape, Spinner, open_menu, route_for};
use crate::ui::icons::{Ic, Icon};

#[component]
pub fn PlaylistsPage() -> Element {
    let app = use_app();
    let lists = use_resource(move || {
        let _ = (app.revision)();
        async move { app.server().playlists(None, None).await }
    });
    rsx! {
        div { class: "page",
            div { class: "page-head", div { h1 { "Playlists" } div { class: "sub", "Right-click anything and choose “Add to playlist…” to build one." } } }
            match &*lists.read() {
                None => rsx! { Spinner {} },
                Some(Err(e)) => rsx! { ErrorView { message: e.to_string() } },
                Some(Ok(l)) if l.is_empty() => rsx! { Empty { icon: Ic::List, title: "No playlists yet" } },
                Some(Ok(l)) => rsx! {
                    div { class: "grid",
                        for p in l.iter() {
                            MediaCard { key: "{p.rating_key():?}", item: p.clone(), shape: Some(Shape::Square) }
                        }
                    }
                },
            }
        }
    }
}

#[component]
pub fn PlaylistPage(id: String) -> Element {
    let app = use_app();
    let nav = use_navigator();
    let data = use_resource(use_reactive((&id,), move |(id,)| {
        let _ = (app.revision)();
        async move {
            let server = app.server();
            let head = server.playlist(&id).await?;
            let items = server.playlist_items(&id, 0, 2000).await?.metadata;
            Ok::<_, patina_plex::Error>((head, items))
        }
    }));
    let read = data.read();
    let (head, items) = match &*read {
        None => return rsx! { Spinner { fill: true } },
        Some(Err(e)) => return rsx! { div { class: "page", ErrorView { message: e.to_string() } } },
        Some(Ok((Some(h), items))) => (h.clone(), items.clone()),
        Some(Ok((None, _))) => return rsx! { div { class: "page", Empty { icon: Ic::List, title: "Playlist not found" } } },
    };
    let cover = images::opt(head.composite.as_deref().or(head.thumb.as_deref()), 600, 600);
    let total: i64 = items.iter().filter_map(|m| m.duration).sum();
    let smart = head.smart;
    let play = head.clone();
    let shuffle = head.clone();
    let del_id = id.clone();

    rsx! {
        div { class: "page",
            div { class: "detail-top square", style: "grid-template-columns: 220px 1fr; margin-bottom: 30px",
                div { class: "detail-poster", Img { src: cover } }
                div { class: "detail-info",
                    div { class: "crumbs", if smart { "Smart playlist" } else { "Playlist" } }
                    h1 { "{head.title()}" }
                    div { class: "detail-meta", span { "{items.len()} items" } span { class: "dot" } span { "{fmt::duration(total)}" } }
                    if let Some(s) = &head.summary { p { class: "summary-text", "{s}" } }
                    div { class: "detail-actions",
                        button { class: "btn primary lg", disabled: items.is_empty(), onclick: move |_| player::play(app, PlayRequest::new(play.clone())), Icon { name: Ic::Play } "Play" }
                        button { class: "btn lg", disabled: items.is_empty(), onclick: move |_| player::play(app, PlayRequest { shuffle: true, ..PlayRequest::new(shuffle.clone()) }), Icon { name: Ic::Shuffle } "Shuffle" }
                        button { class: "btn danger lg", onclick: move |_| {
                                let id = del_id.clone();
                                spawn(async move {
                                    match app.server().delete_playlist(&id).await {
                                        Ok(()) => { app.toast("Playlist deleted"); nav.replace(Route::PlaylistsPage {}); }
                                        Err(e) => app.error(e),
                                    }
                                });
                            },
                            Icon { name: Ic::Trash } "Delete"
                        }
                    }
                }
            }
            if items.is_empty() {
                Empty { icon: Ic::List, title: "This playlist is empty" }
            } else {
                div { class: "list",
                    for (i, item) in items.iter().enumerate() {
                        PlaylistRow { key: "{item.playlist_item_id:?}", playlist: id.clone(), index: i + 1, item: item.clone(), smart }
                    }
                }
            }
        }
    }
}

#[component]
fn PlaylistRow(playlist: String, index: usize, item: Metadata, smart: bool) -> Element {
    let app = use_app();
    let nav = use_navigator();
    let art = images::opt(item.poster(), 120, 180);
    let target = route_for(&item);
    let ctx = item.clone();
    let play = item.clone();
    let sub = fmt::subtitle(&item);
    let pid = item.playlist_item_id;
    rsx! {
        div { class: "list-row", style: "grid-template-columns: 30px 44px 1fr auto",
            onclick: move |_| { nav.push(target.clone()); },
            oncontextmenu: move |e| { e.prevent_default(); let p = e.client_coordinates(); open_menu(app, &ctx, p.x, p.y); },
            div { class: "muted", style: "text-align: right; font-variant-numeric: tabular-nums", "{index}" }
            div { class: "thumb poster", Img { src: art } }
            div { class: "grow",
                div { class: "t", "{item.full_title()}" }
                div { class: "s", "{sub}" }
            }
            div { class: "end",
                if let Some(d) = item.duration { span { "{fmt::duration(d)}" } }
                button { class: "icon-btn", title: "Play", onclick: move |e| { e.stop_propagation(); player::play(app, PlayRequest::new(play.clone())); }, Icon { name: Ic::Play, class: "sm" } }
                if !smart {
                    if let Some(pid) = pid {
                        button { class: "icon-btn", title: "Remove from playlist",
                            onclick: move |e| {
                                e.stop_propagation();
                                let playlist = playlist.clone();
                                spawn(async move {
                                    match app.server().remove_from_playlist(&playlist, pid).await {
                                        Ok(()) => app.bump(),
                                        Err(e) => app.error(e),
                                    }
                                });
                            },
                            Icon { name: Ic::X, class: "sm" }
                        }
                    }
                }
            }
        }
    }
}
