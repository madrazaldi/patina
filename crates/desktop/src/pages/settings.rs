//! Settings: the player, automatic playback, appearance, Seerr and the account.

use crate::prelude::*;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::switch::Switch;
use patina_core::config::{self, PlayerSettings};
use patina_core::ops::setup;
use patina_core::prefs::{self, ForcedMode, HdrMode, Resolution, Rules, ShowRules, SubtitleMode};
use patina_core::seerr::{self, SeerrStatus};
use patina_core::seerr::client::SeerrAuthKind;
use patina_core::session;
use patina_plex::HomeUser;

use crate::art::art_of;
use crate::core::{bg, blocking, core, message};
use crate::format;
use crate::nav::{self, Route};
use crate::store::{self, store};
use crate::theme;
use crate::widgets;

// Automatic playback rules ---------------------------------------------------------

const SUBTITLES: [(SubtitleMode, &str); 4] =
    [(SubtitleMode::Keep, "Leave as Plex has them"), (SubtitleMode::Off, "Off"), (SubtitleMode::Always, "Always on"), (SubtitleMode::Foreign, "Only for foreign dialogue")];
const FORCED: [(ForcedMode, &str); 4] = [
    (ForcedMode::Auto, "Full subtitles (forced if nothing else)"),
    (ForcedMode::Prefer, "Forced first, then full"),
    (ForcedMode::Avoid, "Never forced"),
    (ForcedMode::Only, "Only forced"),
];
const RESOLUTION: [(Resolution, &str); 5] = [(Resolution::Any, "Any"), (Resolution::Uhd, "4K"), (Resolution::Fhd, "1080p"), (Resolution::Hd, "720p"), (Resolution::Sd, "SD")];
const CODECS: [(&str, &str); 4] = [("h264", "H.264"), ("hevc", "HEVC (H.265)"), ("av1", "AV1"), ("vp9", "VP9")];
const HDR: [(HdrMode, &str); 3] = [(HdrMode::Any, "Any"), (HdrMode::Prefer, "Prefer HDR"), (HdrMode::Avoid, "Avoid HDR (SDR)")];
/// Writes one player setting.
type SetText = fn(&mut PlayerSettings, String);
type SetFlag = fn(&mut PlayerSettings, bool);

const THRESHOLDS: [f64; 4] = [0.8, 0.85, 0.9, 0.95];

/// "eng, ja; French" → the languages, `None` when there are none.
fn parts(text: &str) -> Option<Vec<String>> {
    let v: Vec<String> = text.split([',', ';']).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect();
    (!v.is_empty()).then_some(v)
}

/// "English audio · subtitles for foreign dialogue · 1080p"
fn summary(r: &Rules) -> String {
    let mut bits = Vec::new();
    if let Some(a) = r.audio_languages.as_ref().filter(|a| !a.is_empty()) {
        bits.push(format!("{} audio", a.join(", ")));
    }
    match r.subtitles {
        None | Some(SubtitleMode::Keep) => {}
        Some(SubtitleMode::Off) => bits.push("no subtitles".into()),
        Some(SubtitleMode::Always) => bits.push(match r.subtitle_languages.as_ref().filter(|l| !l.is_empty()) {
            Some(l) => format!("subtitles ({})", l.join(", ")),
            None => "subtitles".into(),
        }),
        Some(SubtitleMode::Foreign) => bits.push("subtitles for foreign dialogue".into()),
    }
    if let Some(f) = r.forced.filter(|f| *f != ForcedMode::Auto) {
        bits.push(format!("{} forced", serde_plain(&f)));
    }
    if let Some(c) = r.avoid_commentary {
        bits.push(if c { "skip commentary" } else { "allow commentary" }.into());
    }
    if let Some(res) = r.resolution.filter(|r| *r != Resolution::Any) {
        bits.push(res.label().into());
    }
    if let Some(c) = r.codec.as_ref().filter(|c| !c.is_empty()) {
        bits.push(c.clone());
    }
    match r.hdr {
        Some(HdrMode::Prefer) => bits.push("HDR".into()),
        Some(HdrMode::Avoid) => bits.push("SDR".into()),
        _ => {}
    }
    if bits.is_empty() { "No rules".into() } else { bits.join(" · ") }
}

/// The serde name of a rule value ("prefer", "only").
fn serde_plain<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_value(v).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

fn labels<T: Copy>(l: &[(T, &str)]) -> Vec<(T, String)> {
    l.iter().map(|(v, l)| (*v, l.to_string())).collect()
}

/// A select-like button: `(value, label)` choices, `None` for "Not set".
fn select<T: Clone + PartialEq + 'static>(
    id: impl Into<ElementId>,
    width: Pixels,
    current: Option<T>,
    unset: Option<&'static str>,
    choices: Vec<(T, String)>,
    pick: impl Fn(Option<T>, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let shown = match &current {
        Some(c) => choices.iter().find(|(v, _)| v == c).map(|(_, l)| l.clone()).unwrap_or_default(),
        None => unset.unwrap_or_default().to_string(),
    };
    let pick = Rc::new(pick);
    Button::new(id)
        .outline()
        .small()
        .w(width)
        .justify_between()
        .child(div().flex_1().min_w_0().truncate().text_left().child(shown))
        .dropdown_caret(true)
        .dropdown_menu(move |mut menu, _, _| {
            if let Some(u) = unset {
                let pick = pick.clone();
                menu = menu.item(PopupMenuItem::new(u).checked(current.is_none()).on_click(move |_, w, cx| pick(None, w, cx)));
            }
            for (v, l) in &choices {
                let (v, pick) = (v.clone(), pick.clone());
                menu = menu.item(PopupMenuItem::new(l.clone()).checked(current.as_ref() == Some(&v)).on_click(move |_, w, cx| pick(Some(v.clone()), w, cx)));
            }
            menu
        })
}

/// A label with an optional description, and a control on the right.
fn row(first: bool, title: impl Into<SharedString>, desc: Option<SharedString>, desc_color: Option<Hsla>, control: Option<AnyElement>, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .flex_none()
        .items_center()
        .justify_between()
        .gap_6()
        .px_4()
        .py_3p5()
        .when(!first, |d| d.border_t_1().border_color(theme.border))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(div().text_sm().font_medium().child(title.into()))
                .when_some(desc, |d, s| d.child(div().text_size(px(13.)).text_color(desc_color.unwrap_or(theme.muted_foreground)).child(s))),
        )
        .when_some(control, |d, c| d.child(h_flex().flex_none().items_center().gap_2().child(c)))
}

/// A card with a title, an optional description and action, and divided rows.
fn card(title: &str, desc: Option<&str>, action: Option<AnyElement>, cx: &App) -> Div {
    let theme = cx.theme();
    v_flex().flex_none().rounded(px(12.)).border_1().border_color(theme.border).bg(theme.colors.group_box).overflow_hidden().child(
        h_flex()
            .items_start()
            .justify_between()
            .gap_4()
            .px_6()
            .py_5()
            .border_b_1()
            .border_color(theme.border)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1p5()
                    .child(div().text_base().font_semibold().child(title.to_string()))
                    .when_some(desc, |d, t| d.child(div().text_sm().text_color(theme.muted_foreground).child(t.to_string()))),
            )
            .children(action),
    )
}

fn alert(text: String, cx: &App) -> Div {
    let theme = cx.theme();
    div().p_4().child(div().px_4().py_3().rounded(px(8.)).border_1().border_color(theme.danger).text_sm().text_color(theme.danger).child(text))
}

// The page ---------------------------------------------------------------------------

pub struct Settings {
    probe: Option<Result<String, String>>,
    probe_task: Option<Task<()>>,
    mpv_path: Entity<InputState>,
    extra_args: Entity<InputState>,
    skip_key: Entity<InputState>,
    discord_id: Entity<InputState>,
    downloads_dir: Entity<InputState>,
    poster: Entity<SliderState>,
    profile: Option<Rules>,
    shows: Vec<ShowRules>,
    prefs_error: Option<String>,
    audio: Entity<InputState>,
    subtitle_langs: Entity<InputState>,
    seerr: Option<SeerrStatus>,
    seerr_error: Option<String>,
    seerr_url: Entity<InputState>,
    seerr_key: Entity<InputState>,
    connecting: bool,
    home: Vec<HomeUser>,
    pin: Entity<InputState>,
    pin_for: Option<HomeUser>,
    scope: String,
    seerr_seen: Option<String>,
    _subs: Vec<Subscription>,
}

impl Settings {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let p = store(cx).read(cx).settings.player.clone();
        let poster_size = store(cx).read(cx).settings.ui.poster_size;
        let input = |placeholder: &str, value: &str, window: &mut Window, cx: &mut Context<Self>| {
            let (placeholder, value) = (placeholder.to_string(), value.to_string());
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder).default_value(value))
        };
        let mpv_path = input("Auto-detect", &p.mpv_path, window, cx);
        let extra_args = input("--fs", &p.extra_args, window, cx);
        let skip_key = input("", &p.skip_key, window, cx);
        let discord_id = input("Application ID", &p.discord_app_id, window, cx);
        let downloads_dir = input("Default (Movies/Patina)", &p.downloads_dir, window, cx);
        let audio = input("Not set", "", window, cx);
        let subtitle_langs = input("Not set", "", window, cx);
        let seerr_url = input("http://seerr.local:5055", "", window, cx);
        let seerr_key = cx.new(|cx| InputState::new(window, cx).placeholder("Optional").masked(true));
        let pin = cx.new(|cx| InputState::new(window, cx).masked(true));
        let poster = cx.new(|_| SliderState::new().max(240.).min(120.).step(4.).default_value(poster_size as f32));

        // Each text setting is saved as it is typed.
        let mut subs = Vec::new();
        let fields: [(&Entity<InputState>, SetText); 5] = [
            (&mpv_path, |p, v| p.mpv_path = v),
            (&extra_args, |p, v| p.extra_args = v),
            (&skip_key, |p, v| p.skip_key = v),
            (&discord_id, |p, v| p.discord_app_id = v.trim().to_string()),
            (&downloads_dir, |p, v| p.downloads_dir = v),
        ];
        for (input, set) in fields {
            subs.push(cx.subscribe_in(input, window, move |_, state, e: &InputEvent, _, cx| {
                if matches!(e, InputEvent::Change) {
                    let v = state.read(cx).value().to_string();
                    store(cx).update(cx, |s, cx| s.update_player(cx, move |p| set(p, v)));
                }
            }));
        }
        for input in [&audio, &subtitle_langs] {
            subs.push(cx.subscribe_in(input, window, |this, _, e: &InputEvent, _, cx| {
                if matches!(e, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                    this.languages_changed(cx);
                }
            }));
        }
        for input in [&seerr_url, &seerr_key] {
            subs.push(cx.subscribe_in(input, window, |_, _, e: &InputEvent, _, cx| {
                if matches!(e, InputEvent::Change) {
                    cx.notify();
                }
            }));
        }
        subs.push(cx.subscribe_in(&pin, window, |this: &mut Self, state, e: &InputEvent, window, cx| {
            if matches!(e, InputEvent::PressEnter { .. })
                && let Some(u) = this.pin_for.take()
            {
                let code = state.read(cx).value().to_string();
                window.close_dialog(cx);
                Self::switch_to(&u, Some(code), cx);
            }
        }));
        subs.push(cx.subscribe(&poster, |_, _, e: &SliderEvent, cx| {
            let SliderEvent::Change(v) = e else { return };
            let size = v.start().round() as u32;
            store(cx).update(cx, |s, cx| {
                if s.settings.ui.poster_size != size {
                    s.update_ui(cx, move |u| u.poster_size = size);
                }
            });
        }));
        // Another account or server has its own rules, home users and Seerr session.
        subs.push(cx.observe_in(&store(cx), window, |this: &mut Self, store, window, cx| {
            let s = store.read(cx);
            let (scope, seerr_url) = (s.scope(), seerr::config(&core(cx)).map(|c| c.url));
            if scope != this.scope {
                this.scope = scope;
                this.load_prefs(window, cx);
                this.load_home(cx);
                this.load_seerr(window, cx);
            } else if seerr_url != this.seerr_seen {
                this.load_seerr(window, cx);
            }
        }));

        let mut this = Self {
            probe: None,
            probe_task: None,
            mpv_path,
            extra_args,
            skip_key,
            discord_id,
            downloads_dir,
            poster,
            profile: None,
            shows: Vec::new(),
            prefs_error: None,
            audio,
            subtitle_langs,
            seerr: None,
            seerr_error: None,
            seerr_url,
            seerr_key,
            connecting: false,
            home: Vec::new(),
            pin,
            pin_for: None,
            scope: store(cx).read(cx).scope(),
            seerr_seen: None,
            _subs: subs,
        };
        this.test(Duration::ZERO, cx);
        this.load_prefs(window, cx);
        this.load_home(cx);
        this.load_seerr(window, cx);
        this
    }

    fn test(&mut self, delay: Duration, cx: &mut Context<Self>) {
        self.probe = None;
        cx.notify();
        let app = core(cx);
        self.probe_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            let result = bg(async move { setup::probe_mpv(&app).await }).await;
            this.update(cx, |s, cx| {
                s.probe = Some(result.map_err(|e| message(&e)));
                cx.notify();
            })
            .ok();
        }));
    }

    // Automatic playback -----------------------------------------------------------

    fn load_prefs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let app = core(cx);
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let result = blocking(move || Ok::<_, anyhow::Error>((prefs::view(&app, None)?.profile, prefs::shows(&app)?))).await;
            handle
                .update(cx, |_, window, cx| {
                    this.update(cx, |s, cx| {
                        match result {
                            Ok((profile, shows)) => {
                                let join = |l: &Option<Vec<String>>| l.as_ref().map(|l| l.join(", ")).unwrap_or_default();
                                let (a, sl) = (join(&profile.audio_languages), join(&profile.subtitle_languages));
                                s.audio.update(cx, |i, cx| i.set_value(a, window, cx));
                                s.subtitle_langs.update(cx, |i, cx| i.set_value(sl, window, cx));
                                s.profile = Some(profile);
                                s.shows = shows;
                                s.prefs_error = None;
                            }
                            Err(e) => s.prefs_error = Some(message(&e)),
                        }
                        cx.notify();
                    })
                    .ok();
                })
                .ok();
        })
        .detach();
    }

    fn save_profile(&mut self, rules: Rules, window: &mut Window, cx: &mut Context<Self>) {
        let app = core(cx);
        let (weak, handle) = (cx.entity().downgrade(), window.window_handle());
        cx.spawn(async move |_, cx| {
            let result = blocking(move || prefs::save(&app, None, None, rules)).await;
            match result {
                Ok(()) => {
                    handle.update(cx, |_, window, cx| weak.update(cx, |s, cx| s.load_prefs(window, cx)).ok()).ok();
                }
                Err(e) => {
                    cx.update(|cx| store::error(cx, message(&e)));
                }
            }
        })
        .detach();
    }

    /// The language fields save when they lose focus or Enter is pressed.
    fn languages_changed(&mut self, cx: &mut Context<Self>) {
        let Some(profile) = self.profile.clone() else { return };
        let audio = parts(&self.audio.read(cx).value());
        let subs = parts(&self.subtitle_langs.read(cx).value());
        if audio == profile.audio_languages && subs == profile.subtitle_languages {
            return;
        }
        let rules = Rules { audio_languages: audio, subtitle_languages: subs, ..profile };
        let (app, weak) = (core(cx), cx.entity().downgrade());
        self.profile = Some(rules.clone());
        cx.spawn(async move |_, cx| {
            if let Err(e) = blocking(move || prefs::save(&app, None, None, rules)).await {
                cx.update(|cx| store::error(cx, message(&e)));
            }
            weak.update(cx, |_, cx| cx.notify()).ok();
        })
        .detach();
    }

    fn set_rule(&self, cx: &mut Context<Self>, f: impl FnOnce(&mut Rules) + 'static) -> impl Fn(&mut Window, &mut App) + 'static {
        let weak = cx.entity().downgrade();
        let f = std::cell::RefCell::new(Some(f));
        move |window, cx| {
            let Some(f) = f.borrow_mut().take() else { return };
            weak.update(cx, |s, cx| {
                let mut rules = s.profile.clone().unwrap_or_default();
                f(&mut rules);
                s.save_profile(rules, window, cx);
            })
            .ok();
        }
    }

    fn rules_form(&self, rules: &Rules, cx: &mut Context<Self>) -> Div {
        let w = px(208.);
        let unset = Some("Not set");
        macro_rules! pick {
            ($field:ident) => {{
                let weak = cx.entity().downgrade();
                move |v, window: &mut Window, cx: &mut App| {
                    weak.update(cx, |s, cx| {
                        let mut rules = s.profile.clone().unwrap_or_default();
                        rules.$field = v;
                        s.save_profile(rules, window, cx);
                    })
                    .ok();
                }
            }};
        }
        let mut codecs: Vec<(String, String)> = CODECS.iter().map(|(v, l)| (v.to_string(), l.to_string())).collect();
        if let Some(c) = rules.codec.as_ref().filter(|c| !CODECS.iter().any(|(id, _)| id == c)) {
            codecs.push((c.clone(), c.clone()));
        }
        let field = |el: AnyElement| div().w(w).flex_none().child(el);
        v_flex()
            .child(row(true, "Audio languages", Some("In order of preference, e.g. “eng, ja”. Codes or names.".into()), None, Some(field(Input::new(&self.audio).small().into_any_element()).into_any_element()), cx))
            .child(row(
                false,
                "Skip commentary",
                Some("Never pick commentary or audio-description tracks.".into()),
                None,
                Some(select("rule-commentary", w, rules.avoid_commentary, unset, vec![(true, "Skip commentary".into()), (false, "Allow commentary".into())], pick!(avoid_commentary)).into_any_element()),
                cx,
            ))
            .child(row(false, "Subtitles", Some("When subtitles are turned on.".into()), None, Some(select("rule-subtitles", w, rules.subtitles, unset, labels(&SUBTITLES), pick!(subtitles)).into_any_element()), cx))
            .child(row(false, "Subtitle languages", Some("The audio languages when empty.".into()), None, Some(field(Input::new(&self.subtitle_langs).small().into_any_element()).into_any_element()), cx))
            .child(row(false, "Forced subtitles", Some("Which kind of subtitle track to pick.".into()), None, Some(select("rule-forced", w, rules.forced, unset, labels(&FORCED), pick!(forced)).into_any_element()), cx))
            .child(row(
                false,
                "Version: resolution",
                Some("Which version to play when a title has several files.".into()),
                None,
                Some(select("rule-resolution", w, rules.resolution, unset, labels(&RESOLUTION), pick!(resolution)).into_any_element()),
                cx,
            ))
            .child(row(false, "Version: video codec", None, None, Some(select("rule-codec", w, rules.codec.clone(), unset, codecs, pick!(codec)).into_any_element()), cx))
            .child(row(false, "Version: HDR", None, None, Some(select("rule-hdr", w, rules.hdr, unset, labels(&HDR), pick!(hdr)).into_any_element()), cx))
    }

    fn clear_show(&mut self, show: &ShowRules, window: &mut Window, cx: &mut Context<Self>) {
        let (app, rk, title) = (core(cx), show.rating_key.clone(), show.title.clone());
        let (weak, handle) = (cx.entity().downgrade(), window.window_handle());
        store::act_then(cx, blocking(move || prefs::save(&app, Some(&rk), Some(&title), Rules::default())), move |_, cx| {
            store::toast(cx, "Cleared");
            handle.update(cx, |_, window, cx| weak.update(cx, |s, cx| s.load_prefs(window, cx)).ok()).ok();
        });
    }

    // Seerr --------------------------------------------------------------------

    /// The saved instance is checked on open; the form connects a new one.
    fn load_seerr(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let app = core(cx);
        let config = seerr::config(&app);
        self.seerr = None;
        self.seerr_error = None;
        self.seerr_seen = config.as_ref().map(|c| c.url.clone());
        let placeholder = if config.as_ref().is_some_and(|c| c.auth == SeerrAuthKind::ApiKey) { "Saved (enter to replace)" } else { "Optional" };
        self.seerr_key.update(cx, |i, cx| i.set_placeholder(placeholder, window, cx));
        cx.notify();
        let Some(config) = config else { return };
        let url = config.url.clone();
        self.seerr_url.update(cx, |i, cx| i.set_value(url, window, cx));
        cx.spawn(async move |this, cx| {
            let result = bg(async move { seerr::status(&app).await }).await;
            this.update(cx, |s, cx| {
                if s.seerr_seen.as_deref() != Some(config.url.as_str()) {
                    return;
                }
                match result {
                    Ok(st) => s.seerr = Some(st),
                    Err(e) => s.seerr_error = Some(e.to_string()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn connect_seerr(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let app = core(cx);
        let url = self.seerr_url.read(cx).value().to_string();
        let typed = self.seerr_key.read(cx).value().to_string();
        // An empty field keeps a saved key; disconnect first to switch to the Plex sign-in.
        let keep = typed.trim().is_empty() && seerr::config(&app).is_some_and(|c| c.auth == SeerrAuthKind::ApiKey);
        let key = (!keep).then_some(typed);
        self.connecting = true;
        self.seerr_error = None;
        cx.notify();
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let result = bg(async move { seerr::connect(&app, &url, key.as_deref()).await }).await;
            handle
                .update(cx, |_, window, cx| {
                    this.update(cx, |s, cx| {
                        s.connecting = false;
                        match result {
                            Ok(st) => {
                                s.seerr_key.update(cx, |i, cx| i.set_value("", window, cx));
                                store::toast(cx, format!("Connected to Seerr as {}", st.user.name));
                                reload_settings(cx);
                                s.load_seerr(window, cx);
                                s.seerr = Some(st);
                            }
                            Err(e) => s.seerr_error = Some(e.to_string()),
                        }
                        cx.notify();
                    })
                    .ok();
                })
                .ok();
        })
        .detach();
    }

    fn disconnect_seerr(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let app = core(cx);
        let (weak, handle) = (cx.entity().downgrade(), window.window_handle());
        store::act_then(cx, blocking(move || seerr::disconnect(&app).map_err(|e| anyhow::anyhow!(e.to_string()))), move |_, cx| {
            store::toast(cx, "Seerr disconnected");
            reload_settings(cx);
            handle
                .update(cx, |_, window, cx| {
                    weak.update(cx, |s, cx| {
                        s.seerr_url.update(cx, |i, cx| i.set_value("", window, cx));
                        s.load_seerr(window, cx);
                    })
                    .ok()
                })
                .ok();
        });
    }

    // Account --------------------------------------------------------------------

    fn load_home(&mut self, cx: &mut Context<Self>) {
        self.home.clear();
        if !store(cx).read(cx).has_tv() {
            return;
        }
        let app = core(cx);
        cx.spawn(async move |this, cx| {
            let result = bg(async move {
                match app.tv() {
                    Some(tv) => tv.home_users().await.map_err(anyhow::Error::from),
                    None => Ok(Vec::new()),
                }
            })
            .await;
            this.update(cx, |s, cx| {
                s.home = result.unwrap_or_default();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn switch_to(user: &HomeUser, pin: Option<String>, cx: &mut App) {
        let Some(uuid) = user.uuid.clone() else { return };
        nav::nav(cx).update(cx, |n, cx| n.reset(cx));
        let app = core(cx);
        store::act(cx, async move { session::switch_user(&app, &uuid, pin.as_deref()).await }, None, false);
    }

    fn ask_pin(&mut self, user: HomeUser, window: &mut Window, cx: &mut Context<Self>) {
        let pin = self.pin.clone();
        pin.update(cx, |i, cx| {
            i.set_value("", window, cx);
            i.focus(window, cx);
        });
        let name = user.title.clone().or_else(|| user.username.clone()).unwrap_or_default();
        self.pin_for = Some(user.clone());
        let user = Rc::new(user);
        window.open_dialog(cx, move |dialog, _, cx| {
            let (u1, u2, p1, p2) = (user.clone(), user.clone(), pin.clone(), pin.clone());
            dialog
                .title("Enter PIN")
                .w(px(360.))
                .on_ok(move |_, _, cx| {
                    let code = p1.read(cx).value().to_string();
                    Self::switch_to(&u1, Some(code), cx);
                    true
                })
                .child(
                    v_flex()
                        .gap_4()
                        .child(div().text_sm().text_color(cx.theme().muted_foreground).child(format!("{name}'s profile is protected.")))
                        .child(Input::new(&pin).large().text_center()),
                )
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(Button::new("pin-cancel").ghost().label("Cancel").on_click(|_, window, cx| window.close_dialog(cx)))
                        .child(Button::new("pin-switch").primary().label("Switch").on_click(move |_, window, cx| {
                            let code = p2.read(cx).value().to_string();
                            window.close_dialog(cx);
                            Self::switch_to(&u2, Some(code), cx);
                        })),
                )
        });
    }

    // Rendering ------------------------------------------------------------------

    fn player_card(&self, p: &PlayerSettings, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let info = &store(cx).read(cx).info;
        let (mpv_dir, conf, qualities) = (info.mpv_config_dir.clone(), info.mpv_conf_exists, info.qualities.iter().map(|q| (q.id.to_string(), q.label.to_string())).collect::<Vec<_>>());
        let probe = match &self.probe {
            None => row(true, "mpv executable", Some("Checking…".into()), None, None, cx),
            Some(Ok(t)) => row(true, "mpv executable", Some(t.clone().into()), None, None, cx),
            Some(Err(e)) => row(true, "mpv executable", Some(e.clone().into()), Some(theme.danger), None, cx),
        }
        .child(
            h_flex()
                .flex_none()
                .gap_2()
                .child(div().w(px(240.)).child(Input::new(&self.mpv_path).small()))
                .child(Button::new("mpv-test").outline().small().label("Test").on_click(cx.listener(|this, _, _, cx| this.test(Duration::from_millis(300), cx)))),
        );
        let switch = |key: &'static str, on: bool, set: SetFlag| {
            Switch::new(key)
                .checked(on)
                .on_click(move |on: &bool, _, cx| {
                    let on = *on;
                    store(cx).update(cx, |s, cx| s.update_player(cx, move |p| set(p, on)));
                })
                .into_any_element()
        };
        let toggles: [(&'static str, &str, &str, bool, SetFlag); 4] = [
            ("continuous", "Autoplay next episode", "Queue the following episodes when you play one.", p.continuous, |p, v| p.continuous = v),
            ("sync_streams", "Sync audio & subtitle tracks", "Start with the tracks selected in Plex, and remember tracks you switch to in mpv.", p.sync_streams, |p, v| p.sync_streams = v),
            ("auto_skip_intro", "Skip intros automatically", "Uses Plex intro markers. Otherwise press the skip key when prompted.", p.auto_skip_intro, |p, v| p.auto_skip_intro = v),
            ("auto_skip_credits", "Skip credits automatically", "Jumps past credits, or straight to the next episode.", p.auto_skip_credits, |p, v| p.auto_skip_credits = v),
        ];
        let thresholds = THRESHOLDS.iter().map(|t| ((t * 100.).round() as i64, format!("{}%", (t * 100.).round()))).collect();
        let threshold = (p.watched_threshold * 100.).round() as i64;
        card("Player", Some("Everything plays in mpv, with your normal mpv configuration, scripts and key bindings."), None, cx)
            .child(probe)
            .child(row(
                false,
                "mpv configuration",
                Some(format!("{mpv_dir}{}", if conf { " (mpv.conf found)" } else { "" }).into()),
                None,
                Some(
                    Button::new("mpv-config")
                        .outline()
                        .small()
                        .label("Open folder")
                        .on_click(|_, _, cx| {
                            let dir = config::mpv_config_dir();
                            match std::fs::create_dir_all(&dir) {
                                Ok(()) => cx.open_with_system(&dir),
                                Err(e) => store::error(cx, e.to_string()),
                            }
                        })
                        .into_any_element(),
                ),
                cx,
            ))
            .child(row(
                false,
                "Extra mpv arguments",
                Some("Appended to the command line, e.g. --fs --profile=hq. Your mpv.conf is always loaded.".into()),
                None,
                Some(div().w(px(288.)).child(Input::new(&self.extra_args).small()).into_any_element()),
                cx,
            ))
            .child(row(
                false,
                "Streaming quality",
                Some("Original plays the file directly. Lower qualities ask the server to transcode, which helps on slow connections.".into()),
                None,
                Some(
                    select("quality", px(224.), Some(p.quality.clone()), None, qualities, |v, _, cx| {
                        if let Some(q) = v {
                            store(cx).update(cx, |s, cx| s.update_player(cx, move |p| p.quality = q));
                        }
                    })
                    .into_any_element(),
                ),
                cx,
            ))
            .children(toggles.into_iter().map(|(key, title, desc, on, set)| row(false, title, Some(desc.into()), None, Some(switch(key, on, set)), cx)))
            .child(row(
                false,
                "Skip key",
                Some("mpv key name that skips an intro or credits. It is bound only while Patina controls mpv.".into()),
                None,
                Some(div().w(px(128.)).child(Input::new(&self.skip_key).small()).into_any_element()),
                cx,
            ))
            .child(row(
                false,
                "Mark as watched after",
                Some("How much of an item must play before it counts as watched.".into()),
                None,
                Some(
                    select("watched-threshold", px(112.), Some(threshold), None, thresholds, |v, _, cx| {
                        if let Some(t) = v {
                            store(cx).update(cx, |s, cx| s.update_player(cx, move |p| p.watched_threshold = t as f64 / 100.));
                        }
                    })
                    .into_any_element(),
                ),
                cx,
            ))
            .child(row(false, "System media controls", Some("Show what's playing in the OS media controls and respond to media keys.".into()), None, Some(switch("media_controls", p.media_controls, |p, v| p.media_controls = v)), cx))
            .child(row(false, "Discord Rich Presence", Some("Show what you're watching or listening to on your Discord profile.".into()), None, Some(switch("discord", p.discord, |p, v| p.discord = v)), cx))
            .when(p.discord, |c| {
                c.child(row(
                    false,
                    "Discord application ID",
                    Some("Create an application at discord.com/developers and paste its ID here. Its name is what Discord shows, e.g. “Patina”.".into()),
                    None,
                    Some(
                        h_flex()
                            .gap_2()
                            .child(div().w(px(208.)).child(Input::new(&self.discord_id).small()))
                            .child(Button::new("discord-portal").outline().small().icon(IconName::ExternalLink).label("Developer portal").on_click(|_, _, cx| cx.open_url("https://discord.com/developers/applications")))
                            .into_any_element(),
                    ),
                    cx,
                ))
            })
            .child(row(
                false,
                "Downloads folder",
                Some("Where movies, episodes and music you download for offline viewing are saved.".into()),
                None,
                Some(
                    h_flex()
                        .gap_2()
                        .child(div().w(px(240.)).child(Input::new(&self.downloads_dir).small()))
                        .child(Button::new("downloads-open").outline().small().label("Open").on_click(|_, _, cx| {
                            let dir = core(cx).settings().player.downloads_dir();
                            match std::fs::create_dir_all(&dir) {
                                Ok(()) => cx.open_with_system(&dir),
                                Err(e) => store::error(cx, e.to_string()),
                            }
                        }))
                        .into_any_element(),
                ),
                cx,
            ))
    }

    fn rules_card(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let set = self.profile.as_ref().is_some_and(|r| !r.is_empty());
        let clear = set.then(|| {
            let clear = self.set_rule(cx, |r| *r = Rules::default());
            Button::new("rules-clear").ghost().small().label("Clear all").on_click(move |_, window, cx| clear(window, cx)).into_any_element()
        });
        let c = card(
            "Automatic playback",
            Some("Audio, subtitle and version choices made for you when something starts playing, for the signed-in profile on this server. Shows can override these from their page. Applies to new episodes too."),
            clear,
            cx,
        );
        if let Some(e) = &self.prefs_error {
            return c.child(alert(e.clone(), cx));
        }
        let Some(profile) = self.profile.clone() else { return c };
        c.child(self.rules_form(&profile, cx)).when(!self.shows.is_empty(), |c| {
            c.child(div().px_4().py_2().border_t_1().border_color(theme.border).bg(theme.muted.opacity(0.3)).text_xs().font_medium().text_color(theme.muted_foreground).child("SHOWS WITH THEIR OWN PREFERENCES"))
                .children(self.shows.iter().enumerate().map(|(i, s)| {
                    let (rk, show) = (s.rating_key.clone(), s.clone());
                    row(
                        false,
                        s.title.clone(),
                        Some(summary(&s.rules).into()),
                        None,
                        Some(
                            h_flex()
                                .gap_2()
                                .child(Button::new(("show-open", i)).outline().small().label("Open").on_click(move |_, _, cx| nav::push(Route::Item { rating_key: rk.clone(), server: None }, cx)))
                                .child(Button::new(("show-clear", i)).danger().small().label("Clear").on_click(cx.listener(move |this, _, window, cx| this.clear_show(&show, window, cx))))
                                .into_any_element(),
                        ),
                        cx,
                    )
                }))
        })
    }

    fn appearance_card(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let s = store(cx).read(cx);
        let (accent, size, spoilers) = (s.settings.ui.accent.clone(), s.settings.ui.poster_size, s.settings.ui.hide_spoilers);
        let accents = s.info.accents.iter().map(|a| (a.id.to_string(), a.label.to_string(), a.color.unwrap_or("#e6e1d6").to_string())).collect::<Vec<_>>();
        let swatches = h_flex().gap_2().children(accents.into_iter().map(|(id, label, color)| {
            let on = id == accent;
            div()
                .id(SharedString::from(format!("accent-{id}")))
                .p(px(2.))
                .rounded_full()
                .border_2()
                .border_color(if on { theme.foreground } else { gpui::transparent_black() })
                .cursor_pointer()
                .tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(label.clone()).build(window, cx))
                .on_click(move |_, _, cx| {
                    let id = id.clone();
                    store(cx).update(cx, |s, cx| s.update_ui(cx, move |u| u.accent = id));
                })
                .child(div().size_5().rounded_full().bg(theme::hex(&color)))
        }));
        card("Appearance", None, None, cx)
            .child(row(true, "Accent", Some("Used sparingly: progress, watched marks and focus.".into()), None, Some(swatches.into_any_element()), cx))
            .child(row(false, "Poster size", Some(format!("{size}px").into()), None, Some(div().w(px(224.)).child(Slider::new(&self.poster)).into_any_element()), cx))
            .child(row(
                false,
                "Hide spoilers",
                Some("Blur thumbnails and summaries of episodes you haven't watched. Hover a summary to reveal it.".into()),
                None,
                Some(
                    Switch::new("hide-spoilers")
                        .checked(spoilers)
                        .on_click(|on: &bool, _, cx| {
                            let on = *on;
                            store(cx).update(cx, |s, cx| s.update_ui(cx, move |u| u.hide_spoilers = on));
                        })
                        .into_any_element(),
                ),
                cx,
            ))
    }

    fn seerr_card(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let config = seerr::config(&core(cx));
        let connected = config.is_some();
        let action = connected.then(|| {
            Button::new("seerr-disconnect").ghost().small().label("Disconnect").disabled(self.connecting).on_click(cx.listener(|this, _, window, cx| this.disconnect_seerr(window, cx))).into_any_element()
        });
        let status = config.map(|c| {
            let open = Button::new("seerr-open").outline().small().label("Open requests").on_click(|_, _, cx| nav::push(Route::Requests, cx)).into_any_element();
            match (&self.seerr, &self.seerr_error) {
                (Some(s), _) => {
                    let sub = format!(
                        "{}{} · {}{}",
                        s.url,
                        s.version.as_ref().map(|v| format!(" · Seerr {v}")).unwrap_or_default(),
                        if s.auth == SeerrAuthKind::Plex { "signed in with Plex" } else { "API key" },
                        if s.can.manage { " · can manage requests" } else { "" }
                    );
                    row(true, format!("Connected as {}", s.user.name), Some(sub.into()), None, Some(open), cx)
                }
                (None, Some(e)) => row(true, "Not connected", Some(e.clone().into()), Some(theme.danger), Some(open), cx),
                (None, None) => row(true, c.url, Some("Checking…".into()), None, Some(open), cx),
            }
        });
        let url_empty = self.seerr_url.read(cx).value().trim().is_empty();
        card(
            "Seerr",
            Some("Request movies and shows the server doesn't have through Seerr, Overseerr or Jellyseerr. Leave the API key empty to sign in with your Plex account."),
            action,
            cx,
        )
        .children(status)
        .child(row(!connected, "Server address", Some("For example http://192.168.1.10:5055".into()), None, Some(div().w(px(288.)).child(Input::new(&self.seerr_url).small()).into_any_element()), cx))
        .child(row(
            false,
            "API key",
            Some("In Seerr under Settings → General. Requests made with it count as the Seerr admin's.".into()),
            None,
            Some(
                h_flex()
                    .gap_2()
                    .child(div().w(px(288.)).child(Input::new(&self.seerr_key).small()))
                    .child(
                        Button::new("seerr-connect")
                            .primary()
                            .small()
                            .label(if self.connecting { "Connecting…" } else if connected { "Reconnect" } else { "Connect" })
                            .disabled(self.connecting || url_empty)
                            .on_click(cx.listener(|this, _, window, cx| this.connect_seerr(window, cx))),
                    )
                    .into_any_element(),
            ),
            cx,
        ))
        .when_some(self.seerr_error.clone().filter(|_| !connected), |c, e| c.child(alert(e, cx)))
    }

    fn account_card(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let s = store(cx).read(cx);
        let user = s.session.as_ref().and_then(|x| x.user.clone());
        let version = s.info.version;
        let avatar = |name: &str, thumb: Option<&str>, size: f32| {
            div()
                .relative()
                .size(px(size))
                .flex_none()
                .rounded_full()
                .overflow_hidden()
                .bg(theme.muted)
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(size * 0.36))
                .font_medium()
                .child(format::initials(name))
                .children(art_of(thumb, size * 2., size * 2., None).map(|i| i.absolute().inset_0().size_full()))
        };
        let first = match &user {
            Some(u) => h_flex()
                .px_4()
                .py_3p5()
                .gap_6()
                .items_center()
                .justify_between()
                .child(
                    h_flex()
                        .min_w_0()
                        .gap_3()
                        .child(avatar(&u.name, u.thumb.as_deref(), 40.))
                        .child(
                            v_flex()
                                .gap_1()
                                .child(div().text_sm().font_medium().child(u.name.clone()))
                                .child(div().text_sm().text_color(theme.muted_foreground).child(format!("{}{}", u.email.clone().unwrap_or_default(), if u.plex_pass { " · Plex Pass" } else { "" }))),
                        ),
                )
                .child(Button::new("sign-out").danger().small().label("Sign out").on_click(|_, _, cx| sign_out(cx))),
            None => row(
                true,
                "Direct server connection",
                Some("Not signed in to plex.tv.".into()),
                None,
                Some(Button::new("sign-in").outline().small().label("Sign in with Plex").on_click(|_, _, cx| sign_out(cx)).into_any_element()),
                cx,
            ),
        };
        let me = user.as_ref().and_then(|u| u.uuid.clone());
        let home = (self.home.len() > 1).then(|| {
            v_flex()
                .gap_3()
                .px_4()
                .py_3p5()
                .border_t_1()
                .border_color(theme.border)
                .child(v_flex().gap_1().child(div().text_sm().font_medium().child("Plex Home")).child(div().text_size(px(13.)).text_color(theme.muted_foreground).child("Switch to another member of your Plex Home.")))
                .child(h_flex().flex_wrap().gap_2().children(self.home.iter().enumerate().map(|(i, u)| {
                    let active = u.uuid.is_some() && u.uuid == me;
                    let name = u.title.clone().or_else(|| u.username.clone()).unwrap_or_else(|| "User".into());
                    let user = u.clone();
                    Button::new(("home-user", i))
                        .when(active, |b| b.primary())
                        .when(!active, |b| b.outline())
                        .small()
                        .rounded(px(999.))
                        .pl_1()
                        .child(h_flex().gap_1p5().items_center().child(avatar(&name, u.thumb.as_deref(), 20.)).child(format!("{name}{}", if u.protected { " · PIN" } else { "" })))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if active {
                                return;
                            }
                            if user.protected {
                                this.ask_pin(user.clone(), window, cx);
                            } else {
                                Self::switch_to(&user, None, cx);
                            }
                        }))
                })))
        });
        card("Account", None, None, cx).child(first).children(home).child(row(false, "Version", Some(format!("Patina {version}").into()), None, None, cx))
    }
}

/// Pick up settings changed outside the store (Seerr connect and disconnect).
fn reload_settings(cx: &mut App) {
    let settings = core(cx).settings();
    store(cx).update(cx, |s, cx| {
        s.settings = settings;
        s.refresh_seerr(cx);
        cx.notify();
    });
}

fn sign_out(cx: &mut App) {
    let app = core(cx);
    store::act(cx, blocking(move || {
        session::sign_out(&app);
        Ok::<_, anyhow::Error>(())
    }), None, false);
}

impl Render for Settings {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = store(cx).read(cx).settings.player.clone();
        let path = store(cx).read(cx).info.config_path.clone();
        widgets::page("settings").child(
            v_flex()
                .w_full()
                .max_w(px(960.))
                .child(widgets::page_head("Settings", Some(format!("Saved to {path}").into()), None, cx))
                .child(
                    v_flex()
                        .gap_6()
                        .child(self.player_card(&p, cx))
                        .child(self.rules_card(cx))
                        .child(self.appearance_card(cx))
                        .child(self.seerr_card(cx))
                        .child(self.account_card(cx)),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    #[test]
    fn language_lists() {
        assert_eq!(parts(" eng, ja ;French,, "), Some(vec!["eng".to_string(), "ja".into(), "French".into()]));
        assert_eq!(parts("  , "), None);
    }

    #[test]
    fn rule_summaries() {
        assert_eq!(summary(&Rules::default()), "No rules");
        let r = Rules {
            audio_languages: Some(vec!["eng".into()]),
            subtitles: Some(SubtitleMode::Always),
            subtitle_languages: Some(vec!["ja".into()]),
            forced: Some(ForcedMode::Prefer),
            avoid_commentary: Some(true),
            resolution: Some(Resolution::Fhd),
            codec: Some("hevc".into()),
            hdr: Some(HdrMode::Avoid),
        };
        assert_eq!(summary(&r), "eng audio · subtitles (ja) · prefer forced · skip commentary · 1080p · hevc · SDR");
        let r = Rules { subtitles: Some(SubtitleMode::Foreign), resolution: Some(Resolution::Uhd), ..Rules::default() };
        assert_eq!(summary(&r), "subtitles for foreign dialogue · 4K");
    }
}
