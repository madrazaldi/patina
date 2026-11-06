//! Everything before the library: splash, sign-in, server choice, failure,
//! and, when the server can't be reached, playback of downloads offline.

use crate::prelude::*;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use patina_core::session;
use patina_core::state::Stage;

use crate::core::{bg, core, message};
use crate::store::store;

/// What the gate shows, read from the store.
struct Seen {
    stage: Stage,
    can_offline: bool,
    tint: Hsla,
}

#[derive(Clone, PartialEq)]
enum Mode {
    Choose,
    Browser { url: String },
    Code { code: String },
    Password,
    Manual,
}

pub struct Gate {
    mode: Mode,
    busy: bool,
    error: Option<String>,
    login: Entity<InputState>,
    password: Entity<InputState>,
    code: Entity<InputState>,
    url: Entity<InputState>,
    token: Entity<InputState>,
    /// Showing downloads instead of the failure screen.
    offline: Option<AnyView>,
    _subs: Vec<Subscription>,
}

impl Gate {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = |placeholder: &str, window: &mut Window, cx: &mut Context<Self>| {
            let p = placeholder.to_string();
            cx.new(|cx| InputState::new(window, cx).placeholder(p))
        };
        let login = input("Email or username", window, cx);
        let password = cx.new(|cx| InputState::new(window, cx).placeholder("Password").masked(true));
        let code = input("Two-factor code (if enabled)", window, cx);
        let url = cx.new(|cx| InputState::new(window, cx).placeholder("http://192.168.1.10:32400").default_value("http://localhost:32400"));
        let token = input("X-Plex-Token", window, cx);
        let mut subs = vec![cx.observe(&store(cx), |_, _, cx| cx.notify())];
        for state in [&login, &password, &code, &url, &token] {
            subs.push(cx.subscribe_in(state, window, |this, _, e: &InputEvent, window, cx| match e {
                InputEvent::PressEnter { .. } => this.submit(window, cx),
                InputEvent::Change => cx.notify(),
                _ => {}
            }));
        }
        Self { mode: Mode::Choose, busy: false, error: None, login, password, code, url, token, offline: None, _subs: subs }
    }

    fn start_pin(&mut self, strong: bool, cx: &mut Context<Self>) {
        self.error = None;
        self.busy = true;
        cx.notify();
        let app = core(cx);
        cx.spawn(async move |this, cx| {
            let pin = bg({
                let app = app.clone();
                async move { session::create_pin(&app, strong).await }
            })
            .await;
            let pin = match pin {
                Ok(pin) => pin,
                Err(e) => {
                    this.update(cx, |g, cx| g.fail(message(&e), cx)).ok();
                    return;
                }
            };
            this.update(cx, |g, cx| {
                g.busy = false;
                g.mode = if strong { Mode::Browser { url: pin.url.clone() } } else { Mode::Code { code: pin.code.clone() } };
                if strong {
                    cx.open_url(&pin.url);
                }
                cx.notify();
            })
            .ok();
            // Resolves once signed in (the stage changes) or when cancelled.
            let (id, attempt) = (pin.id, pin.attempt);
            if let Err(e) = bg(async move { session::complete_pin(&app, id, attempt).await }).await {
                this.update(cx, |g, cx| g.fail(message(&e), cx)).ok();
            }
        })
        .detach();
    }

    fn fail(&mut self, error: String, cx: &mut Context<Self>) {
        self.error = Some(error);
        self.mode = Mode::Choose;
        self.busy = false;
        cx.notify();
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        session::cancel_pin(&core(cx));
        self.mode = Mode::Choose;
        cx.notify();
    }

    fn back(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        self.mode = Mode::Choose;
        cx.notify();
    }

    fn submit(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let app = core(cx);
        let fut: std::pin::Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>> = match self.mode {
            Mode::Password => {
                let (login, password, code) = (self.login.read(cx).value().to_string(), self.password.read(cx).value().to_string(), self.code.read(cx).value().to_string());
                if login.is_empty() || password.is_empty() {
                    return;
                }
                Box::pin(async move {
                    let code = Some(code.as_str()).filter(|c| !c.trim().is_empty());
                    session::password_sign_in(&app, &login, &password, code).await
                })
            }
            Mode::Manual => {
                let (url, token) = (self.url.read(cx).value().to_string(), self.token.read(cx).value().to_string());
                if token.trim().is_empty() {
                    return;
                }
                Box::pin(async move { session::sign_in_manual(&app, &url, token.trim()).await })
            }
            _ => return,
        };
        self.busy = true;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = bg(fut).await;
            this.update(cx, |g, cx| {
                g.busy = false;
                if let Err(e) = result {
                    g.error = Some(message(&e));
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn retry(cx: &mut App) {
        let app = core(cx);
        cx.background_spawn(bg(async move { session::retry(&app).await })).detach();
    }

    fn link(id: &'static str, text: &'static str, cx: &mut Context<Self>, f: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static) -> Button {
        Button::new(id).link().small().label(text).text_color(cx.theme().muted_foreground).on_click(cx.listener(move |this, _, window, cx| f(this, window, cx)))
    }

    fn body(&mut self, store: &Seen, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let muted = cx.theme().muted_foreground;
        let text = |s: String| div().text_sm().text_color(muted).text_center().child(s).into_any_element();
        let mut out: Vec<AnyElement> = Vec::new();
        match &store.stage {
            Stage::Booting | Stage::Ready => out.push(Spinner::new().into_any_element()),
            Stage::Connecting { name } => {
                out.push(Spinner::new().into_any_element());
                out.push(text(format!("Connecting to {name}…")));
            }
            Stage::Failed { message } => {
                out.push(Alert::error("failed", message.clone()).into_any_element());
                let can_offline = store.can_offline;
                out.push(
                    v_flex()
                        .w_full()
                        .gap_2()
                        .child(Button::new("retry").primary().label("Try again").on_click(|_, _, cx| Self::retry(cx)))
                        .when(can_offline, |d| {
                            d.child(Button::new("offline").outline().label("Play downloads offline").on_click(cx.listener(|this, _, window, cx| {
                                this.offline = Some(crate::pages::offline_downloads(window, cx));
                                cx.notify();
                            })))
                        })
                        .child(Button::new("signout").outline().label("Sign out").on_click(|_, _, cx| session::sign_out(&core(cx))))
                        .into_any_element(),
                );
            }
            Stage::PickServer { servers, user } => {
                out.push(text(format!("Signed in as {}. Choose a server.", user.name)));
                let tint = store.tint;
                out.push(
                    v_flex()
                        .w_full()
                        .gap_2()
                        .children(servers.iter().map(|s| {
                            let id = s.id.clone();
                            let mut desc = if s.owned { "Owned".to_string() } else { format!("Shared by {}", s.source_title.as_deref().unwrap_or("someone")) };
                            if let Some(p) = &s.platform {
                                desc.push_str(&format!(" · {p}"));
                            }
                            if let Some(v) = &s.version {
                                desc.push_str(&format!(" · v{v}"));
                            }
                            h_flex()
                                .id(SharedString::from(format!("server-{}", s.id)))
                                .w_full()
                                .gap_3()
                                .px_3()
                                .py_2()
                                .rounded(px(8.))
                                .border_1()
                                .border_color(cx.theme().border)
                                .cursor_pointer()
                                .hover(|s| s.bg(cx.theme().muted.opacity(0.5)))
                                .on_click(move |_, _, cx| {
                                    let app = core(cx);
                                    let id = id.clone();
                                    cx.background_spawn(bg(async move { session::run(&app, session::pick_server(&app, &id)).await })).detach();
                                })
                                .child(v_flex().flex_1().min_w_0().child(div().text_sm().font_medium().child(s.name.clone())).child(div().text_xs().text_color(muted).truncate().child(desc)))
                                .child(
                                    div()
                                        .px_2()
                                        .py_0p5()
                                        .rounded(px(6.))
                                        .border_1()
                                        .text_xs()
                                        .when(s.presence, |d| d.border_color(tint.opacity(0.4)).text_color(tint).bg(tint.opacity(0.1)))
                                        .when(!s.presence, |d| d.border_color(cx.theme().border))
                                        .child(if s.presence { "Online" } else { "Offline?" }),
                                )
                        }))
                        .into_any_element(),
                );
                out.push(Button::new("another").link().small().label("Use another account").on_click(|_, _, cx| session::sign_out(&core(cx))).into_any_element());
            }
            Stage::Login => match self.mode.clone() {
                Mode::Choose => {
                    out.push(text("A Plex client that plays everything in mpv, with your own mpv configuration.".into()));
                    out.push(
                        v_flex()
                            .w_full()
                            .gap_2()
                            .child(Button::new("plex").primary().large().label("Sign in with Plex").disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.start_pin(true, cx))))
                            .child(Button::new("code").outline().large().label("Use a link code").disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.start_pin(false, cx))))
                            .into_any_element(),
                    );
                    out.push(
                        h_flex()
                            .gap_4()
                            .child(Self::link("password", "Email and password", cx, |this, window, cx| {
                                this.mode = Mode::Password;
                                this.login.update(cx, |s, cx| s.focus(window, cx));
                                cx.notify();
                            }))
                            .child(Self::link("manual", "Connect to a server directly", cx, |this, window, cx| {
                                this.mode = Mode::Manual;
                                this.url.update(cx, |s, cx| s.focus(window, cx));
                                cx.notify();
                            }))
                            .into_any_element(),
                    );
                }
                Mode::Browser { url } => {
                    out.push(Spinner::new().into_any_element());
                    out.push(text("Approve Patina in the browser window that just opened.".into()));
                    out.push(
                        h_flex()
                            .gap_4()
                            .child(Self::link("again", "Open it again", cx, move |_, _, cx| cx.open_url(&url)))
                            .child(Self::link("cancel", "Cancel", cx, |this, _, cx| this.cancel(cx)))
                            .into_any_element(),
                    );
                }
                Mode::Code { code } => {
                    out.push(
                        h_flex()
                            .gap_1()
                            .text_sm()
                            .text_color(muted)
                            .child("Go to")
                            .child(Button::new("link").link().small().label("plex.tv/link").on_click(|_, _, cx| cx.open_url("https://plex.tv/link")))
                            .child("on any device and enter")
                            .into_any_element(),
                    );
                    out.push(
                        div()
                            .w_full()
                            .py_3()
                            .rounded(px(8.))
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().muted.opacity(0.5))
                            .text_center()
                            .text_size(px(36.))
                            .font_family(cx.theme().mono_font_family.clone())
                            .child(code)
                            .into_any_element(),
                    );
                    out.push(text("Waiting for approval…".into()));
                    out.push(Self::link("cancel", "Cancel", cx, |this, _, cx| this.cancel(cx)).into_any_element());
                }
                Mode::Password => {
                    let ready = !self.login.read(cx).value().is_empty() && !self.password.read(cx).value().is_empty();
                    out.push(
                        v_flex()
                            .w_full()
                            .gap_2()
                            .child(Input::new(&self.login))
                            .child(Input::new(&self.password))
                            .child(Input::new(&self.code))
                            .child(Button::new("signin").primary().label("Sign in").disabled(self.busy || !ready).on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))))
                            .into_any_element(),
                    );
                    out.push(Self::link("back", "Back", cx, |this, _, cx| this.back(cx)).into_any_element());
                }
                Mode::Manual => {
                    let ready = !self.token.read(cx).value().trim().is_empty();
                    out.push(text("Enter the server address and an X-Plex-Token. Watchlist and Plex Home need a plex.tv sign-in.".into()));
                    out.push(
                        v_flex()
                            .w_full()
                            .gap_2()
                            .child(Input::new(&self.url))
                            .child(Input::new(&self.token))
                            .child(Button::new("connect").primary().label("Connect").disabled(self.busy || !ready).on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))))
                            .into_any_element(),
                    );
                    out.push(Self::link("back", "Back", cx, |this, _, cx| this.back(cx)).into_any_element());
                }
            },
        }
        if let Some(e) = &self.error {
            out.push(Alert::error("error", e.clone()).into_any_element());
        }
        let _ = window;
        out
    }
}

impl Render for Gate {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = {
            let s = store(cx).read(cx);
            Seen {
                stage: s.stage.clone(),
                can_offline: s.downloads.iter().any(|d| matches!(d.state, patina_core::downloads::State::Done)),
                tint: s.tint(),
            }
        };
        let failed_or_connecting = matches!(store.stage, Stage::Failed { .. } | Stage::Connecting { .. });
        if let Some(offline) = self.offline.clone().filter(|_| failed_or_connecting) {
            let connecting = match &store.stage {
                Stage::Connecting { name } => Some(name.clone()),
                _ => None,
            };
            return v_flex()
                .size_full()
                .child(
                    h_flex()
                        .h(px(48.))
                        .flex_none()
                        .items_center()
                        .gap_3()
                        .pl(px(84.))
                        .pr_6()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(div().text_size(px(17.)).font_semibold().child("Patina"))
                        .child(div().text_sm().text_color(cx.theme().muted_foreground).child(match &connecting {
                            Some(name) => format!("Connecting to {name}…"),
                            None => "Offline".to_string(),
                        }))
                        .child(div().flex_1())
                        .child(match connecting {
                            Some(_) => Spinner::new().into_any_element(),
                            None => Button::new("retry").outline().small().label("Try again").on_click(|_, _, cx| Self::retry(cx)).into_any_element(),
                        }),
                )
                .child(div().flex_1().min_h_0().child(offline))
                .child(crate::player::now_playing_bar(window, cx))
                .into_any_element();
        }
        let body = self.body(&store, window, cx);
        div()
            .id("gate")
            .size_full()
            .flex()
            .overflow_y_scroll()
            .p_10()
            .child(
                v_flex()
                    .m_auto()
                    .w_full()
                    .max_w(px(384.))
                    .items_center()
                    .gap_6()
                    .child(div().text_size(px(36.)).font_semibold().child("Patina"))
                    .child(
                        v_flex()
                            .w_full()
                            .items_center()
                            .gap_4()
                            .p_6()
                            .rounded(px(12.))
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().colors.group_box)
                            .children(body),
                    ),
            )
            .into_any_element()
    }
}
