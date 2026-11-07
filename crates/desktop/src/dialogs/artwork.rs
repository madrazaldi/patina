//! Poster / background picker.

use crate::prelude::*;
use gpui_kit::component::spinner::Spinner;
use patina_plex::Metadata;
use serde_json::Value;

use super::form::{self, Guard, Modal};
use crate::art::art_of;
use crate::core::{Load, bg, core};
use crate::store::store;

/// Which image is being chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Poster,
    Background,
}

/// One choice: where it is (`key`, or else `ratingKey`), its thumbnail, provider and whether it is current.
#[derive(Clone, Debug, PartialEq)]
struct Choice {
    url: Option<String>,
    thumb: Option<String>,
    provider: Option<String>,
    selected: bool,
}

/// Plex sends `selected` as a bool or as 0/1.
fn truthy(v: Option<&Value>) -> bool {
    match v {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0) != 0,
        Some(Value::String(s)) => s == "1" || s == "true",
        _ => false,
    }
}

fn choice(m: &Metadata) -> Choice {
    Choice {
        url: m.key.clone().or_else(|| m.rating_key.clone()),
        thumb: m.thumb.clone().or_else(|| m.key.clone()),
        provider: m.extra.get("provider").and_then(Value::as_str).map(str::to_string),
        selected: truthy(m.extra.get("selected")),
    }
}

/// Columns and cell width for cells of at least `min` across `width`.
fn columns(width: f32, min: f32, gap: f32) -> (usize, f32) {
    let n = (((width + gap) / (min + gap)).floor() as usize).max(1);
    (n, (width - gap * (n as f32 - 1.)) / n as f32)
}

pub struct ArtworkDialog {
    item: Metadata,
    kind: Kind,
    guard: Guard,
    choices: Load<Vec<Choice>>,
    /// The choice being applied.
    busy: Option<String>,
}

pub fn open(item: Metadata, kind: Kind, window: &mut Window, cx: &mut App) {
    let title = if kind == Kind::Background { "Choose background" } else { "Choose poster" };
    let view = cx.new(|cx| ArtworkDialog::new(item, kind, cx));
    form::show(view, title, form::WIDE, window, cx);
}

impl Modal for ArtworkDialog {
    fn busy(&self) -> bool {
        self.busy.is_some()
    }
}

impl ArtworkDialog {
    fn new(item: Metadata, kind: Kind, cx: &mut Context<Self>) -> Self {
        let app = core(cx);
        let rk = item.rating_key().to_string();
        cx.spawn(async move |this, cx| {
            let result = bg(async move {
                let server = app.server()?;
                let list = if kind == Kind::Background { server.arts(&rk).await? } else { server.posters(&rk).await? };
                Ok::<_, anyhow::Error>(list.iter().map(choice).collect::<Vec<_>>())
            })
            .await;
            this.update(cx, |d, cx| {
                if d.guard.valid(cx) {
                    d.choices = Load::from_result(result);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        Self { item, kind, guard: Guard::new(cx), choices: Load::Loading, busy: None }
    }

    fn pick(&mut self, url: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy.is_some() {
            return;
        }
        if !self.guard.valid(cx) {
            window.close_dialog(cx);
            return;
        }
        let (app, rk, kind) = (core(cx), self.item.rating_key().to_string(), self.kind);
        self.busy = Some(url.clone());
        cx.notify();
        let fut = async move {
            let server = app.server()?;
            if kind == Kind::Background { server.set_art(&rk, &url).await? } else { server.set_poster(&rk, &url).await? }
            Ok::<_, anyhow::Error>(())
        };
        let ok = if kind == Kind::Background { "Background updated" } else { "Poster updated" };
        form::run(window, cx, fut, Some(ok.into()), true, |d, ok, window, cx| {
            d.busy = None;
            if ok {
                window.close_dialog(cx);
            }
            cx.notify();
        });
    }
}

impl Render for ArtworkDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let tint = store(cx).read(cx).tint();
        let art = self.kind == Kind::Background;
        let square = !art && matches!(self.item.kind(), "artist" | "album");
        let busy = self.busy.clone();
        let body = match &self.choices {
            Load::Failed(e) => form::error(e.clone(), cx).into_any_element(),
            Load::Loading => form::spinner().into_any_element(),
            Load::Ready(l) if l.is_empty() => {
                form::empty("Nothing to choose", format!("No {} available.", if art { "backgrounds" } else { "posters" }), cx).into_any_element()
            }
            Load::Ready(l) => {
                // The dialog's content width.
                let (_, w) = columns(form::WIDE - 32., if art { 180. } else { 104. }, 12.);
                let ratio = if art { 16. / 9. } else if square { 1. } else { 2. / 3. };
                let h = w / ratio;
                h_flex()
                    .id("choices")
                    .flex_wrap()
                    .items_start()
                    .gap_x_3()
                    .gap_y_4()
                    .max_h(px(440.))
                    .overflow_y_scroll()
                    .p_0p5()
                    .children(l.iter().enumerate().map(|(i, c)| {
                        let url = c.url.clone();
                        let waiting = busy.is_some() && busy == url;
                        let provider = if c.selected { "Current".to_string() } else { c.provider.clone().unwrap_or_default() };
                        div()
                            .id(("choice", i))
                            .w(px(w))
                            .flex_none()
                            .when(busy.is_none(), |d| d.cursor_pointer())
                            .child(
                                div()
                                    .relative()
                                    .w(px(w))
                                    .h(px(h))
                                    .rounded(px(10.))
                                    .overflow_hidden()
                                    .bg(theme.muted)
                                    .border_2()
                                    .border_color(if c.selected { tint } else { theme.border })
                                    .children(art_of(c.thumb.as_deref(), w, h, None).map(|i| i.size_full().object_fit(ObjectFit::Cover)))
                                    .when(c.selected, |d| {
                                        d.child(
                                            div()
                                                .absolute()
                                                .top(px(6.))
                                                .right(px(6.))
                                                .size_5()
                                                .rounded_full()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .bg(theme.popover)
                                                .text_color(tint)
                                                .child(Icon::new(IconName::Check).size_3()),
                                        )
                                    })
                                    .when(waiting, |d| {
                                        d.child(div().absolute().inset_0().flex().items_center().justify_center().bg(hsla(0., 0., 0.05, 0.5)).child(Spinner::new()))
                                    }),
                            )
                            .child(div().mt_1p5().text_xs().truncate().text_color(if c.selected { tint } else { theme.muted_foreground }).child(provider))
                            .when_some(url.filter(|_| busy.is_none()), |d, url| d.on_click(cx.listener(move |this, _, window, cx| this.pick(url.clone(), window, cx))))
                    }))
                    .into_any_element()
            }
        };
        v_flex()
            .child(form::desc(self.item.full_title(), cx))
            .child(body)
            .child(form::foot(cx).child(Button::new("cancel").outline().label("Cancel").disabled(self.busy.is_some()).on_click(|_, window, cx| window.close_dialog(cx))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    #[test]
    fn choices_read_plex_extras() {
        let m: Metadata = serde_json::from_str(r#"{"key":"/a/1","thumb":"/t/1","provider":"tmdb","selected":1}"#).unwrap();
        let c = choice(&m);
        assert_eq!(c.url.as_deref(), Some("/a/1"));
        assert_eq!(c.provider.as_deref(), Some("tmdb"));
        assert!(c.selected);
        let m: Metadata = serde_json::from_str(r#"{"ratingKey":"upload://x","selected":false}"#).unwrap();
        let c = choice(&m);
        assert_eq!(c.url.as_deref(), Some("upload://x"));
        assert!(!c.selected);
    }

    #[test]
    fn columns_fill_the_width() {
        let (n, w) = columns(608., 104., 12.);
        assert_eq!(n, 5);
        assert!((w - 112.).abs() < 0.01);
        assert_eq!(columns(608., 180., 12.).0, 3);
        assert_eq!(columns(50., 104., 12.).0, 1);
    }
}
