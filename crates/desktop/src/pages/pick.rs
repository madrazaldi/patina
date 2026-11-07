//! "Pick something to watch": a few random candidates that fit what you feel like.

use crate::prelude::*;
use std::time::Duration;

use gpui_kit::Global;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::kbd::Kbd;
use patina_core::ops::pick::{self, PickKind, PickOptions, PickQuery, PickResult};

use crate::core::{Load, bg, core, message};
use crate::pages::browse::ui;
use crate::store::store;
use crate::widgets;

const RUNTIMES: [u32; 6] = [30, 60, 90, 120, 150, 180];
const RATINGS: [u32; 4] = [5, 6, 7, 8];

/// The form as the user left it.
#[derive(Clone, Debug, PartialEq)]
struct Form {
    kind: PickKind,
    section: String,
    unwatched: bool,
    max_minutes: Option<u32>,
    min_rating: Option<u32>,
    year_min: String,
    year_max: String,
    genres: Vec<String>,
    content_ratings: Vec<String>,
}

impl Default for Form {
    fn default() -> Self {
        Self { kind: PickKind::Movie, section: String::new(), unwatched: true, max_minutes: None, min_rating: None, year_min: String::new(), year_max: String::new(), genres: vec![], content_ratings: vec![] }
    }
}

impl Form {
    fn query(&self) -> PickQuery {
        PickQuery {
            kind: self.kind,
            unwatched: self.unwatched,
            max_minutes: self.max_minutes,
            min_minutes: None,
            genres: self.genres.clone(),
            content_ratings: self.content_ratings.clone(),
            sections: if self.section.is_empty() { vec![] } else { vec![self.section.clone()] },
            year_min: year(&self.year_min),
            year_max: year(&self.year_max),
            min_rating: self.min_rating.map(f64::from),
            count: 4,
            exclude: vec![],
        }
    }
}

fn year(v: &str) -> Option<i64> {
    v.trim().parse::<i64>().ok().filter(|n| *n > 0)
}

fn toggle(list: &mut Vec<String>, value: &str) {
    if let Some(i) = list.iter().position(|v| v == value) {
        list.remove(i);
    } else {
        list.push(value.to_string());
    }
}

/// The form kept for the session, so coming back doesn't start over.
struct Remembered(String, Form);

impl Global for Remembered {}

pub struct Pick {
    scope: String,
    form: Form,
    options: Load<PickOptions>,
    result: Option<PickResult>,
    error: Option<String>,
    busy: bool,
    /// Rating keys already offered for these criteria, so a reroll shows something new.
    seen: Vec<String>,
    token: u64,
    year_min: Entity<InputState>,
    year_max: Entity<InputState>,
    focus: FocusHandle,
    _subs: Vec<Subscription>,
}

impl Pick {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let scope = store(cx).read(cx).scope();
        let form = cx.try_global::<Remembered>().filter(|r| r.0 == scope).map(|r| r.1.clone()).unwrap_or_default();
        let (a, b) = (form.year_min.clone(), form.year_max.clone());
        let year_min = cx.new(|cx| InputState::new(window, cx).placeholder("Any").default_value(a));
        let year_max = cx.new(|cx| InputState::new(window, cx).placeholder("Any").default_value(b));
        let subs = vec![
            cx.subscribe_in(&year_min, window, |this, state, e: &InputEvent, _, cx| {
                if let InputEvent::Change = e {
                    this.form.year_min = state.read(cx).value().to_string();
                    this.changed(cx);
                }
            }),
            cx.subscribe_in(&year_max, window, |this, state, e: &InputEvent, _, cx| {
                if let InputEvent::Change = e {
                    this.form.year_max = state.read(cx).value().to_string();
                    this.changed(cx);
                }
            }),
        ];
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let mut this = Self { scope, form, options: Load::Loading, result: None, error: None, busy: false, seen: vec![], token: 0, year_min, year_max, focus, _subs: subs };
        let app = core(cx);
        cx.spawn(async move |this, cx| {
            let result = bg(async move { pick::options(&app).await }).await;
            this.update(cx, |p, cx| {
                p.options = Load::from_result(result);
                cx.notify();
            })
            .ok();
        })
        .detach();
        this.changed(cx);
        this
    }

    /// New criteria start a fresh draw, shortly after the last change.
    fn changed(&mut self, cx: &mut Context<Self>) {
        cx.set_global(Remembered(self.scope.clone(), self.form.clone()));
        self.token += 1;
        let mine = self.token;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(250)).await;
            this.update(cx, |p, cx| {
                if p.token == mine {
                    p.roll(false, cx);
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn edit(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Form)) {
        f(&mut self.form);
        // A library of the other kind no longer applies once the kind changes.
        if let Some(opts) = self.options.ready()
            && !self.form.section.is_empty()
            && !opts.sections.iter().any(|s| s.id == self.form.section && fits(self.form.kind, &s.kind))
        {
            self.form.section.clear();
        }
        self.changed(cx);
    }

    fn roll(&mut self, again: bool, cx: &mut Context<Self>) {
        self.token += 1;
        let mine = self.token;
        let mut q = self.form.query();
        if again {
            q.exclude = self.seen.clone();
        }
        self.busy = true;
        self.error = None;
        cx.notify();
        let app = core(cx);
        let scope = self.scope.clone();
        cx.spawn(async move |this, cx| {
            let result = bg(async move { pick::pick(&app, &q).await }).await;
            this.update(cx, |p, cx| {
                if mine != p.token {
                    return;
                }
                p.busy = false;
                if store(cx).read(cx).scope() != scope {
                    return;
                }
                match result {
                    Ok(r) => {
                        let keys = r.items.iter().map(|m| m.rating_key().to_string());
                        if again && !r.wrapped {
                            p.seen.extend(keys);
                        } else {
                            p.seen = keys.collect();
                        }
                        p.result = Some(r);
                    }
                    Err(e) => p.error = Some(message(&e)),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.form = Form::default();
        self.year_min.update(cx, |s, cx| s.set_value("", window, cx));
        self.year_max.update(cx, |s, cx| s.set_value("", window, cx));
        self.changed(cx);
    }

    fn form_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let empty = PickOptions::default();
        let choices = self.options.ready().unwrap_or(&empty);
        let me = cx.entity().downgrade();
        let field = |label: &'static str, control: AnyElement| v_flex().gap_2().child(div().text_sm().font_medium().child(label)).child(control);
        let w = None;
        let m = me.clone();
        let kind = ui::select(
            "pick-kind",
            w,
            vec![(PickKind::Movie, "Movies".into()), (PickKind::Show, "Shows".into()), (PickKind::Any, "Movies and shows".into())],
            &self.form.kind,
            false,
            move |k, _, cx| {
                m.update(cx, |p, cx| p.edit(cx, |f| f.kind = k)).ok();
            },
        );
        let mut sections: Vec<(String, SharedString)> = vec![(String::new(), "Any library".into())];
        sections.extend(choices.sections.iter().filter(|s| fits(self.form.kind, &s.kind)).map(|s| (s.id.clone(), SharedString::from(s.title.clone()))));
        let m = me.clone();
        let section = ui::select("pick-section", w, sections, &self.form.section, false, move |id, _, cx| {
            m.update(cx, |p, cx| p.edit(cx, |f| f.section = id)).ok();
        });
        let mut runtimes: Vec<(Option<u32>, SharedString)> = vec![(None, "Any length".into())];
        runtimes.extend(RUNTIMES.iter().map(|m| (Some(*m), SharedString::from(format!("Up to {m} minutes")))));
        let m = me.clone();
        let runtime = ui::select("pick-runtime", w, runtimes, &self.form.max_minutes, false, move |v, _, cx| {
            m.update(cx, |p, cx| p.edit(cx, |f| f.max_minutes = v)).ok();
        });
        let mut ratings: Vec<(Option<u32>, SharedString)> = vec![(None, "Any rating".into())];
        ratings.extend(RATINGS.iter().map(|r| (Some(*r), SharedString::from(format!("{r}.0 or higher")))));
        let m = me.clone();
        let rating = ui::select("pick-rating", w, ratings, &self.form.min_rating, false, move |v, _, cx| {
            m.update(cx, |p, cx| p.edit(cx, |f| f.min_rating = v)).ok();
        });
        let m = me.clone();
        let unwatched = Checkbox::new("pick-unwatched").label("Only unwatched").checked(self.form.unwatched).on_click(move |on, _, cx| {
            let on = *on;
            m.update(cx, |p, cx| p.edit(cx, |f| f.unwatched = on)).ok();
        });
        let chips = |id: &'static str, label: &'static str, all: &[String], on: &[String], genres: bool| {
            let me = me.clone();
            (!all.is_empty()).then(|| {
                v_flex()
                    .min_w_0()
                    .gap_1()
                    .child(h_flex().mb_1p5().gap_1().text_sm().font_medium().child(label).child(div().font_normal().text_color(theme.muted_foreground).child("any of")))
                    .child(div().id(id).max_h(px(208.)).overflow_y_scroll().child(h_flex().flex_wrap().gap_1p5().children(all.iter().enumerate().map(|(i, v)| {
                        let selected = on.contains(v);
                        let value = v.clone();
                        let me = me.clone();
                        let b = Button::new((id, i)).small().rounded_full().label(v.clone()).on_click(move |_, _, cx| {
                            let value = value.clone();
                            me.update(cx, |p, cx| p.edit(cx, |f| toggle(if genres { &mut f.genres } else { &mut f.content_ratings }, &value))).ok();
                        });
                        if selected { b.primary() } else { b.outline() }
                    }))))
            })
        };
        let genres = chips("pick-genre", "Genres", &choices.genres, &self.form.genres, true);
        let ratings = chips("pick-content-rating", "Content rating", &choices.content_ratings, &self.form.content_ratings, false);
        v_flex()
            .flex_none()
            .w(px(280.))
            .p_4()
            .gap_4()
            .rounded(px(12.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.colors.group_box)
            .child(field("Looking for", kind))
            .child(field("Library", section))
            .child(unwatched)
            .child(field("Runtime", runtime))
            .child(field("Audience rating", rating))
            .child(
                h_flex()
                    .gap_2p5()
                    .child(div().flex_1().min_w_0().child(field("From year", Input::new(&self.year_min).small().into_any_element())))
                    .child(div().flex_1().min_w_0().child(field("To year", Input::new(&self.year_max).small().into_any_element()))),
            )
            .children(genres)
            .children(ratings)
            .child(h_flex().child(Button::new("pick-reset").ghost().small().label("Reset").on_click(cx.listener(|p, _, window, cx| p.reset(window, cx)))))
            .into_any_element()
    }

    fn results_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        if let Some(e) = &self.error {
            let me = cx.entity().downgrade();
            let retry: ui::Handler = std::rc::Rc::new(move |_, cx| {
                me.update(cx, |p, cx| p.roll(false, cx)).ok();
            });
            return ui::status_error("pick-retry", e.clone(), Some(retry), cx).into_any_element();
        }
        let Some(r) = &self.result else { return ui::status_loading().into_any_element() };
        if r.items.is_empty() {
            return ui::status_empty("Nothing fits", Some("Loosen a filter, such as the runtime, rating or years.".into()), cx).into_any_element();
        }
        let mut note = if r.wrapped {
            format!("That was everything that fits ({}); starting over.", r.matches)
        } else {
            let fit = if r.matches == 1 { "title fits" } else { "titles fit" };
            let left = if r.remaining > 0 { format!(", {} not offered yet", r.remaining) } else { String::new() };
            format!("{} {fit}{left}", r.matches)
        };
        if !r.libraries.is_empty() {
            note.push_str(&format!(" · {}", r.libraries.join(", ")));
        }
        v_flex()
            .min_w_0()
            .child(
                h_flex()
                    .mb_5()
                    .gap_3p5()
                    .items_center()
                    .text_sm()
                    .child(
                        Button::new("reroll")
                            .primary()
                            .disabled(self.busy)
                            .child(h_flex().gap_2().items_center().child(Icon::new(IconName::Dices).size_4()).child("Reroll").child(Kbd::new(Keystroke::parse("r").unwrap_or_default())))
                            .on_click(cx.listener(|p, _, _, cx| p.roll(true, cx))),
                    )
                    .child(div().text_color(theme.muted_foreground).child(note)),
            )
            .child(ui::item_grid("pick", r.items.clone(), false, None, cx))
            .into_any_element()
    }
}

/// Whether a library of `kind` fits what the form looks for.
fn fits(want: PickKind, kind: &str) -> bool {
    match want {
        PickKind::Any => true,
        PickKind::Movie => kind == "movie",
        PickKind::Show => kind == "show",
    }
}

impl Render for Pick {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let form = self.form_view(cx);
        let results = self.results_view(cx);
        widgets::page("pick")
            .track_focus(&self.focus)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|p, _, window, cx| {
                    if !p.focus.contains_focused(window, cx) {
                        p.focus.focus(window, cx);
                    }
                }),
            )
            .on_key_down(cx.listener(|p, e: &KeyDownEvent, window, cx| {
                let k = &e.keystroke;
                let typing = p.year_min.read(cx).focus_handle(cx).is_focused(window) || p.year_max.read(cx).focus_handle(cx).is_focused(window);
                if k.key == "r" && !k.modifiers.platform && !k.modifiers.control && !k.modifiers.alt && !typing && !window.has_active_dialog(cx) {
                    cx.stop_propagation();
                    p.roll(true, cx);
                }
            }))
            .child(widgets::page_head("Pick something to watch", Some("A few random candidates that fit what you feel like. Press R to reroll.".into()), None, cx))
            .child(h_flex().items_start().gap_6().child(form).child(div().flex_1().min_w_0().child(results)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    #[test]
    fn the_form_becomes_a_query() {
        let form = Form { section: "2".into(), year_min: " 1990".into(), year_max: "abc".into(), min_rating: Some(7), genres: vec!["Drama".into()], ..Form::default() };
        let q = form.query();
        assert_eq!(q.sections, vec!["2".to_string()]);
        assert_eq!((q.year_min, q.year_max), (Some(1990), None));
        assert_eq!(q.min_rating, Some(7.0));
        assert_eq!(q.count, 4);
        assert!(q.unwatched);
        assert_eq!(Form::default().query().sections, Vec::<String>::new());
    }

    #[test]
    fn chips_toggle_on_and_off() {
        let mut list = vec!["A".to_string()];
        toggle(&mut list, "B");
        toggle(&mut list, "A");
        assert_eq!(list, vec!["B".to_string()]);
    }
}
