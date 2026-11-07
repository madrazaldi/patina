//! The library's filter picker: filter names, then the chosen filter's values.

use crate::prelude::*;
use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use patina_core::ops::library::FilterView;
use patina_plex::Directory;

use crate::core::{Load, bg, core};

/// What was picked: the filter, the value and its label.
pub type OnPick = Rc<dyn Fn(&FilterView, String, String, &mut Window, &mut App)>;

pub struct FilterPicker {
    section: String,
    kind: Option<u32>,
    filters: Vec<FilterView>,
    /// Filters with a value picked already, and the picked `(filter, value)` pairs.
    picked: Vec<(String, String)>,
    choosing: Option<FilterView>,
    values: Load<Vec<Directory>>,
    query: Entity<InputState>,
    on_pick: OnPick,
    _subs: Vec<Subscription>,
}

impl FilterPicker {
    pub fn new(
        section: String,
        kind: Option<u32>,
        filters: Vec<FilterView>,
        picked: Vec<(String, String)>,
        on_pick: OnPick,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx));
        let subs = vec![cx.subscribe(&query, |_, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                cx.notify();
            }
        })];
        Self { section, kind, filters, picked, choosing: None, values: Load::Loading, query, on_pick, _subs: subs }
    }

    fn choose(&mut self, f: FilterView, window: &mut Window, cx: &mut Context<Self>) {
        self.query.update(cx, |q, cx| {
            q.set_value("", window, cx);
            q.set_placeholder(format!("Search {}", f.title.to_lowercase()), window, cx);
        });
        self.choosing = Some(f.clone());
        self.values = Load::Loading;
        let (app, id, kind) = (core(cx), self.section.clone(), self.kind);
        cx.spawn(async move |this, cx| {
            let result = bg(async move { Ok::<_, anyhow::Error>(app.server()?.section_filter_values(&id, &f.filter, kind).await?) }).await;
            this.update(cx, |p, cx| {
                p.values = Load::from_result(result);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
}

fn row(id: impl Into<ElementId>, cx: &App) -> Stateful<Div> {
    h_flex()
        .id(id)
        .w_full()
        .gap_2()
        .px_2()
        .py_1p5()
        .rounded(px(6.))
        .text_sm()
        .cursor_pointer()
        .hover(|s| s.bg(cx.theme().accent))
}

impl Render for FilterPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let body = v_flex().w(px(260.)).max_h(px(420.)).gap_0p5();
        let Some(f) = self.choosing.clone() else {
            return body
                .children(self.filters.clone().into_iter().enumerate().map(|(i, f)| {
                    let on = self.picked.iter().any(|(k, _)| *k == f.filter);
                    let title = f.title.clone();
                    row(("filter", i), cx)
                        .on_click(cx.listener(move |this, _, window, cx| this.choose(f.clone(), window, cx)))
                        .child(div().flex_1().child(title))
                        .when(on, |d| d.child(div().size_1p5().rounded_full().bg(crate::store::store(cx).read(cx).tint())))
                        .child(Icon::new(IconName::ChevronRight).size_3p5().text_color(theme.muted_foreground))
                }))
                .into_any_element();
        };
        let q = self.query.read(cx).value().trim().to_lowercase();
        let all = self.values.ready().cloned().unwrap_or_default();
        let shown: Vec<Directory> = all.iter().filter(|v| q.is_empty() || v.title.as_deref().unwrap_or("").to_lowercase().contains(&q)).cloned().collect();
        let note = |text: SharedString| h_flex().justify_center().px_2().py_3().text_xs().text_color(theme.muted_foreground).child(text);
        let _ = window;
        body.child(
            row("back", cx)
                .gap_1()
                .text_color(theme.muted_foreground)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.choosing = None;
                    cx.notify();
                }))
                .child(Icon::new(IconName::ChevronLeft).size_3p5())
                .child(f.title.clone()),
        )
        .child(div().h_px().my_1().bg(theme.border))
        .when(all.len() > 12, |d| d.child(div().px_1().pb_1p5().child(Input::new(&self.query).small())))
        .child(
            v_flex().id("values").max_h(px(320.)).overflow_y_scroll().child(match &self.values {
                Load::Loading => h_flex().justify_center().py_3().child(Spinner::new().small()).into_any_element(),
                Load::Failed(e) => note(e.clone().into()).into_any_element(),
                Load::Ready(_) if shown.is_empty() => note("Nothing to choose".into()).into_any_element(),
                Load::Ready(_) => v_flex()
                    .children(shown.into_iter().enumerate().map(|(i, v)| {
                        let key = v.key.clone().unwrap_or_default();
                        let label = v.title.clone().unwrap_or_else(|| key.clone());
                        let on = self.picked.iter().any(|(k, val)| *k == f.filter && *val == key);
                        let f = f.clone();
                        let pick = self.on_pick.clone();
                        let shown_label = label.clone();
                        row(("value", i), cx)
                            .on_click(move |_, window, cx| pick(&f, key.clone(), label.clone(), window, cx))
                            .child(div().flex_1().truncate().child(shown_label))
                            .when(on, |d| d.child(Icon::new(IconName::Check).size_3p5()))
                    }))
                    .into_any_element(),
            }),
        )
        .into_any_element()
    }
}
