//! Automatic audio, subtitle and version preferences for one show, and the
//! form for one set of rules (also fit for the profile's). Fields left unset
//! inherit the profile's rules; the matching is in `patina_core::prefs`.

use crate::prelude::*;
use std::rc::Rc;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use patina_core::prefs::{self, ForcedMode, HdrMode, Resolution, Rules, SubtitleMode};
use patina_plex::Metadata;

use super::form::{self, Guard, Modal};
use super::tracks::show_of;
use crate::core::{Load, blocking, core, message};
use crate::store;

const SUBTITLES: &[(SubtitleMode, &str, &str)] = &[
    (SubtitleMode::Keep, "keep", "Leave as Plex has them"),
    (SubtitleMode::Off, "off", "Off"),
    (SubtitleMode::Always, "always", "Always on"),
    (SubtitleMode::Foreign, "foreign", "Only for foreign dialogue"),
];
const FORCED: &[(ForcedMode, &str, &str)] = &[
    (ForcedMode::Auto, "auto", "Full subtitles (forced if nothing else)"),
    (ForcedMode::Prefer, "prefer", "Forced first, then full"),
    (ForcedMode::Avoid, "avoid", "Never forced"),
    (ForcedMode::Only, "only", "Only forced"),
];
const RESOLUTION: &[(Resolution, &str, &str)] = &[
    (Resolution::Any, "any", "Any"),
    (Resolution::Uhd, "4k", "4K"),
    (Resolution::Fhd, "1080", "1080p"),
    (Resolution::Hd, "720", "720p"),
    (Resolution::Sd, "sd", "SD"),
];
const CODECS: &[(&str, &str)] = &[("h264", "H.264"), ("hevc", "HEVC (H.265)"), ("av1", "AV1"), ("vp9", "VP9")];
const HDR: &[(HdrMode, &str, &str)] = &[(HdrMode::Any, "any", "Any"), (HdrMode::Prefer, "prefer", "Prefer HDR"), (HdrMode::Avoid, "avoid", "Avoid HDR (SDR)")];

/// The "no preference" choice of every select.
const NONE: &str = "";

/// "eng, ja; fr" → ["eng", "ja", "fr"]; none when empty.
fn parts(text: &str) -> Option<Vec<String>> {
    let out: Vec<String> = text.split([',', ';']).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect();
    (!out.is_empty()).then_some(out)
}

/// The options of an enum select, "unset" first.
fn options<T>(unset: &str, table: &[(T, &str, &str)]) -> Vec<(String, String)> {
    std::iter::once((NONE.to_string(), unset.to_string())).chain(table.iter().map(|(_, id, l)| (id.to_string(), l.to_string()))).collect()
}

fn id_of<T: PartialEq>(table: &[(T, &str, &str)], v: Option<&T>) -> String {
    v.and_then(|v| table.iter().find(|(t, _, _)| t == v)).map_or(NONE.to_string(), |(_, id, _)| id.to_string())
}

fn value_of<T: Copy>(table: &[(T, &str, &str)], id: &str) -> Option<T> {
    table.iter().find(|(_, i, _)| *i == id).map(|(t, _, _)| *t)
}

/// The codec choices, plus a saved codec that isn't one of them.
fn codecs(unset: &str, current: Option<&str>) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = std::iter::once((NONE.to_string(), unset.to_string())).chain(CODECS.iter().map(|(id, l)| (id.to_string(), l.to_string()))).collect();
    if let Some(c) = current.filter(|c| !c.is_empty() && !CODECS.iter().any(|(id, _)| id == c)) {
        out.push((c.to_string(), c.to_string()));
    }
    out
}

type OnSave = Rc<dyn Fn(Rules, &mut Window, &mut App)>;

/// The fields of one set of automatic playback preferences (`PlaybackRulesForm`).
pub struct RulesForm {
    rules: Rules,
    /// Label of the "no preference" choice.
    unset: SharedString,
    disabled: bool,
    on_save: OnSave,
    audio: Entity<InputState>,
    subtitles: Entity<InputState>,
    _subs: Vec<Subscription>,
}

impl RulesForm {
    pub fn new(rules: Rules, unset: impl Into<SharedString>, on_save: impl Fn(Rules, &mut Window, &mut App) + 'static, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let unset = unset.into();
        let join = |v: &Option<Vec<String>>| v.clone().unwrap_or_default().join(", ");
        let audio = cx.new(|cx| InputState::new(window, cx).placeholder(unset.clone()).default_value(join(&rules.audio_languages)));
        let subtitles = cx.new(|cx| InputState::new(window, cx).placeholder(unset.clone()).default_value(join(&rules.subtitle_languages)));
        // Saved when the field is left or Enter is pressed, like a change event.
        let commit = |audio: bool| {
            move |this: &mut Self, state: &Entity<InputState>, ev: &InputEvent, window: &mut Window, cx: &mut Context<Self>| {
                if matches!(ev, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                    let value = parts(&state.read(cx).value());
                    let current = if audio { &this.rules.audio_languages } else { &this.rules.subtitle_languages };
                    if *current != value {
                        this.set(window, cx, |r| if audio { r.audio_languages = value } else { r.subtitle_languages = value });
                    }
                }
            }
        };
        let subs = vec![cx.subscribe_in(&audio, window, commit(true)), cx.subscribe_in(&subtitles, window, commit(false))];
        Self { rules, unset, disabled: false, on_save: Rc::new(on_save), audio, subtitles, _subs: subs }
    }

    /// Show rules saved elsewhere (or reloaded after a save).
    pub fn set_rules(&mut self, rules: Rules, window: &mut Window, cx: &mut Context<Self>) {
        let join = |v: &Option<Vec<String>>| v.clone().unwrap_or_default().join(", ");
        self.audio.update(cx, |s, cx| s.set_value(join(&rules.audio_languages), window, cx));
        self.subtitles.update(cx, |s, cx| s.set_value(join(&rules.subtitle_languages), window, cx));
        self.rules = rules;
        cx.notify();
    }

    fn set(&mut self, window: &mut Window, cx: &mut Context<Self>, f: impl FnOnce(&mut Rules)) {
        f(&mut self.rules);
        cx.notify();
        (self.on_save)(self.rules.clone(), window, cx);
    }

    fn row(title: &str, text: Option<&str>, control: impl IntoElement, first: bool, cx: &App) -> Div {
        h_flex()
            .px_4()
            .py_3p5()
            .gap_6()
            .items_center()
            .justify_between()
            .when(!first, |d| d.border_t_1().border_color(cx.theme().border))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(div().text_sm().font_medium().child(title.to_string()))
                    .when_some(text, |d, t| d.child(div().text_size(px(13.)).text_color(cx.theme().muted_foreground).child(t.to_string()))),
            )
            .child(div().w(px(208.)).flex_none().child(control))
    }
}

impl Render for RulesForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let unset = self.unset.to_string();
        let r = self.rules.clone();
        let disabled = self.disabled;
        let me = cx.entity().downgrade();
        let pick = |f: fn(&mut Rules, &str)| {
            let me = me.clone();
            move |v: String, window: &mut Window, cx: &mut App| {
                me.update(cx, |this, cx| this.set(window, cx, |r| f(r, &v))).ok();
            }
        };
        let commentary = vec![(NONE.to_string(), unset.clone()), ("true".to_string(), "Skip commentary".to_string()), ("false".to_string(), "Allow commentary".to_string())];
        let commentary_value = r.avoid_commentary.map_or(NONE.to_string(), |b| b.to_string());
        let codec_value = r.codec.clone().unwrap_or_default();
        crate::widgets::rows(cx)
            .child(Self::row("Audio languages", Some("In order of preference, e.g. “eng, ja”. Codes or names."), Input::new(&self.audio).disabled(disabled), true, cx))
            .child(Self::row(
                "Skip commentary",
                Some("Never pick commentary or audio-description tracks."),
                form::select("rules-commentary", commentary, &commentary_value, disabled, pick(|r, v| r.avoid_commentary = v.parse().ok())),
                false,
                cx,
            ))
            .child(Self::row(
                "Subtitles",
                Some("When subtitles are turned on."),
                form::select("rules-subtitles", options(&unset, SUBTITLES), &id_of(SUBTITLES, r.subtitles.as_ref()), disabled, pick(|r, v| r.subtitles = value_of(SUBTITLES, v))),
                false,
                cx,
            ))
            .child(Self::row("Subtitle languages", Some("The audio languages when empty."), Input::new(&self.subtitles).disabled(disabled), false, cx))
            .child(Self::row(
                "Forced subtitles",
                Some("Which kind of subtitle track to pick."),
                form::select("rules-forced", options(&unset, FORCED), &id_of(FORCED, r.forced.as_ref()), disabled, pick(|r, v| r.forced = value_of(FORCED, v))),
                false,
                cx,
            ))
            .child(Self::row(
                "Version: resolution",
                Some("Which version to play when a title has several files."),
                form::select("rules-resolution", options(&unset, RESOLUTION), &id_of(RESOLUTION, r.resolution.as_ref()), disabled, pick(|r, v| r.resolution = value_of(RESOLUTION, v))),
                false,
                cx,
            ))
            .child(Self::row(
                "Version: video codec",
                None,
                form::select("rules-codec", codecs(&unset, r.codec.as_deref()), &codec_value, disabled, pick(|r, v| r.codec = (!v.is_empty()).then(|| v.to_string()))),
                false,
                cx,
            ))
            .child(Self::row(
                "Version: HDR",
                None,
                form::select("rules-hdr", options(&unset, HDR), &id_of(HDR, r.hdr.as_ref()), disabled, pick(|r, v| r.hdr = value_of(HDR, v))),
                false,
                cx,
            ))
    }
}

pub struct ShowPrefsDialog {
    show: Option<String>,
    title: String,
    guard: Guard,
    /// The show's own rules.
    own: Load<Rules>,
    form: Option<Entity<RulesForm>>,
}

pub fn open(item: Metadata, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| ShowPrefsDialog::new(&item, window, cx));
    form::show(view, "Playback preferences", form::WIDE, window, cx);
}

impl Modal for ShowPrefsDialog {}

impl ShowPrefsDialog {
    fn new(item: &Metadata, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (show, title) = show_of(item);
        let mut this = Self { show, title, guard: Guard::new(cx), own: Load::Loading, form: None };
        match this.show.clone() {
            Some(rk) => this.load(rk, window, cx),
            None => this.own = Load::Failed("This item doesn't belong to a show.".into()),
        }
        this
    }

    fn load(&mut self, rk: String, window: &mut Window, cx: &mut Context<Self>) {
        let app = core(cx);
        cx.spawn_in(window, async move |this, cx| {
            let result = blocking(move || prefs::view(&app, Some(&rk))).await;
            this.update_in(cx, |d, window, cx| {
                if !d.guard.valid(cx) {
                    return;
                }
                match result {
                    Ok(v) => {
                        let own = v.show.unwrap_or_default();
                        match &d.form {
                            Some(f) => f.update(cx, |f, cx| f.set_rules(own.clone(), window, cx)),
                            None => {
                                let me = cx.entity().downgrade();
                                let on_save = move |rules: Rules, window: &mut Window, cx: &mut App| {
                                    me.update(cx, |d, cx| d.save(rules, window, cx)).ok();
                                };
                                d.form = Some(cx.new(|cx| RulesForm::new(own.clone(), "Same as profile", on_save, window, cx)));
                            }
                        }
                        d.own = Load::Ready(own);
                    }
                    Err(e) => d.own = Load::Failed(message(&e)),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn save(&mut self, rules: Rules, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rk) = self.show.clone() else { return };
        if !self.guard.valid(cx) {
            window.close_dialog(cx);
            return;
        }
        let (app, title) = (core(cx), self.title.clone());
        let show = rk.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = blocking(move || prefs::save(&app, Some(&show), Some(&title), rules)).await;
            if let Err(e) = &result {
                cx.update(|_, cx| store::error(cx, message(e))).ok();
            }
            this.update_in(cx, |d, window, cx| d.load(rk, window, cx)).ok();
        })
        .detach();
    }
}

impl Render for ShowPrefsDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let desc = format!(
            "{}: tracks and version picked automatically when this show plays, including new episodes. Anything left unset follows your profile's preferences in Settings.",
            self.title
        );
        let empty = self.own.ready().is_none_or(Rules::is_empty);
        let body = match (&self.own, &self.form) {
            (Load::Failed(e), _) => form::error(e.clone(), cx).into_any_element(),
            (Load::Ready(_), Some(f)) => f.clone().into_any_element(),
            _ => form::spinner().py_10().into_any_element(),
        };
        v_flex().child(form::desc(desc, cx)).child(body).child(
            form::foot(cx)
                .child(Button::new("clear").outline().label("Clear for this show").disabled(empty).on_click(cx.listener(|this, _, window, cx| this.save(Rules::default(), window, cx))))
                .child(Button::new("done").primary().label("Done").on_click(|_, window, cx| window.close_dialog(cx))),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    #[test]
    fn language_lists_split_on_commas_and_semicolons() {
        assert_eq!(parts("eng, ja; fr"), Some(vec!["eng".into(), "ja".into(), "fr".into()]));
        assert_eq!(parts(" , "), None);
    }

    #[test]
    fn selects_round_trip() {
        assert_eq!(id_of(RESOLUTION, Some(&Resolution::Fhd)), "1080");
        assert_eq!(id_of(RESOLUTION, None), NONE);
        assert_eq!(value_of(FORCED, "only"), Some(ForcedMode::Only));
        assert_eq!(value_of(FORCED, NONE), None);
        assert_eq!(options("Same as profile", HDR)[0], (NONE.to_string(), "Same as profile".to_string()));
        assert_eq!(codecs("Not set", Some("mpeg2")).last().map(|c| c.0.as_str()), Some("mpeg2"));
        assert_eq!(codecs("Not set", Some("hevc")).len(), 5);
    }
}
