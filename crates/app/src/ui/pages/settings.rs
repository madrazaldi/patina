use dioxus::core::spawn_forever;
use dioxus::prelude::*;
use patina_plex::HomeUser;

use crate::config::{self, ACCENTS, QUALITIES};
use crate::fmt;
use crate::session;
use crate::state::use_app;
use crate::ui::components::Img;
use crate::ui::icons::{Ic, Icon};

#[component]
pub fn SettingsPage() -> Element {
    rsx! {
        div { class: "page", style: "max-width: 1080px",
            div { class: "page-head", div { h1 { "Settings" } div { class: "sub", "Saved to {config::config_dir().join(\"config.toml\").display()}" } } }
            PlayerSettings {}
            Appearance {}
            Account {}
        }
    }
}

#[component]
fn Toggle(on: bool, onchange: EventHandler<bool>) -> Element {
    rsx! { div { class: if on { "toggle on" } else { "toggle" }, onclick: move |_| onchange.call(!on) } }
}

#[component]
fn PlayerSettings() -> Element {
    let app = use_app();
    let p = app.settings.read().player.clone();
    let mut probe = use_resource(move || async move {
        let program = app.settings.peek().player.mpv_program();
        (program.clone(), patina_mpv::probe(&program).await.map_err(|e| e.to_string()))
    });
    let mpv_dir = config::mpv_config_dir();
    let mpv_dir_str = mpv_dir.display().to_string();
    let mpv_conf_exists = mpv_dir.join("mpv.conf").exists();

    rsx! {
        div { class: "panel",
            h3 { Icon { name: Ic::Play, class: "sm" } "Player" }
            p { class: "desc", "Everything plays in mpv, using your normal mpv configuration, scripts and key bindings." }

            div { class: "setting",
                div { class: "label",
                    b { "mpv executable" }
                    match &*probe.read() {
                        Some((prog, Ok(v))) => rsx! { span { style: "color: var(--accent)", "✓ {v} — {prog}" } },
                        Some((prog, Err(e))) => rsx! { span { style: "color: var(--danger)", "Could not run {prog}: {e}" } },
                        None => rsx! { span { "Checking…" } },
                    }
                }
                div { class: "control",
                    input { class: "input", style: "width: 260px", placeholder: "Auto-detect", value: "{p.mpv_path}",
                        oninput: move |e| app.update_settings(|s| s.player.mpv_path = e.value()) }
                    button { class: "btn sm", onclick: move |_| probe.restart(), "Test" }
                }
            }
            div { class: "setting",
                div { class: "label",
                    b { "mpv configuration" }
                    span { {format!("{mpv_dir_str}{}", if mpv_conf_exists { " — mpv.conf found" } else { "" })} }
                }
                div { class: "control",
                    button { class: "btn sm", onclick: move |_| { let _ = std::fs::create_dir_all(&mpv_dir); let _ = open::that(&mpv_dir); }, Icon { name: Ic::Folder, class: "sm" } "Open folder" }
                }
            }
            div { class: "setting",
                div { class: "label", b { "Extra mpv arguments" } span { "Appended to the command line, e.g. --fs --profile=hq. Your mpv.conf is always loaded." } }
                div { class: "control",
                    input { class: "input", style: "width: 320px", placeholder: "--fs", value: "{p.extra_args}",
                        oninput: move |e| app.update_settings(|s| s.player.extra_args = e.value()) }
                }
            }
            div { class: "setting",
                div { class: "label", b { "Streaming quality" } span { "Original plays the file directly. Lower qualities ask the server to transcode (useful on slow connections)." } }
                div { class: "control",
                    select { class: "select", style: "width: 240px", onchange: move |e| app.update_settings(|s| s.player.quality = e.value()),
                        for q in QUALITIES {
                            option { key: "{q.id}", value: "{q.id}", selected: q.id == p.quality, "{q.label}" }
                        }
                    }
                }
            }
            div { class: "setting",
                div { class: "label", b { "Autoplay next episode" } span { "Queue the following episodes when you play one." } }
                div { class: "control", Toggle { on: p.continuous, onchange: move |v| app.update_settings(|s| s.player.continuous = v) } }
            }
            div { class: "setting",
                div { class: "label", b { "Sync audio & subtitle tracks" } span { "Start with the tracks selected in Plex, and remember tracks you switch to in mpv." } }
                div { class: "control", Toggle { on: p.sync_streams, onchange: move |v| app.update_settings(|s| s.player.sync_streams = v) } }
            }
            div { class: "setting",
                div { class: "label", b { "Skip intros automatically" } span { "Uses Plex intro markers. Otherwise press the skip key when prompted." } }
                div { class: "control", Toggle { on: p.auto_skip_intro, onchange: move |v| app.update_settings(|s| s.player.auto_skip_intro = v) } }
            }
            div { class: "setting",
                div { class: "label", b { "Skip credits automatically" } span { "Jumps past credits — or straight to the next episode." } }
                div { class: "control", Toggle { on: p.auto_skip_credits, onchange: move |v| app.update_settings(|s| s.player.auto_skip_credits = v) } }
            }
            div { class: "setting",
                div { class: "label", b { "Skip key" } span { "mpv key name that skips an intro or credits (bound only while Patina controls mpv)." } }
                div { class: "control",
                    input { class: "input", style: "width: 140px", value: "{p.skip_key}", oninput: move |e| app.update_settings(|s| s.player.skip_key = e.value()) }
                }
            }
            div { class: "setting",
                div { class: "label", b { "Mark as watched after" } span { "How much of an item must be played before it counts as watched." } }
                div { class: "control",
                    select { class: "select", style: "width: 140px",
                        onchange: move |e| app.update_settings(|s| s.player.watched_threshold = e.value().parse().unwrap_or(0.9)),
                        for t in [0.8, 0.85, 0.9, 0.95] {
                            option { key: "{t}", value: "{t}", selected: (t - p.watched_threshold).abs() < 0.001, "{t * 100.0:.0}%" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Appearance() -> Element {
    let app = use_app();
    let ui = app.settings.read().ui.clone();
    rsx! {
        div { class: "panel",
            h3 { Icon { name: Ic::Sparkles, class: "sm" } "Appearance" }
            div { class: "setting",
                div { class: "label", b { "Accent colour" } }
                div { class: "control",
                    for a in ACCENTS {
                        div {
                            key: "{a.id}",
                            title: "{a.label}",
                            onclick: move |_| app.update_settings(|s| s.ui.accent = a.id.to_string()),
                            style: format!(
                                "width: 28px; height: 28px; border-radius: 50%; cursor: pointer; background: {}; box-shadow: {};",
                                a.color,
                                if ui.accent == a.id { format!("0 0 0 3px var(--bg), 0 0 0 5px {}", a.color) } else { "none".into() }
                            ),
                        }
                    }
                }
            }
            div { class: "setting",
                div { class: "label", b { "Poster size" } span { "{ui.poster_size}px" } }
                div { class: "control",
                    input { r#type: "range", min: "120", max: "240", step: "4", value: "{ui.poster_size}", style: "width: 240px; accent-color: var(--accent)",
                        oninput: move |e| app.update_settings(|s| s.ui.poster_size = e.value().parse().unwrap_or(168)) }
                }
            }
            div { class: "setting",
                div { class: "label", b { "Hide spoilers" } span { "Blur thumbnails and summaries of episodes you haven't watched (hover to reveal)." } }
                div { class: "control", Toggle { on: ui.hide_spoilers, onchange: move |v| app.update_settings(|s| s.ui.hide_spoilers = v) } }
            }
        }
    }
}

#[component]
fn Account() -> Element {
    let app = use_app();
    let session = app.session.read().clone();
    let user = session.as_ref().and_then(|s| s.user.clone());
    let has_tv = session.as_ref().and_then(|s| s.tv.as_ref()).is_some();
    let home = use_resource(move || async move {
        match app.tv() {
            Some(tv) => tv.home_users().await.unwrap_or_default(),
            None => Vec::new(),
        }
    });
    let mut pin_for = use_signal(|| None::<HomeUser>);
    let mut pin = use_signal(String::new);
    let current_uuid = user.as_ref().and_then(|u| u.uuid.clone());

    let mut switch = move |u: HomeUser, pin_code: Option<String>| {
        let uuid = u.uuid.clone().unwrap_or_default();
        pin_for.set(None);
        spawn_forever(async move {
            if let Err(e) = session::switch_user(app, uuid, pin_code).await {
                app.error(format!("{e:#}"));
                session::restore(app).await;
            }
        });
    };

    rsx! {
        div { class: "panel",
            h3 { Icon { name: Ic::User, class: "sm" } "Account" }
            if let Some(u) = &user {
                div { class: "setting",
                    div { class: "label flex",
                        div { class: "avatar", style: "width: 44px; height: 44px",
                            if u.thumb.is_some() { Img { src: u.thumb.clone() } } else { "{fmt::initials(u.display_name())}" }
                        }
                        div {
                            b { "{u.display_name()}" }
                            span { {format!("{}{}", u.email.clone().unwrap_or_default(), if u.subscription.as_ref().is_some_and(|s| s.active) { " · Plex Pass" } else { "" })} }
                        }
                    }
                    div { class: "control", button { class: "btn sm danger", onclick: move |_| session::sign_out(app), Icon { name: Ic::LogOut, class: "sm" } "Sign out" } }
                }
            } else {
                div { class: "setting",
                    div { class: "label", b { "Direct server connection" } span { "Not signed in to plex.tv." } }
                    div { class: "control", button { class: "btn sm", onclick: move |_| session::sign_out(app), "Sign in with Plex" } }
                }
            }
            if has_tv {
                if let Some(users) = home.read().as_ref().filter(|u| u.len() > 1) {
                    div { class: "setting stack",
                        div { class: "label", b { "Plex Home" } span { "Switch to another member of your Plex Home." } }
                        div { class: "control flex wrap",
                            for u in users.clone() {
                                {
                                    let active = u.uuid.is_some() && u.uuid == current_uuid;
                                    let target = u.clone();
                                    rsx! {
                                        div { class: if active { "chip active" } else { "chip" }, key: "{u.uuid:?}",
                                            onclick: move |_| {
                                                if active { return; }
                                                if target.protected { pin.set(String::new()); pin_for.set(Some(target.clone())); } else { switch(target.clone(), None); }
                                            },
                                            div { class: "avatar", style: "width: 20px; height: 20px; font-size: 9px",
                                                if u.thumb.is_some() { Img { src: u.thumb.clone() } } else { "{fmt::initials(u.name())}" }
                                            }
                                            "{u.name()}"
                                            if u.protected { Icon { name: Ic::Info, class: "sm" } }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "setting",
                div { class: "label", b { "Version" } span { "Patina {config::VERSION}" } }
            }
        }
        if let Some(u) = pin_for() {
            div { class: "modal-backdrop", onclick: move |_| pin_for.set(None),
                div { class: "modal", onclick: move |e| e.stop_propagation(),
                    h2 { "Enter PIN" }
                    p { class: "desc", "{u.name()}'s profile is protected." }
                    input { class: "input", r#type: "password", autofocus: true, maxlength: "4", value: "{pin}",
                        oninput: move |e| pin.set(e.value()),
                        onkeydown: {
                            let u = u.clone();
                            move |e: KeyboardEvent| if e.key() == Key::Enter { switch(u.clone(), Some(pin())); }
                        }
                    }
                    div { class: "foot",
                        button { class: "btn ghost", onclick: move |_| pin_for.set(None), "Cancel" }
                        button { class: "btn primary", onclick: move |_| switch(u.clone(), Some(pin())), "Switch" }
                    }
                }
            }
        }
    }
}
