use std::collections::HashMap;
use std::time::Duration;

use dioxus::prelude::*;
use patina_plex::{Activity, HistoryQuery, Metadata, TranscodeSession};

use crate::fmt;
use crate::images;
use crate::state::use_app;
use crate::ui::components::{Empty, ErrorView, Img, Spinner, route_for};
use crate::ui::icons::{Ic, Icon};

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Playing,
    History,
    Tasks,
}

#[component]
pub fn ActivityPage() -> Element {
    let mut tab = use_signal(|| Tab::Playing);
    rsx! {
        div { class: "page",
            div { class: "page-head",
                div { h1 { "Activity" } div { class: "sub", "What's playing on the server, watch history, and background work." } }
                div { class: "tabs",
                    for (t, label) in [(Tab::Playing, "Now playing"), (Tab::History, "History"), (Tab::Tasks, "Server tasks")] {
                        div { class: if tab() == t { "tab active" } else { "tab" }, onclick: move |_| tab.set(t), "{label}" }
                    }
                }
            }
            match tab() {
                Tab::Playing => rsx! { Sessions {} },
                Tab::History => rsx! { History {} },
                Tab::Tasks => rsx! { Tasks {} },
            }
        }
    }
}

/// Re-run a fetch every `secs` seconds.
fn use_poll(secs: u64) -> Signal<u64> {
    let mut tick = use_signal(|| 0u64);
    use_future(move || async move {
        loop {
            tokio::time::sleep(Duration::from_secs(secs)).await;
            *tick.write() += 1;
        }
    });
    tick
}

#[component]
fn Sessions() -> Element {
    let app = use_app();
    let tick = use_poll(4);
    let sessions = use_resource(move || {
        let _ = tick();
        async move { app.server().sessions().await }
    });
    match &*sessions.read() {
        None => rsx! { Spinner {} },
        Some(Err(e)) => rsx! { ErrorView { message: e.to_string() } },
        Some(Ok(list)) if list.is_empty() => rsx! { Empty { icon: Ic::Activity, title: "Nothing is playing", text: "Streams from every Plex app on this server appear here live." } },
        Some(Ok(list)) => rsx! {
            div { class: "stats", style: "margin-bottom: 18px",
                div { class: "panel stat", b { "{list.len()}" } span { "active streams" } }
                div { class: "panel stat",
                    b { "{list.iter().filter(|m| m.transcode_session.as_ref().is_some_and(|t| t.video_decision.as_deref() == Some(\"transcode\"))).count()}" }
                    span { "transcoding" }
                }
                div { class: "panel stat",
                    b { "{fmt::bitrate(list.iter().filter_map(|m| m.session.as_ref().and_then(|s| s.bandwidth)).sum())}" }
                    span { "total bandwidth" }
                }
            }
            div { class: "panels",
                for m in list.iter() { SessionCard { key: "{m.session_key:?}", item: m.clone() } }
            }
        },
    }
}

fn decision_label(t: Option<&TranscodeSession>) -> (String, bool) {
    match t {
        None => ("Direct play".into(), false),
        Some(t) => {
            let v = t.video_decision.as_deref().unwrap_or("");
            let a = t.audio_decision.as_deref().unwrap_or("");
            if v == "transcode" {
                let hw = if t.transcode_hw_requested { " (HW)" } else { "" };
                let src = t.source_video_codec.clone().unwrap_or_default().to_uppercase();
                let dst = t.video_codec.clone().unwrap_or_default().to_uppercase();
                (format!("Transcode {src} → {dst}{hw}"), true)
            } else if a == "transcode" {
                ("Audio transcode".into(), true)
            } else {
                ("Direct stream".into(), false)
            }
        }
    }
}

#[component]
fn SessionCard(item: Metadata) -> Element {
    let app = use_app();
    let nav = use_navigator();
    let owned = app.session.read().as_ref().map(|s| s.owned).unwrap_or(false);
    let poster = images::opt(item.poster(), 200, 300);
    let user = item.user.clone().unwrap_or_default();
    let player = item.player.clone().unwrap_or_default();
    let state = player.state.clone().unwrap_or_default();
    let progress = item.progress().unwrap_or(0.0);
    let (decision, transcoding) = decision_label(item.transcode_session.as_ref());
    let bandwidth = item.session.as_ref().and_then(|s| s.bandwidth);
    let location = item.session.as_ref().and_then(|s| s.location.clone()).unwrap_or_default();
    let session_id = item.session.as_ref().and_then(|s| s.id.clone());
    let target = route_for(&item);
    let user_name = user.title.clone().unwrap_or_else(|| "Unknown".into());
    let (title, sub) = match item.kind() {
        "episode" => (item.grandparent_title.clone().unwrap_or_default(), format!("{} · {}", item.episode_code(), item.title())),
        "track" => (item.title().to_string(), item.grandparent_title.clone().unwrap_or_default()),
        _ => (item.title().to_string(), item.year.map(|y| y.to_string()).unwrap_or_default()),
    };
    rsx! {
        div { class: "session",
            div { class: "poster", style: "cursor: pointer", onclick: move |_| { nav.push(target.clone()); }, Img { src: poster } }
            div { style: "min-width: 0",
                span { class: "state badge accent", "{state.to_uppercase()}" }
                h4 { "{title}" }
                div { class: "muted small", "{sub}" }
                div { class: "who",
                    div { class: "avatar",
                        if user.thumb.is_some() { Img { src: user.thumb.clone() } } else { "{fmt::initials(&user_name)}" }
                    }
                    span { "{user_name}" }
                    span { class: "muted", "· {player.title.clone().unwrap_or_default()} ({player.product.clone().unwrap_or_default()})" }
                }
                div { class: "tags",
                    span { class: if transcoding { "badge accent" } else { "badge" }, "{decision}" }
                    if !location.is_empty() { span { class: "badge", "{location.to_uppercase()}" } }
                    if let Some(b) = bandwidth { span { class: "badge", "{fmt::bitrate(b)}" } }
                    if let Some(addr) = player.address.clone() { span { class: "badge", "{addr}" } }
                }
                div { class: "bar", i { style: "width: {progress * 100.0:.1}%" } }
                div { class: "flex small muted", style: "justify-content: space-between; margin-top: 6px",
                    span { "{fmt::clock(item.view_offset.unwrap_or(0))} / {fmt::clock(item.duration.unwrap_or(0))}" }
                    if owned {
                        if let Some(sid) = session_id {
                            span { class: "link", onclick: move |_| {
                                    let sid = sid.clone();
                                    spawn(async move {
                                        match app.server().terminate_session(&sid, "Stopped by the server owner.").await {
                                            Ok(()) => app.toast("Stream stopped"),
                                            Err(e) => app.error(e),
                                        }
                                    });
                                },
                                "Stop stream"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn History() -> Element {
    let app = use_app();
    let mut page = use_signal(|| 0u32);
    let owned = app.session.read().as_ref().map(|s| s.owned).unwrap_or(false);
    let data = use_resource(move || {
        let p = page();
        let _ = (app.revision)();
        async move {
            let server = app.server();
            let history = server.history(&HistoryQuery { start: p * 60, size: 60, ..Default::default() }).await?;
            let accounts: HashMap<i64, String> = if owned {
                server.accounts().await.unwrap_or_default().into_iter().filter_map(|a| Some((a.id?, a.name?))).collect()
            } else {
                HashMap::new()
            };
            Ok::<_, patina_plex::Error>((history, accounts))
        }
    });
    let read = data.read();
    let (history, accounts) = match &*read {
        None => return rsx! { Spinner {} },
        Some(Err(e)) => return rsx! { ErrorView { message: e.to_string() } },
        Some(Ok(x)) => x.clone(),
    };
    if history.metadata.is_empty() && page() == 0 {
        return rsx! { Empty { icon: Ic::Clock, title: "No history yet" } };
    }
    let has_more = history.has_more() || history.metadata.len() == 60;
    rsx! {
        div { class: "list",
            for m in history.metadata.iter() {
                HistoryRow {
                    key: "{m.history_key:?}",
                    item: m.clone(),
                    who: m.account_id.and_then(|id| accounts.get(&id).cloned()).unwrap_or_default(),
                    owned,
                }
            }
        }
        div { class: "flex", style: "justify-content: center; margin-top: 18px",
            button { class: "btn sm", disabled: page() == 0, onclick: move |_| page -= 1, Icon { name: Ic::ChevronLeft, class: "sm" } "Newer" }
            span { class: "muted small", "Page {page() + 1}" }
            button { class: "btn sm", disabled: !has_more, onclick: move |_| page += 1, "Older" Icon { name: Ic::ChevronRight, class: "sm" } }
        }
    }
}

#[component]
fn HistoryRow(item: Metadata, who: String, owned: bool) -> Element {
    let app = use_app();
    let thumb = images::opt(item.poster(), 120, 180);
    let when = item.viewed_at.map(fmt::ago).unwrap_or_default();
    let key = item.history_key.clone();
    rsx! {
        div { class: "list-row", style: "cursor: default",
            div { class: "thumb poster", Img { src: thumb } }
            div { class: "grow",
                div { class: "t", "{item.full_title()}" }
                div { class: "s", if !who.is_empty() { "{who} · " } "{when}" }
            }
            div { class: "end",
                if owned {
                    if let Some(k) = key {
                        button { class: "icon-btn", title: "Delete from history",
                            onclick: move |_| {
                                let id = k.rsplit('/').next().unwrap_or("").to_string();
                                spawn(async move {
                                    match app.server().delete_history(&id).await {
                                        Ok(()) => app.bump(),
                                        Err(e) => app.error(e),
                                    }
                                });
                            },
                            Icon { name: Ic::Trash, class: "sm" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Tasks() -> Element {
    let app = use_app();
    let tick = use_poll(3);
    let data = use_resource(move || {
        let _ = tick();
        async move {
            let server = app.server();
            let activities = server.activities().await.unwrap_or_default();
            let transcodes = server.transcode_sessions().await.unwrap_or_default();
            (activities, transcodes)
        }
    });
    let read = data.read();
    let Some((activities, transcodes)) = &*read else { return rsx! { Spinner {} } };
    rsx! {
        div { class: "panel",
            h3 { Icon { name: Ic::Sparkles, class: "sm" } "Background activity" }
            if activities.is_empty() {
                p { class: "desc", "The server is idle." }
            }
            for a in activities.iter() { ActivityRow { key: "{a.uuid:?}", activity: a.clone() } }
        }
        div { class: "panel",
            h3 { Icon { name: Ic::Zap, class: "sm" } "Transcoder sessions" }
            if transcodes.is_empty() {
                p { class: "desc", "No transcodes running." }
            }
            for t in transcodes.iter() {
                div { class: "setting", key: "{t.key:?}",
                    div { class: "label grow",
                        b { "{t.source_video_codec.clone().unwrap_or_default().to_uppercase()} → {t.video_codec.clone().unwrap_or_default().to_uppercase()} · {t.audio_codec.clone().unwrap_or_default().to_uppercase()}" }
                        span { {format!("{} · {} · speed {:.1}×{}", t.context.clone().unwrap_or_default(), t.protocol.clone().unwrap_or_default(), t.speed.unwrap_or(0.0), if t.throttled { " (throttled)" } else { "" })} }
                        div { class: "bar", style: "margin-top: 8px", i { style: "width: {t.progress.unwrap_or(0.0):.1}%" } }
                    }
                    if let Some(k) = t.key.clone() {
                        div { class: "control",
                            button { class: "btn sm danger", onclick: move |_| {
                                    let k = k.rsplit('/').next().unwrap_or("").to_string();
                                    spawn(async move { if let Err(e) = app.server().kill_transcode_session(&k).await { app.error(e) } });
                                },
                                "Stop"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ActivityRow(activity: Activity) -> Element {
    let app = use_app();
    let pct = activity.progress.unwrap_or(0);
    rsx! {
        div { class: "setting",
            div { class: "label grow",
                b { "{activity.title.clone().unwrap_or_default()}" }
                span { "{activity.subtitle.clone().unwrap_or_default()}" }
                div { class: "bar", style: "margin-top: 8px", i { style: "width: {pct}%" } }
            }
            div { class: "control",
                span { class: "muted", "{pct}%" }
                if activity.cancellable {
                    if let Some(uuid) = activity.uuid.clone() {
                        button { class: "btn sm", onclick: move |_| {
                                let uuid = uuid.clone();
                                spawn(async move { if let Err(e) = app.server().cancel_activity(&uuid).await { app.error(e) } });
                            },
                            "Cancel"
                        }
                    }
                }
            }
        }
    }
}
