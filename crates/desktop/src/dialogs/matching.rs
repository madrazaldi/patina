//! "Fix match": search the agent for the right title, apply it, or unmatch.

use crate::prelude::*;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use patina_plex::{MatchResult, Metadata};

use super::form::{self, Guard, Modal};
use super::playlist::thumb;
use crate::core::{Load, bg, core};
use crate::store::store;
use crate::widgets;

pub struct MatchDialog {
    item: Metadata,
    guard: Guard,
    title: Entity<InputState>,
    year: Entity<InputState>,
    results: Load<Vec<MatchResult>>,
    /// Drops the results of a search a newer one replaced.
    generation: u64,
    busy: bool,
    _subs: Vec<Subscription>,
}

pub fn open(item: Metadata, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| MatchDialog::new(item, window, cx));
    form::show(view, "Fix match", form::WIDE, window, cx);
}

impl Modal for MatchDialog {
    fn busy(&self) -> bool {
        self.busy
    }
}

impl MatchDialog {
    fn new(item: Metadata, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let title = cx.new(|cx| InputState::new(window, cx).placeholder("Title").default_value(item.title().to_string()));
        let year = cx.new(|cx| InputState::new(window, cx).placeholder("Year").default_value(item.year.map(|y| y.to_string()).unwrap_or_default()));
        let on_input = |this: &mut Self, _: &Entity<InputState>, ev: &InputEvent, _: &mut Window, cx: &mut Context<Self>| match ev {
            InputEvent::PressEnter { .. } => this.search(true, cx),
            InputEvent::Change => cx.notify(),
            _ => {}
        };
        let subs = vec![cx.subscribe_in(&title, window, on_input), cx.subscribe_in(&year, window, on_input)];
        let mut this = Self { item, guard: Guard::new(cx), title, year, results: Load::Loading, generation: 0, busy: false, _subs: subs };
        // Automatic matches on open.
        this.search(false, cx);
        this
    }

    fn search(&mut self, manual: bool, cx: &mut Context<Self>) {
        let title = self.title.read(cx).value().trim().to_string();
        if manual && title.is_empty() {
            return;
        }
        self.generation += 1;
        let run = self.generation;
        self.results = Load::Loading;
        cx.notify();
        let year = self.year.read(cx).value().trim().parse::<i64>().ok().filter(|y| *y > 0);
        let (title, year) = if manual { (Some(title), year) } else { (None, None) };
        let (app, rk, asked) = (core(cx), self.item.rating_key().to_string(), self.guard.clone());
        cx.spawn(async move |this, cx| {
            let result = bg(async move { Ok::<_, anyhow::Error>(app.server()?.matches(&rk, title.as_deref(), year, None).await?) }).await;
            this.update(cx, |d, cx| {
                if d.generation == run && asked.valid(cx) {
                    d.results = Load::from_result(result);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn apply(&mut self, r: &MatchResult, window: &mut Window, cx: &mut Context<Self>) {
        let Some(guid) = r.guid.clone() else { return };
        if self.busy {
            return;
        }
        if !self.guard.valid(cx) {
            window.close_dialog(cx);
            return;
        }
        let name = r.name.clone();
        let ok = format!("Matched to “{}”", name.clone().unwrap_or_default());
        let (app, rk) = (core(cx), self.item.rating_key().to_string());
        self.busy = true;
        cx.notify();
        let fut = async move { Ok::<_, anyhow::Error>(app.server()?.apply_match(&rk, &guid, name.as_deref()).await?) };
        form::run(window, cx, fut, Some(ok), true, |d, ok, window, cx| {
            d.busy = false;
            if ok {
                window.close_dialog(cx);
            }
            cx.notify();
        });
    }

    fn unmatch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let me = cx.entity().downgrade();
        widgets::confirm(
            "Unmatch this item?",
            Some("Its metadata and artwork are cleared until it's matched again.".into()),
            "Unmatch",
            true,
            window,
            cx,
            move |window, cx| {
                me.update(cx, |d, cx| {
                    if !d.guard.valid(cx) {
                        return;
                    }
                    let (app, rk) = (core(cx), d.item.rating_key().to_string());
                    let fut = async move { Ok::<_, anyhow::Error>(app.server()?.unmatch(&rk).await?) };
                    form::run(window, cx, fut, Some("Unmatched".into()), true, |_, ok, window, cx| {
                        if ok {
                            window.close_dialog(cx);
                        }
                    });
                })
                .ok();
            },
        );
    }
}

impl Render for MatchDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let tint = store(cx).read(cx).tint();
        let square = matches!(self.item.kind(), "artist" | "album");
        let busy = self.busy;
        let no_title = self.title.read(cx).value().trim().is_empty();
        let body = match &self.results {
            Load::Failed(e) => form::error(e.clone(), cx).into_any_element(),
            Load::Loading => form::spinner().into_any_element(),
            Load::Ready(l) if l.is_empty() => form::empty("No matches", "No matches. Try a different title or year.", cx).into_any_element(),
            Load::Ready(l) => v_flex()
                .id("matches")
                .max_h(px(420.))
                .overflow_y_scroll()
                .gap_1()
                .children(l.iter().enumerate().map(|(i, r)| {
                    let pick = r.clone();
                    form::item_row(("match", i), busy, cx)
                        .items_start()
                        .child(thumb(r.thumb.as_deref(), 32., if square { 32. } else { 48. }, cx))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_0p5()
                                .child(
                                    h_flex()
                                        .flex_wrap()
                                        .gap_2()
                                        .items_center()
                                        .text_sm()
                                        .font_medium()
                                        .child(r.name.clone().unwrap_or_default())
                                        .when_some(r.year, |d, y| d.child(div().font_normal().text_color(theme.muted_foreground).child(format!("({y})"))))
                                        .when(r.matched, |d| d.child(form::badge("Current", Some(tint), cx))),
                                )
                                .when_some(r.summary.clone().filter(|s| !s.is_empty()), |d, s| {
                                    d.child(div().text_xs().line_clamp(2).text_color(theme.muted_foreground).child(s))
                                }),
                        )
                        .when_some(r.score, |d, s| d.child(div().text_xs().text_color(theme.muted_foreground).child(format!("{s}%"))))
                        .when(!busy, |d| d.on_click(cx.listener(move |this, _, window, cx| this.apply(&pick, window, cx))))
                }))
                .into_any_element(),
        };
        v_flex()
            .child(form::desc(self.item.full_title(), cx))
            .child(
                h_flex()
                    .mb_4()
                    .gap_2()
                    .child(div().flex_1().child(Input::new(&self.title)))
                    .child(div().w(px(76.)).child(Input::new(&self.year)))
                    .child(Button::new("search").outline().label("Search").disabled(no_title).on_click(cx.listener(|this, _, _, cx| this.search(true, cx)))),
            )
            .child(body)
            .child(
                form::foot(cx)
                    .child(div().mr_auto().child(Button::new("unmatch").danger().label("Unmatch").disabled(busy).on_click(cx.listener(|this, _, window, cx| this.unmatch(window, cx)))))
                    .child(Button::new("cancel").outline().label("Cancel").disabled(busy).on_click(|_, window, cx| window.close_dialog(cx))),
            )
    }
}
