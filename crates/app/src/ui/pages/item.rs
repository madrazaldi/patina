use dioxus::prelude::*;
use patina_plex::{Metadata, MetadataOptions, Tag, channel_label};

use crate::fmt;
use crate::images;
use crate::player::{self, PlayRequest};
use crate::state::{AppState, use_app};
use crate::ui::Route;
use crate::ui::components::{Empty, ErrorView, HubRow, Img, MediaCard, Row, Shape, Spinner, browse, open_menu};
use crate::ui::icons::{Ic, Icon};

#[component]
pub fn ItemPage(rating_key: String) -> Element {
    let app = use_app();
    let data = use_resource(use_reactive((&rating_key,), move |(rk,)| {
        let _ = (app.revision)();
        async move {
            let r = app.server().metadata(&rk, MetadataOptions::full()).await;
            (rk, r)
        }
    }));
    let read = data.read();
    match &*read {
        Some((rk, Ok(Some(item)))) if *rk == rating_key => rsx! { Detail { key: "{rk}", item: item.clone() } },
        Some((rk, Ok(None))) if *rk == rating_key => rsx! {
            div { class: "page", Empty { icon: Ic::Alert, title: "Not found", text: "This item no longer exists on the server." } }
        },
        Some((rk, Err(e))) if *rk == rating_key => rsx! { div { class: "page", ErrorView { message: e.to_string() } } },
        _ => rsx! { Spinner { fill: true } },
    }
}

fn glow(item: &Metadata) -> Option<String> {
    let c = item.ultra_blur_colors.as_ref()?;
    let (tl, tr, br, bl) = (c.top_left.as_ref()?, c.top_right.as_ref()?, c.bottom_right.as_ref()?, c.bottom_left.as_ref()?);
    Some(format!(
        "background: radial-gradient(60% 60% at 0% 0%, #{tl}, transparent), radial-gradient(60% 60% at 100% 0%, #{tr}, transparent), radial-gradient(60% 60% at 100% 100%, #{br}, transparent), radial-gradient(60% 60% at 0% 100%, #{bl}, transparent);"
    ))
}

fn tag_route(item: &Metadata, tag: &Tag) -> Option<Route> {
    let section = item.library_section_id?;
    let filter = tag.filter.clone()?;
    Some(browse(&format!("/library/sections/{section}/all?{filter}"), tag.name()))
}

#[component]
fn Detail(item: Metadata) -> Element {
    let app = use_app();
    let nav = use_navigator();
    let mut expanded = use_signal(|| false);
    let media_index = use_signal(|| 0usize);

    let kind = item.kind().to_string();
    let rk = item.rating_key().to_string();
    let shape = match kind.as_str() {
        "episode" | "clip" => Shape::Landscape,
        "album" | "artist" | "playlist" | "photoalbum" | "track" => Shape::Square,
        _ => Shape::Poster,
    };
    let top_class = match shape {
        Shape::Landscape => "detail-top landscape",
        Shape::Square => "detail-top square",
        Shape::Poster => "detail-top",
    };
    let poster_src = match shape {
        Shape::Landscape => images::opt(item.thumb.as_deref(), 840, 472),
        Shape::Square => images::opt(item.thumb.as_deref().or(item.parent_thumb.as_deref()), 600, 600),
        Shape::Poster => images::opt(item.thumb.as_deref(), 500, 750),
    };
    let backdrop = images::opt(item.backdrop(), 1920, 1080);
    let glow_style = glow(&item);
    let spoiler = app.settings.read().ui.hide_spoilers && kind == "episode" && !item.is_watched();

    // Meta line
    let mut meta: Vec<String> = Vec::new();
    match kind.as_str() {
        "episode" => {
            meta.push(item.episode_code());
            if let Some(d) = &item.originally_available_at {
                meta.push(fmt::air_date(d));
            }
        }
        _ => {
            if let Some(y) = item.year {
                meta.push(y.to_string());
            }
        }
    }
    if let Some(d) = item.duration.filter(|_| kind != "show" && kind != "season") {
        meta.push(fmt::duration(d));
    }
    if kind == "show"
        && let Some(n) = item.child_count {
            meta.push(format!("{n} season{}", if n == 1 { "" } else { "s" }));
        }
    if let Some(n) = item.leaf_count.filter(|_| matches!(kind.as_str(), "show" | "season" | "album")) {
        meta.push(format!("{n} {}", if kind == "album" { "tracks" } else { "episodes" }));
    }
    let content_rating = item.content_rating.clone();
    let media = item.media.get(media_index()).or(item.media.first()).cloned();
    let media_label = media.as_ref().map(|m| m.label()).filter(|l| !l.is_empty());

    let progress = item.progress();
    let left = fmt::remaining(&item);
    let is_container = item.is_container();
    let playable = item.is_playable() || is_container;
    let watched = item.is_watched();
    let summary = item.summary.clone().unwrap_or_default();
    let long_summary = summary.len() > 380;

    // Crumbs
    let crumbs: Vec<(String, Route)> = match kind.as_str() {
        "episode" => [
            item.grandparent_rating_key.clone().zip(item.grandparent_title.clone()),
            item.parent_rating_key.clone().zip(item.parent_title.clone()),
        ]
        .into_iter()
        .flatten()
        .map(|(k, t)| (t, Route::ItemPage { rating_key: k }))
        .collect(),
        "season" | "album" => item
            .parent_rating_key
            .clone()
            .zip(item.parent_title.clone())
            .map(|(k, t)| vec![(t, Route::ItemPage { rating_key: k })])
            .unwrap_or_default(),
        "track" => [
            item.grandparent_rating_key.clone().zip(item.grandparent_title.clone()),
            item.parent_rating_key.clone().zip(item.parent_title.clone()),
        ]
        .into_iter()
        .flatten()
        .map(|(k, t)| (t, Route::ItemPage { rating_key: k }))
        .collect(),
        _ => Vec::new(),
    };

    let has_tv = app.session.read().as_ref().and_then(|s| s.tv.as_ref()).is_some();
    let discover_id = item.discover_id().map(str::to_string).filter(|_| has_tv && matches!(kind.as_str(), "movie" | "show"));
    let did = discover_id.clone();
    let mut watchlisted = use_resource(move || {
        let id = did.clone();
        async move {
            let (Some(id), Some(tv)) = (id, app.tv()) else { return None };
            tv.user_state(&id).await.ok().flatten().map(|s| s.watchlisted_at.is_some())
        }
    });

    let play_item = item.clone();
    let start_item = item.clone();
    let shuffle_item = item.clone();
    let menu_item = item.clone();
    let playlist_item = item.clone();
    let rk_watch = rk.clone();
    let rk_rate = rk.clone();
    let user_rating = item.user_rating.unwrap_or(0.0);

    rsx! {
        div { class: "detail",
            if let Some(g) = glow_style { div { class: "detail-glow", style: "{g}" } }
            div { class: "detail-backdrop", Img { src: backdrop } }
            div { class: "detail-inner",
                div { class: "{top_class}",
                    div { class: "detail-poster",
                        if poster_src.is_some() {
                            Img { src: poster_src, class: if spoiler { "spoiler" } else { "" } }
                        }
                    }
                    div { class: "detail-info",
                        if !crumbs.is_empty() {
                            div { class: "crumbs",
                                for (i, (title, route)) in crumbs.into_iter().enumerate() {
                                    if i > 0 { Icon { name: Ic::ChevronRight, class: "sm" } }
                                    a { onclick: move |_| { nav.push(route.clone()); }, "{title}" }
                                }
                            }
                        }
                        h1 { "{item.title()}" }
                        if let Some(o) = item.original_title.as_ref().filter(|o| o.as_str() != item.title()) {
                            div { class: "original", "{o}" }
                        }
                        div { class: "detail-meta",
                            Ratings { item: item.clone() }
                            for (i, m) in meta.iter().enumerate() {
                                if i > 0 { span { class: "dot" } }
                                span { "{m}" }
                            }
                            if let Some(c) = content_rating { span { class: "badge", "{c}" } }
                            if let Some(l) = media_label { span { class: "badge solid", "{l}" } }
                        }
                        div { class: "detail-actions",
                            if playable {
                                button { class: "btn primary lg",
                                    onclick: move |_| player::play(app, PlayRequest { media_index: media_index(), ..PlayRequest::new(play_item.clone()) }),
                                    Icon { name: Ic::Play }
                                    if progress.is_some() { "Resume" } else if is_container { "Play" } else { "Play" }
                                    if let Some(p) = progress {
                                        span { class: "progress-mini", i { style: "width: {p * 100.0:.0}%" } }
                                    }
                                    if let Some(l) = left { span { style: "font-weight: 500; opacity: .75", "{l}" } }
                                }
                                if progress.is_some() {
                                    button { class: "btn lg",
                                        onclick: move |_| player::play(app, PlayRequest { resume: false, media_index: media_index(), ..PlayRequest::new(start_item.clone()) }),
                                        Icon { name: Ic::Restart } "From start"
                                    }
                                }
                                if is_container {
                                    button { class: "btn lg",
                                        onclick: move |_| player::play(app, PlayRequest { shuffle: true, ..PlayRequest::new(shuffle_item.clone()) }),
                                        Icon { name: Ic::Shuffle } "Shuffle"
                                    }
                                }
                            }
                            if !matches!(kind.as_str(), "artist" | "album" | "track" | "photo" | "photoalbum" | "collection") {
                                button {
                                    class: if watched { "icon-btn big active" } else { "icon-btn big" },
                                    title: if watched { "Mark as unwatched" } else { "Mark as watched" },
                                    onclick: move |_| toggle_watched(app, rk_watch.clone(), watched),
                                    Icon { name: Ic::Check }
                                }
                            }
                            if let Some(id) = discover_id.clone() {
                                {
                                    let on = watchlisted.read().flatten().unwrap_or(false);
                                    rsx! {
                                        button {
                                            class: if on { "icon-btn big active" } else { "icon-btn big" },
                                            title: if on { "Remove from watchlist" } else { "Add to watchlist" },
                                            onclick: move |_| {
                                                let id = id.clone();
                                                spawn(async move {
                                                    let Some(tv) = app.tv() else { return };
                                                    let r = if on { tv.remove_from_watchlist(&id).await } else { tv.add_to_watchlist(&id).await };
                                                    match r {
                                                        Ok(()) => app.toast(if on { "Removed from watchlist" } else { "Added to watchlist" }),
                                                        Err(e) => app.error(e),
                                                    }
                                                    watchlisted.restart();
                                                });
                                            },
                                            Icon { name: if on { Ic::BookmarkCheck } else { Ic::Bookmark } }
                                        }
                                    }
                                }
                            }
                            if !matches!(kind.as_str(), "collection" | "playlist") {
                                button { class: "icon-btn big", title: "Add to playlist",
                                    onclick: move |_| { let mut t = app.playlist_target; t.set(Some(playlist_item.clone())); },
                                    Icon { name: Ic::List }
                                }
                            }
                            button { class: "icon-btn big", title: "More",
                                onclick: move |e| { let p = e.client_coordinates(); open_menu(app, &menu_item, p.x, p.y); },
                                Icon { name: Ic::More }
                            }
                            if matches!(kind.as_str(), "movie" | "show" | "episode" | "season" | "album" | "track" | "artist") {
                                StarRating { value: user_rating, on_rate: move |v: Option<f64>| {
                                    let rk = rk_rate.clone();
                                    spawn(async move {
                                        match app.server().rate(&rk, v).await {
                                            Ok(()) => app.bump(),
                                            Err(e) => app.error(e),
                                        }
                                    });
                                } }
                            }
                        }
                    }
                }

                if let Some(t) = &item.tagline { div { class: "tagline", "{t}" } }
                if !summary.is_empty() {
                    p {
                        class: format!("summary-text{}{}", if long_summary && !expanded() { " clamp" } else { "" }, if spoiler { " spoiler" } else { "" }),
                        onclick: move |_| expanded.toggle(),
                        "{summary}"
                    }
                }

                if matches!(kind.as_str(), "movie" | "episode" | "clip" | "track") && !item.media.is_empty() {
                    StreamPickers { item: item.clone(), media_index }
                }

                Facts { item: item.clone() }
                Children { item: item.clone() }

                if !item.roles.is_empty() {
                    div { class: "section-title", h2 { if kind == "album" || kind == "artist" { "Featuring" } else { "Cast" } } }
                    div { class: "row-scroller",
                        div { class: "row",
                            for role in item.roles.iter().take(40) {
                                Person { key: "{role.id:?}{role.tag:?}", tag: role.clone(), route: tag_route(&item, role) }
                            }
                        }
                    }
                }
                if !item.extras().is_empty() {
                    Row { title: "Extras", items: item.extras().to_vec(), shape: Some(Shape::Landscape) }
                }
                for hub in item.related_hubs().iter().filter(|h| !h.is_empty()) {
                    HubRow { key: "{hub.hub_identifier:?}", hub: hub.clone() }
                }
            }
        }
    }
}

fn toggle_watched(app: AppState, rk: String, watched: bool) {
    spawn(async move {
        let server = app.server();
        let r = if watched { server.unscrobble(&rk).await } else { server.scrobble(&rk).await };
        match r {
            Ok(()) => app.toast(if watched { "Marked as unwatched" } else { "Marked as watched" }),
            Err(e) => app.error(e),
        }
        app.bump();
    });
}

#[component]
fn Ratings(item: Metadata) -> Element {
    let critic = item.rating;
    let audience = item.audience_rating;
    let from_rt = |img: &Option<String>| img.as_deref().map(|i| i.contains("rottentomatoes")).unwrap_or(false);
    let critic_rt = from_rt(&item.rating_image);
    let audience_rt = from_rt(&item.audience_rating_image);
    rsx! {
        if let Some(r) = critic {
            span { class: "rating", title: "Critic rating",
                if critic_rt { "🍅 {r * 10.0:.0}%" } else { Icon { name: Ic::Star } "{r:.1}" }
            }
            span { class: "dot" }
        }
        if let Some(r) = audience {
            span { class: "rating", title: "Audience rating",
                if audience_rt { "🍿 {r * 10.0:.0}%" } else { Icon { name: Ic::Star } "{r:.1}" }
            }
            span { class: "dot" }
        }
    }
}

#[component]
fn StarRating(value: f64, on_rate: EventHandler<Option<f64>>) -> Element {
    let mut hover = use_signal(|| None::<u8>);
    let stars = (value / 2.0).round() as u8;
    let shown = hover().unwrap_or(stars);
    rsx! {
        div { class: "star-rate", title: "Your rating", onmouseleave: move |_| hover.set(None),
            for n in 1..=5u8 {
                span {
                    key: "{n}",
                    onmouseenter: move |_| hover.set(Some(n)),
                    onclick: move |_| on_rate.call(if n == stars { None } else { Some(n as f64 * 2.0) }),
                    Icon { name: Ic::Star, class: if n <= shown { "on" } else { "" } }
                }
            }
        }
    }
}

/// Version / audio / subtitle choosers. Changes are saved to Plex immediately,
/// and playback follows them.
#[component]
fn StreamPickers(item: Metadata, media_index: Signal<usize>) -> Element {
    let app = use_app();
    let media = item.media.get(media_index()).or(item.media.first()).cloned();
    let Some(media) = media else { return rsx! {} };
    let Some(part) = media.parts.first().cloned() else { return rsx! {} };
    let part_id = part.id.unwrap_or_default();
    let audio: Vec<_> = part.audio_streams().cloned().collect();
    let subs: Vec<_> = part.subtitle_streams().cloned().collect();
    let selected_sub = part.selected_subtitle().and_then(|s| s.id).unwrap_or(0);
    let select = move |audio: Option<i64>, sub: Option<i64>| {
        spawn(async move {
            match app.server().select_streams(part_id, audio, sub, true).await {
                Ok(()) => app.bump(),
                Err(e) => app.error(e),
            }
        });
    };
    rsx! {
        div { class: "pickers",
            if item.media.len() > 1 {
                div { class: "picker",
                    label { Icon { name: Ic::Layers, class: "sm" } "Version" }
                    select { class: "select", onchange: move |e| media_index.set(e.value().parse().unwrap_or(0)),
                        for (i, m) in item.media.iter().enumerate() {
                            option { key: "{i}", value: "{i}", selected: i == media_index(), "{m.label()}" }
                        }
                    }
                }
            }
            if !audio.is_empty() {
                div { class: "picker",
                    label { Icon { name: Ic::Volume, class: "sm" } "Audio" }
                    select { class: "select", onchange: move |e| select(e.value().parse().ok(), None),
                        for s in audio.iter() {
                            option { key: "{s.id:?}", value: "{s.id.unwrap_or_default()}", selected: s.selected, "{s.label()}" }
                        }
                    }
                }
            }
            if item.kind() != "track" {
                div { class: "picker",
                    label { Icon { name: Ic::Subtitles, class: "sm" } "Subtitles" }
                    select { class: "select", onchange: move |e| select(None, e.value().parse().ok()),
                        option { value: "0", selected: selected_sub == 0, "None" }
                        for s in subs.iter() {
                            option { key: "{s.id:?}", value: "{s.id.unwrap_or_default()}", selected: s.selected,
                                {format!("{}{}", s.label(), if s.is_external() { " · external" } else { "" })}
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Facts(item: Metadata) -> Element {
    let nav = use_navigator();
    let app = use_app();
    let owned = app.session.read().as_ref().map(|s| s.owned).unwrap_or(false);
    let tags = |label: &'static str, list: &[Tag]| -> Option<(&'static str, Vec<(String, Option<Route>)>)> {
        (!list.is_empty()).then(|| (label, list.iter().take(8).map(|t| (t.name().to_string(), tag_route(&item, t))).collect()))
    };
    let groups: Vec<_> = [
        tags("Directed by", &item.directors),
        tags("Written by", &item.writers),
        tags("Genres", &item.genres),
        tags("Collections", &item.collections),
        tags("Moods", &item.moods),
        tags("Styles", &item.styles),
        tags("Country", &item.countries),
        tags("Labels", &item.labels),
    ]
    .into_iter()
    .flatten()
    .collect();

    let mut plain: Vec<(&'static str, String)> = Vec::new();
    if let Some(s) = &item.studio {
        plain.push(("Studio", s.clone()));
    }
    if let Some(m) = item.media.first() {
        let p = m.parts.first();
        let mut bits = Vec::new();
        if let Some(c) = &m.container {
            bits.push(c.to_uppercase());
        }
        if let (Some(w), Some(h)) = (m.width, m.height) {
            bits.push(format!("{w}×{h}"));
        }
        if let Some(b) = m.bitrate {
            bits.push(fmt::bitrate(b));
        }
        if let Some(size) = p.and_then(|p| p.size) {
            bits.push(fmt::bytes(size));
        }
        if !bits.is_empty() {
            plain.push(("File", bits.join(" · ")));
        }
        if let Some(a) = p.and_then(|p| p.selected_audio()) {
            let ch = a.channels.map(channel_label).unwrap_or_default();
            plain.push(("Audio", format!("{} {}", a.codec.clone().unwrap_or_default().to_uppercase(), ch)));
        }
        if owned
            && let Some(f) = p.and_then(|p| p.file.clone()) {
                plain.push(("Path", f));
            }
    }
    if let Some(t) = item.added_at {
        plain.push(("Added", fmt::date(t)));
    }
    if let Some(t) = item.last_viewed_at {
        let n = item.view_count.unwrap_or(0);
        plain.push(("Last watched", format!("{} · {n} play{}", fmt::ago(t), if n == 1 { "" } else { "s" })));
    }
    if groups.is_empty() && plain.is_empty() {
        return rsx! {};
    }
    rsx! {
        div { class: "facts",
            for (label, values) in groups {
                div { class: "fact", key: "{label}",
                    label { "{label}" }
                    div {
                        for (i, (name, route)) in values.into_iter().enumerate() {
                            if i > 0 { ", " }
                            if let Some(r) = route {
                                a { onclick: move |_| { nav.push(r.clone()); }, "{name}" }
                            } else {
                                span { "{name}" }
                            }
                        }
                    }
                }
            }
            for (label, value) in plain {
                div { class: "fact", key: "{label}", label { "{label}" } div { style: "user-select: text", "{value}" } }
            }
        }
    }
}

#[component]
fn Person(tag: Tag, route: Option<Route>) -> Element {
    let nav = use_navigator();
    let face = images::opt(tag.thumb.as_deref(), 220, 220);
    let name = tag.name().to_string();
    rsx! {
        div { class: "person", onclick: move |_| if let Some(r) = &route { nav.push(r.clone()); },
            div { class: "face", if face.is_some() { Img { src: face } } else { "{fmt::initials(&name)}" } }
            b { "{name}" }
            if let Some(r) = &tag.role { span { "{r}" } }
        }
    }
}

/// Seasons, episodes, albums, tracks or collection items below the header.
#[component]
fn Children(item: Metadata) -> Element {
    let app = use_app();
    let kind = item.kind().to_string();
    let rk = item.rating_key().to_string();
    let wants = matches!(kind.as_str(), "show" | "season" | "artist" | "album" | "collection" | "photoalbum");
    let k = kind.clone();
    let children = use_resource(move || {
        let rk = rk.clone();
        let kind = k.clone();
        let _ = (app.revision)();
        async move {
            if !wants {
                return Ok(Vec::new());
            }
            let server = app.server();
            let mc = if kind == "collection" { server.collection_items(&rk).await? } else { server.children(&rk).await? };
            Ok::<_, patina_plex::Error>(mc.metadata)
        }
    });
    if !wants {
        return rsx! {};
    }
    let read = children.read();
    let list = match &*read {
        None => return rsx! { Spinner {} },
        Some(Err(e)) => return rsx! { ErrorView { message: e.to_string() } },
        Some(Ok(l)) => l.clone(),
    };
    if list.is_empty() {
        return rsx! {};
    }
    match kind.as_str() {
        "show" => rsx! { Row { title: "Seasons", items: list } },
        "season" => rsx! {
            div { class: "section-title", h2 { "Episodes" } }
            div { class: "episodes",
                for ep in list {
                    EpisodeRow { key: "{ep.rating_key():?}", item: ep }
                }
            }
        },
        "album" => rsx! {
            div { class: "section-title", h2 { "Tracks" } }
            div { class: "list",
                for t in list {
                    TrackRow { key: "{t.rating_key():?}", item: t }
                }
            }
        },
        "artist" => rsx! {
            div { class: "section-title", h2 { "Albums" } }
            div { class: "grid", for a in list { MediaCard { key: "{a.rating_key():?}", item: a } } }
        },
        _ => rsx! {
            div { class: "section-title", h2 { "{list.len()} items" } }
            div { class: "grid", for a in list { MediaCard { key: "{a.rating_key():?}", item: a } } }
        },
    }
}

#[component]
pub fn EpisodeRow(item: Metadata) -> Element {
    let app = use_app();
    let nav = use_navigator();
    let spoiler = app.settings.read().ui.hide_spoilers && !item.is_watched();
    let thumb = images::opt(item.thumb.as_deref(), 480, 270);
    let target = Route::ItemPage { rating_key: item.rating_key().to_string() };
    let play = item.clone();
    let ctx = item.clone();
    let mut when = Vec::new();
    if let Some(d) = &item.originally_available_at {
        when.push(fmt::air_date(d));
    }
    if let Some(d) = item.duration {
        when.push(fmt::duration(d));
    }
    let when = when.join(" · ");
    rsx! {
        div { class: "episode",
            onclick: move |_| { nav.push(target.clone()); },
            oncontextmenu: move |e| { e.prevent_default(); let p = e.client_coordinates(); open_menu(app, &ctx, p.x, p.y); },
            div { class: "num", "{item.index.map(|i| i.to_string()).unwrap_or_default()}" }
            div { class: "thumb",
                Img { src: thumb, class: if spoiler { "spoiler" } else { "" } }
                if let Some(p) = item.progress() { div { class: "progress", i { style: "width: {p * 100.0:.1}%" } } }
                button { class: "card-play", onclick: move |e| { e.stop_propagation(); player::play(app, PlayRequest::new(play.clone())); }, Icon { name: Ic::Play } }
            }
            div { class: "body",
                h3 { "{item.title()}" }
                div { class: "when", "{when}" }
                if let Some(s) = &item.summary { p { class: if spoiler { "spoiler" } else { "" }, "{s}" } }
            }
            div { class: "side",
                if item.is_watched() { span { title: "Watched", style: "color: var(--accent)", Icon { name: Ic::CheckCircle } } }
                else if item.progress().is_none() { span { title: "Unwatched", Icon { name: Ic::Circle } } }
            }
        }
    }
}

#[component]
fn TrackRow(item: Metadata) -> Element {
    let app = use_app();
    let play = item.clone();
    let ctx = item.clone();
    rsx! {
        div { class: "list-row track",
            onclick: move |_| player::play(app, PlayRequest::new(play.clone())),
            oncontextmenu: move |e| { e.prevent_default(); let p = e.client_coordinates(); open_menu(app, &ctx, p.x, p.y); },
            div { class: "n", "{item.index.map(|i| i.to_string()).unwrap_or_default()}" }
            div { class: "grow",
                div { class: "t", "{item.title()}" }
                if let Some(a) = item.original_title.as_ref() { div { class: "s", "{a}" } }
            }
            div { class: "end", if let Some(d) = item.duration { "{fmt::clock(d)}" } }
            div { class: "end", Icon { name: Ic::Play, class: "sm" } }
        }
    }
}
