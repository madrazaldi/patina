use dioxus::prelude::*;
use patina_plex::Setting;

use crate::fmt;
use crate::session;
use crate::state::{AppState, use_app};
use crate::ui::components::{Empty, ErrorView, Spinner};
use crate::ui::icons::{Ic, Icon, section_icon};

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Overview,
    Libraries,
    Maintenance,
    Settings,
    Users,
}

#[component]
pub fn ServerPage() -> Element {
    let app = use_app();
    let mut tab = use_signal(|| Tab::Overview);
    let owned = app.session.read().as_ref().map(|s| s.owned).unwrap_or(false);
    let name = app.session.read().as_ref().map(|s| s.server_name.clone()).unwrap_or_default();
    if !owned {
        return rsx! { div { class: "page", Empty { icon: Ic::Server, title: "Server tools need owner access", text: "This server is shared with you." } } };
    }
    rsx! {
        div { class: "page",
            div { class: "page-head",
                div { h1 { "{name}" } div { class: "sub", "Manage your Plex Media Server." } }
                div { class: "tabs",
                    for (t, label) in [(Tab::Overview, "Overview"), (Tab::Libraries, "Libraries"), (Tab::Maintenance, "Maintenance"), (Tab::Settings, "Settings"), (Tab::Users, "Users & devices")] {
                        div { class: if tab() == t { "tab active" } else { "tab" }, onclick: move |_| tab.set(t), "{label}" }
                    }
                }
            }
            match tab() {
                Tab::Overview => rsx! { Overview {} },
                Tab::Libraries => rsx! { Libraries {} },
                Tab::Maintenance => rsx! { Maintenance {} },
                Tab::Settings => rsx! { Prefs {} },
                Tab::Users => rsx! { UsersDevices {} },
            }
        }
    }
}

fn run_action<F, Fut>(app: AppState, ok: &'static str, f: F)
where
    F: FnOnce(patina_plex::PlexServer) -> Fut + 'static,
    Fut: std::future::Future<Output = patina_plex::Result<()>> + 'static,
{
    spawn(async move {
        match f(app.server()).await {
            Ok(()) => app.toast(ok),
            Err(e) => app.error(e),
        }
    });
}

#[component]
fn Overview() -> Element {
    let app = use_app();
    let data = use_resource(move || async move {
        let server = app.server();
        let caps = server.capabilities().await?;
        let update = server.update_status().await.ok();
        let resources = server.statistics_resources(6).await.unwrap_or_default();
        Ok::<_, patina_plex::Error>((caps, update, resources))
    });
    let read = data.read();
    let (caps, update, resources) = match &*read {
        None => return rsx! { Spinner {} },
        Some(Err(e)) => return rsx! { ErrorView { message: e.to_string() } },
        Some(Ok(x)) => x.clone(),
    };
    let attr = |k: &str| caps.attr(k).unwrap_or_else(|| "—".into());
    let plex_pass = caps.attr("myPlexSubscription").map(|v| v == "true" || v == "1").unwrap_or(false);
    let latest = resources.last().cloned().unwrap_or_default();
    let release = update.as_ref().and_then(|u| u.release.first().cloned());
    let base = app.session.read().as_ref().map(|s| s.server.base_url().to_string()).unwrap_or_default();
    rsx! {
        div { class: "stats", style: "margin-bottom: 18px",
            div { class: "panel stat", b { "{latest.host_cpu_utilization.unwrap_or(0.0):.0}%" } span { "host CPU" } }
            div { class: "panel stat", b { "{latest.process_cpu_utilization.unwrap_or(0.0):.0}%" } span { "Plex CPU" } }
            div { class: "panel stat", b { "{latest.host_memory_utilization.unwrap_or(0.0):.0}%" } span { "host memory" } }
            div { class: "panel stat", b { "{latest.process_memory_utilization.unwrap_or(0.0):.0}%" } span { "Plex memory" } }
        }
        div { class: "panels",
            div { class: "panel",
                h3 { Icon { name: Ic::Server, class: "sm" } "Server" }
                dl { class: "kv mt",
                    dt { "Version" } dd { "{attr(\"version\")}" }
                    dt { "Platform" } dd { "{attr(\"platform\")} {attr(\"platformVersion\")}" }
                    dt { "Owner" } dd { "{attr(\"myPlexUsername\")}" }
                    dt { "Plex Pass" } dd { if plex_pass { "Active" } else { "No" } }
                    dt { "Connected via" } dd { "{base}" }
                    dt { "Machine ID" } dd { style: "user-select: text", "{attr(\"machineIdentifier\")}" }
                    dt { "Transcoder" } dd { if attr("transcoderVideo") == "true" || attr("transcoderVideo") == "1" { "Available" } else { "—" } }
                }
            }
            div { class: "panel",
                h3 { Icon { name: Ic::Download, class: "sm" } "Updates" }
                if let Some(r) = release {
                    p { class: "desc", "Version {r.version.clone().unwrap_or_default()} is available." }
                    if let Some(added) = r.added.clone() { p { class: "small muted", style: "white-space: pre-line; max-height: 160px; overflow: auto", "{added}" } }
                    div { class: "flex mt",
                        if update.as_ref().map(|u| u.can_install).unwrap_or(false) {
                            button { class: "btn primary sm", onclick: move |_| run_action(app, "Installing update…", |s| async move { s.apply_update(false, false).await }), "Install now" }
                            button { class: "btn sm", onclick: move |_| run_action(app, "Update scheduled for tonight", |s| async move { s.apply_update(true, false).await }), "Tonight" }
                        } else if let Some(url) = r.download_url.clone() {
                            button { class: "btn sm", onclick: move |_| { let _ = open::that(&url); }, Icon { name: Ic::ExternalLink, class: "sm" } "Download" }
                        }
                    }
                } else {
                    p { class: "desc", "The server is up to date." }
                }
                button { class: "btn sm mt", onclick: move |_| {
                        spawn(async move {
                            match app.server().check_for_updates(true).await {
                                Ok(()) => { app.toast("Checked for updates"); data.clone().restart(); }
                                Err(e) => app.error(e),
                            }
                        });
                    },
                    Icon { name: Ic::Refresh, class: "sm" } "Check now"
                }
            }
        }
    }
}

#[component]
fn Libraries() -> Element {
    let app = use_app();
    let sections = app.sections.read().clone();
    rsx! {
        div { class: "panel",
            h3 { Icon { name: Ic::Folder, class: "sm" } "Libraries" }
            p { class: "desc", "Scan for new files, refresh metadata, analyze media or empty the trash." }
            for s in sections {
                {
                    let id = s.key.clone().unwrap_or_default();
                    let (a, b, c, d) = (id.clone(), id.clone(), id.clone(), id.clone());
                    let locations = s.location.iter().filter_map(|l| l.path.clone()).collect::<Vec<_>>().join(", ");
                    rsx! {
                        div { class: "setting", key: "{id}",
                            div { class: "label flex",
                                Icon { name: section_icon(s.kind.as_deref().unwrap_or("")) }
                                div {
                                    b { "{s.title()}" }
                                    span { "{locations}" }
                                    if let Some(t) = s.scanned_at { span { "Last scanned {fmt::ago(t)}" } }
                                }
                            }
                            div { class: "control",
                                button { class: "btn sm", onclick: move |_| { let id = a.clone(); run_action(app, "Scanning…", move |s| async move { s.refresh_section(&id, false, None).await }) }, "Scan" }
                                button { class: "btn sm", title: "Re-download all metadata", onclick: move |_| { let id = b.clone(); run_action(app, "Refreshing all metadata…", move |s| async move { s.refresh_section(&id, true, None).await }) }, "Refresh" }
                                button { class: "btn sm", onclick: move |_| { let id = c.clone(); run_action(app, "Analyzing…", move |s| async move { s.analyze_section(&id).await }) }, "Analyze" }
                                button { class: "btn sm", onclick: move |_| { let id = d.clone(); run_action(app, "Trash emptied", move |s| async move { s.empty_trash(&id).await }) }, "Empty trash" }
                            }
                        }
                    }
                }
            }
            div { class: "flex mt",
                button { class: "btn sm primary", onclick: move |_| run_action(app, "Scanning all libraries…", |s| async move { s.refresh_all_sections(false).await }), Icon { name: Ic::Refresh, class: "sm" } "Scan all" }
                button { class: "btn sm", onclick: move |_| { spawn(session::reload_sections(app)); }, "Reload list" }
            }
        }
    }
}

#[component]
fn Maintenance() -> Element {
    let app = use_app();
    let tasks = use_resource(move || async move { app.server().butler_tasks().await });
    rsx! {
        div { class: "panel",
            h3 { Icon { name: Ic::Wrench, class: "sm" } "Maintenance" }
            div { class: "setting",
                div { class: "label", b { "Clean bundles" } span { "Delete metadata bundles for media that no longer exists." } }
                div { class: "control", button { class: "btn sm", onclick: move |_| run_action(app, "Cleaning bundles…", |s| async move { s.clean_bundles().await }), "Run" } }
            }
            div { class: "setting",
                div { class: "label", b { "Optimize database" } span { "Compact and re-index the library database." } }
                div { class: "control", button { class: "btn sm", onclick: move |_| run_action(app, "Optimizing database…", |s| async move { s.optimize_database().await }), "Run" } }
            }
        }
        div { class: "panel",
            h3 { Icon { name: Ic::Clock, class: "sm" } "Scheduled tasks" }
            p { class: "desc", "Plex's butler runs these during the maintenance window. Start any of them now." }
            match &*tasks.read() {
                None => rsx! { Spinner {} },
                Some(Err(e)) => rsx! { ErrorView { message: e.to_string() } },
                Some(Ok(list)) => rsx! {
                    for t in list.clone() {
                        {
                            let name = t.name.clone().unwrap_or_default();
                            let (start, stop) = (name.clone(), name.clone());
                            rsx! {
                                div { class: "setting", key: "{name}",
                                    div { class: "label",
                                        b { "{t.title.clone().unwrap_or(name.clone())}" }
                                        span { "{t.description.clone().unwrap_or_default()}" }
                                    }
                                    div { class: "control",
                                        if !t.enabled { span { class: "badge", "DISABLED" } }
                                        button { class: "btn sm", onclick: move |_| { let n = start.clone(); run_action(app, "Task started", move |s| async move { s.start_butler_task(&n).await }) }, "Start" }
                                        button { class: "btn sm ghost", onclick: move |_| { let n = stop.clone(); run_action(app, "Task stopped", move |s| async move { s.stop_butler_task(&n).await }) }, "Stop" }
                                    }
                                }
                            }
                        }
                    }
                },
            }
        }
    }
}

#[component]
fn Prefs() -> Element {
    let app = use_app();
    let mut filter = use_signal(String::new);
    let mut show_advanced = use_signal(|| false);
    let prefs = use_resource(move || async move { app.server().prefs().await });
    let read = prefs.read();
    let list = match &*read {
        None => return rsx! { Spinner {} },
        Some(Err(e)) => return rsx! { ErrorView { message: e.to_string() } },
        Some(Ok(l)) => l.clone(),
    };
    let f = filter().to_lowercase();
    let shown: Vec<Setting> = list
        .into_iter()
        .filter(|s| !s.hidden && (show_advanced() || !s.advanced))
        .filter(|s| {
            f.is_empty()
                || s.label.as_deref().unwrap_or("").to_lowercase().contains(&f)
                || s.id.as_deref().unwrap_or("").to_lowercase().contains(&f)
                || s.summary.as_deref().unwrap_or("").to_lowercase().contains(&f)
        })
        .collect();
    rsx! {
        div { class: "toolbar", style: "margin-bottom: 14px",
            label { class: "search-box", style: "width: 360px",
                Icon { name: Ic::Search, class: "sm" }
                input { placeholder: "Filter settings", value: "{filter}", oninput: move |e| filter.set(e.value()) }
            }
            div { class: if show_advanced() { "chip active" } else { "chip" }, onclick: move |_| show_advanced.toggle(), "Show advanced" }
            span { class: "muted", "{shown.len()} settings" }
        }
        div { class: "panel",
            for s in shown {
                PrefRow { key: "{s.id:?}", setting: s }
            }
        }
    }
}

#[component]
fn PrefRow(setting: Setting) -> Element {
    let app = use_app();
    let mut value = use_signal(|| setting.value_string());
    let id = setting.id.clone().unwrap_or_default();
    let kind = setting.kind.clone().unwrap_or_default();
    let options = setting.enum_options();
    let original = setting.value_string();
    let save = move |id: String, v: String| {
        spawn(async move {
            match app.server().set_pref(&id, &v).await {
                Ok(()) => app.toast("Setting saved"),
                Err(e) => app.error(e),
            }
        });
    };
    let dirty = value() != original;
    let (id1, id2, id3) = (id.clone(), id.clone(), id.clone());
    rsx! {
        div { class: "pref-row",
            div { class: "label",
                b { "{setting.label.clone().unwrap_or(id.clone())}" }
                code { "{id}" }
                if let Some(s) = &setting.summary { span { "{s}" } }
            }
            div { class: "flex", style: "justify-content: flex-end",
                if kind == "bool" {
                    div {
                        class: if value() == "true" || value() == "1" { "toggle on" } else { "toggle" },
                        onclick: move |_| {
                            let on = value() == "true" || value() == "1";
                            let v = if on { "false" } else { "true" }.to_string();
                            value.set(v.clone());
                            save(id1.clone(), v);
                        }
                    }
                } else if !options.is_empty() {
                    select { class: "select", onchange: move |e| { value.set(e.value()); save(id2.clone(), e.value()); },
                        for (v, l) in options.clone() {
                            option { key: "{v}", value: "{v}", selected: v == value(), "{l}" }
                        }
                    }
                } else {
                    input { class: "input", value: "{value}", oninput: move |e| value.set(e.value()) }
                    if dirty {
                        button { class: "btn sm primary", onclick: move |_| save(id3.clone(), value()), "Save" }
                    }
                }
            }
        }
    }
}

#[component]
fn UsersDevices() -> Element {
    let app = use_app();
    let data = use_resource(move || async move {
        let s = app.server();
        let accounts = s.accounts().await.unwrap_or_default();
        let devices = s.devices().await.unwrap_or_default();
        let clients = s.clients().await.unwrap_or_default();
        (accounts, devices, clients)
    });
    let read = data.read();
    let Some((accounts, devices, clients)) = &*read else { return rsx! { Spinner {} } };
    rsx! {
        div { class: "panels",
            div { class: "panel",
                h3 { Icon { name: Ic::Users, class: "sm" } "Accounts" }
                for a in accounts.iter().filter(|a| a.id.unwrap_or(0) > 0) {
                    div { class: "setting", key: "{a.id:?}",
                        div { class: "label", b { "{a.name.clone().unwrap_or_default()}" }
                            span { "Audio: {a.default_audio_language.clone().unwrap_or(\"auto\".into())} · Subtitles: {a.default_subtitle_language.clone().unwrap_or(\"auto\".into())}" } }
                    }
                }
            }
            div { class: "panel",
                h3 { Icon { name: Ic::Tv, class: "sm" } "Players on the network" }
                if clients.is_empty() { p { class: "desc", "No players advertised on the local network right now." } }
                for c in clients.iter() {
                    div { class: "setting", key: "{c.machine_identifier:?}",
                        div { class: "label", b { "{c.name.clone().unwrap_or_default()}" } span { "{c.product.clone().unwrap_or_default()} · {c.address.clone().unwrap_or_default()}" } }
                    }
                }
            }
        }
        div { class: "panel", style: "margin-top: 18px",
            h3 { Icon { name: Ic::Server, class: "sm" } "Devices that have connected" }
            div { style: "max-height: 480px; overflow: auto",
                for d in devices.iter().rev() {
                    div { class: "setting", key: "{d.id:?}",
                        div { class: "label", b { "{d.name.clone().unwrap_or_default()}" } span { "{d.platform.clone().unwrap_or_default()}" } }
                        div { class: "control muted small", if let Some(t) = d.created_at { "first seen {fmt::date(t)}" } }
                    }
                }
            }
        }
    }
}
