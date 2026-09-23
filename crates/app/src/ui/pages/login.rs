use dioxus::core::spawn_forever;
use dioxus::prelude::*;
use patina_plex::{PlexTv, Resource, User};

use crate::session::{self, Stage};
use crate::state::use_app;
use crate::ui::icons::{Ic, Icon};

#[derive(Clone, PartialEq)]
enum Mode {
    Choose,
    Browser { url: String },
    Code { code: String },
    Password,
    Manual,
}

#[component]
pub fn Login() -> Element {
    let app = use_app();
    let mut mode = use_signal(|| Mode::Choose);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    // Bumped to abandon an in-flight PIN poll.
    let mut attempt = use_signal(|| 0u32);

    let mut start_pin = move |strong: bool| {
        error.set(None);
        busy.set(true);
        *attempt.write() += 1;
        let my_attempt = *attempt.peek();
        spawn_forever(async move {
            match session::create_pin(app, strong).await {
                Ok((pin, url)) => {
                    busy.set(false);
                    if strong {
                        let _ = open::that(&url);
                        mode.set(Mode::Browser { url });
                    } else {
                        mode.set(Mode::Code { code: pin.code.clone().unwrap_or_default() });
                    }
                    let Some(id) = pin.id else { return };
                    match session::wait_for_pin(app, id).await {
                        Ok(token) if *attempt.peek() == my_attempt => session::run(app, session::sign_in(app, token)),
                        Ok(_) => {}
                        Err(e) if *attempt.peek() == my_attempt => {
                            error.set(Some(e.to_string()));
                            mode.set(Mode::Choose);
                        }
                        Err(_) => {}
                    }
                }
                Err(e) => {
                    busy.set(false);
                    error.set(Some(format!("{e:#}")));
                }
            }
        });
    };
    let cancel = move |_| {
        *attempt.write() += 1;
        mode.set(Mode::Choose);
    };

    rsx! {
        div { class: "login",
            div { class: "login-bg" }
            div { class: "login-card",
                div { class: "brand", div { class: "brand-mark" } "patina" }
                match mode() {
                    Mode::Choose => rsx! {
                        p { "Your Plex libraries, beautifully presented — and played in ", b { "mpv" }, " with your own config." }
                        div { class: "stack",
                            button { class: "btn primary lg", disabled: busy(), onclick: move |_| start_pin(true),
                                Icon { name: Ic::ExternalLink, class: "sm" } "Sign in with Plex"
                            }
                            button { class: "btn", disabled: busy(), onclick: move |_| start_pin(false),
                                Icon { name: Ic::Link, class: "sm" } "Use a link code"
                            }
                        }
                        div { class: "divider", "or" }
                        div { class: "flex", style: "justify-content: center; gap: 18px;",
                            span { class: "link small", onclick: move |_| mode.set(Mode::Password), "Email & password" }
                            span { class: "link small", onclick: move |_| mode.set(Mode::Manual), "Connect to a server directly" }
                        }
                    },
                    Mode::Browser { url } => rsx! {
                        div { class: "center", style: "min-height: 110px", div { class: "spinner" } }
                        p { "Approve Patina in the browser window that just opened." }
                        div { class: "flex", style: "justify-content: center; gap: 18px; margin-top: 18px",
                            span { class: "link small", onclick: move |_| { let _ = open::that(&url); }, "Open it again" }
                            span { class: "link small", onclick: cancel, "Cancel" }
                        }
                    },
                    Mode::Code { code } => rsx! {
                        p { "Go to ", span { class: "link", onclick: |_| { let _ = open::that("https://plex.tv/link"); }, "plex.tv/link" }, " on any device and enter:" }
                        div { class: "pin-code", "{code}" }
                        div { class: "flex", style: "justify-content: center; color: var(--muted)", div { class: "spinner", style: "width: 16px; height: 16px; border-width: 2px" } span { class: "small", "Waiting for approval…" } }
                        div { style: "margin-top: 18px", span { class: "link small", onclick: cancel, "Cancel" } }
                    },
                    Mode::Password => rsx! { PasswordForm { on_back: move |_| mode.set(Mode::Choose) } },
                    Mode::Manual => rsx! { ManualForm { on_back: move |_| mode.set(Mode::Choose) } },
                }
                if let Some(e) = error() {
                    div { class: "error-box mt", "{e}" }
                }
            }
        }
    }
}

#[component]
fn PasswordForm(on_back: EventHandler<()>) -> Element {
    let app = use_app();
    let mut login = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut code = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let mut submit = move || {
        busy.set(true);
        error.set(None);
        let (l, p, c) = (login(), password(), code());
        spawn_forever(async move {
            let c = (!c.trim().is_empty()).then(|| c.trim().to_string());
            match session::password_sign_in(app, l, p, c).await {
                Ok(token) => session::run(app, session::sign_in(app, token)),
                Err(e) => {
                    busy.set(false);
                    error.set(Some(e.to_string()));
                }
            }
        });
    };
    rsx! {
        div { class: "stack",
            input { class: "input", placeholder: "Email or username", value: "{login}", oninput: move |e| login.set(e.value()) }
            input { class: "input", r#type: "password", placeholder: "Password", value: "{password}", oninput: move |e| password.set(e.value()),
                onkeydown: move |e: KeyboardEvent| if e.key() == Key::Enter { submit() } }
            input { class: "input", placeholder: "2FA code (if enabled)", value: "{code}", oninput: move |e| code.set(e.value()) }
            button { class: "btn primary", disabled: busy() || login().is_empty() || password().is_empty(), onclick: move |_| submit(), "Sign in" }
            if let Some(e) = error() { div { class: "error-box", "{e}" } }
            span { class: "link small", style: "text-align: center", onclick: move |_| on_back.call(()), "Back" }
        }
    }
}

#[component]
fn ManualForm(on_back: EventHandler<()>) -> Element {
    let app = use_app();
    let mut url = use_signal(|| "http://localhost:32400".to_string());
    let mut token = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let submit = move |_| {
        busy.set(true);
        error.set(None);
        let (u, t) = (url().trim().to_string(), token().trim().to_string());
        spawn_forever(async move {
            if let Err(e) = session::sign_in_manual(app, u, t).await {
                busy.set(false);
                error.set(Some(format!("{e:#}")));
            }
        });
    };
    rsx! {
        div { class: "stack",
            p { class: "small", style: "margin: 0", "Enter the server address and an X-Plex-Token. Watchlist and Plex Home features need a plex.tv sign-in." }
            input { class: "input", placeholder: "http://192.168.1.10:32400", value: "{url}", oninput: move |e| url.set(e.value()) }
            input { class: "input", placeholder: "X-Plex-Token", value: "{token}", oninput: move |e| token.set(e.value()) }
            button { class: "btn primary", disabled: busy() || token().trim().is_empty(), onclick: submit, "Connect" }
            if let Some(e) = error() { div { class: "error-box", "{e}" } }
            span { class: "link small", style: "text-align: center", onclick: move |_| on_back.call(()), "Back" }
        }
    }
}

#[component]
pub fn PickServer(servers: Vec<Resource>, user: User) -> Element {
    let app = use_app();
    let name = user.display_name().to_string();
    rsx! {
        div { class: "login",
            div { class: "login-bg" }
            div { class: "login-card",
                div { class: "brand", div { class: "brand-mark" } "patina" }
                p { "Welcome, {name}. Choose a server:" }
                div { class: "stack",
                    for s in servers.clone() {
                        div { class: "server-pick", key: "{s.id()}",
                            onclick: {
                                let s = s.clone();
                                let user = user.clone();
                                let all = servers.clone();
                                move |_| {
                                    let token = app.settings.peek().auth.token.clone();
                                    let Ok(tv) = PlexTv::new(app.client_info(), token) else { return };
                                    session::run(app, session::connect(app, tv, user.clone(), s.clone(), all.clone()));
                                }
                            },
                            div { class: "ic", Icon { name: Ic::Server } }
                            div { class: "grow",
                                b { "{s.name()}" }
                                span {
                                    if s.owned { "Owned" } else { "Shared by {s.source_title.clone().unwrap_or_default()}" }
                                    " · {s.platform.clone().unwrap_or_default()} · v{s.product_version.clone().unwrap_or_default()}"
                                }
                            }
                            if s.presence { span { class: "badge accent", "ONLINE" } } else { span { class: "badge", "OFFLINE?" } }
                        }
                    }
                }
                div { style: "margin-top: 16px", span { class: "link small", onclick: move |_| session::sign_out(app), "Use another account" } }
            }
        }
    }
}

#[component]
pub fn Failed(message: String) -> Element {
    let app = use_app();
    rsx! {
        div { class: "login",
            div { class: "login-bg" }
            div { class: "login-card",
                div { class: "brand", div { class: "brand-mark" } "patina" }
                div { class: "error-box", style: "text-align: left", "{message}" }
                div { class: "stack",
                    button { class: "btn primary", onclick: move |_| {
                            let mut stage = app.stage;
                            stage.set(Stage::Booting);
                            spawn_forever(session::restore(app));
                        },
                        Icon { name: Ic::Refresh, class: "sm" } "Try again"
                    }
                    button { class: "btn", onclick: move |_| session::sign_out(app), Icon { name: Ic::LogOut, class: "sm" } "Sign out" }
                }
            }
        }
    }
}
