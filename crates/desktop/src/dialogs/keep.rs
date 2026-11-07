//! "Keep the next N unwatched episodes downloaded" for a show, season or
//! playlist. The policy is evaluated in `patina_core::smart`.

use crate::prelude::*;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::switch::Switch;
use patina_core::smart::{self, PolicyView};
use patina_plex::Metadata;

use super::form::{self, Guard, Modal};
use crate::core::{blocking, core};
use crate::format;

/// A valid "keep next" count.
fn keep_count(text: &str) -> Option<u32> {
    text.trim().parse().ok().filter(|n| (1..=smart::MAX_KEEP).contains(n))
}

pub struct KeepDialog {
    item: Metadata,
    guard: Guard,
    existing: Option<PolicyView>,
    keep: Entity<InputState>,
    delete_watched: bool,
    also_delete: bool,
    busy: bool,
    _subs: Vec<Subscription>,
}

/// Open the dialog once the current policy (if any) is known.
pub fn open(item: Metadata, window: &mut Window, cx: &mut App) {
    let app = core(cx);
    let guard = Guard::new(cx);
    window
        .spawn(cx, async move |cx| {
            let overview = blocking(move || smart::overview(&app)).await;
            cx.update(|window, cx| {
                if !guard.valid(cx) {
                    return;
                }
                let existing = overview.policies.into_iter().find(|p| p.active && p.policy.rating_key == item.rating_key());
                let view = cx.new(|cx| KeepDialog::new(item, existing, window, cx));
                form::show(view, "Keep downloaded", form::NARROW, window, cx);
            })
            .ok();
        })
        .detach();
}

impl Modal for KeepDialog {
    fn busy(&self) -> bool {
        self.busy
    }
}

impl KeepDialog {
    fn new(item: Metadata, existing: Option<PolicyView>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let count = existing.as_ref().map_or(3, |p| p.policy.keep);
        let keep = cx.new(|cx| InputState::new(window, cx).default_value(count.to_string()));
        keep.update(cx, |s, cx| s.focus(window, cx));
        let subs = vec![cx.subscribe_in(&keep, window, |this, _, ev: &InputEvent, window, cx| match ev {
            InputEvent::PressEnter { .. } => this.save(window, cx),
            InputEvent::Change => cx.notify(),
            _ => {}
        })];
        let delete_watched = existing.as_ref().is_none_or(|p| p.policy.delete_watched);
        Self { item, guard: Guard::new(cx), existing, keep, delete_watched, also_delete: false, busy: false, _subs: subs }
    }

    fn finish(&mut self, ok: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.busy = false;
        if ok {
            window.close_dialog(cx);
        }
        cx.notify();
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(keep) = keep_count(&self.keep.read(cx).value()) else { return };
        if self.busy {
            return;
        }
        if !self.guard.valid(cx) {
            window.close_dialog(cx);
            return;
        }
        let (app, rk, delete_watched) = (core(cx), self.item.rating_key().to_string(), self.delete_watched);
        self.busy = true;
        cx.notify();
        let fut = async move { smart::set_policy(&app, &rk, keep, delete_watched).await };
        form::run(window, cx, fut, Some("Smart download saved".into()), false, |d, ok, window, cx| d.finish(ok, window, cx));
    }

    fn remove(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(existing) = &self.existing else { return };
        if !self.guard.valid(cx) {
            window.close_dialog(cx);
            return;
        }
        let (app, id, also) = (core(cx), existing.policy.id.clone(), self.also_delete);
        self.busy = true;
        cx.notify();
        let fut = async move { smart::remove_policy(&app, &id, also) };
        let ok = if also { "Policy and downloads removed" } else { "Policy removed" };
        form::run(window, cx, fut, Some(ok.into()), false, |d, ok, window, cx| d.finish(ok, window, cx));
    }
}

impl Render for KeepDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let noun = if self.item.kind() == "playlist" { "items" } else { "episodes" };
        let busy = self.busy;
        let valid = keep_count(&self.keep.read(cx).value()).is_some();
        let row = |first: bool| {
            h_flex().px_4().py_3p5().gap_6().items_center().justify_between().when(!first, |d| d.border_t_1().border_color(theme.border))
        };
        let text = |title: String, desc: String| {
            v_flex().flex_1().min_w_0().gap_0p5().child(div().text_sm().font_medium().child(title)).child(div().text_sm().text_color(theme.muted_foreground).child(desc))
        };
        let current = self.existing.as_ref().map(|p| {
            format!(
                "{} downloaded{}{}",
                p.downloads,
                if p.bytes > 0 { format!(", {}", format::bytes(p.bytes)) } else { String::new() },
                if p.policy.enabled { "" } else { " (paused)" }
            )
        });
        v_flex()
            .child(form::desc(
                format!(
                    "{}: keep the next unwatched {noun} downloaded. New ones are fetched as you watch, and as they are added to the server.",
                    self.item.title()
                ),
                cx,
            ))
            .child(
                crate::widgets::rows(cx)
                    .child(row(true).child(text("Keep next".into(), format!("Unwatched {noun} to keep on this computer."))).child(div().w(px(80.)).child(Input::new(&self.keep).disabled(busy))))
                    .child(
                        row(false).child(text("Remove after watching".into(), "Delete these downloads once they are watched.".into())).child(
                            Switch::new("keep-delete-watched").checked(self.delete_watched).disabled(busy).on_click(cx.listener(|this, on: &bool, _, cx| {
                                this.delete_watched = *on;
                                cx.notify();
                            })),
                        ),
                    )
                    .when_some(current, |d, current| {
                        d.child(
                            row(false).child(text("Currently".into(), current)).child(
                                Checkbox::new("keep-also-delete").label("Also delete them").checked(self.also_delete).disabled(busy).on_click(cx.listener(|this, on: &bool, _, cx| {
                                    this.also_delete = *on;
                                    cx.notify();
                                })),
                            ),
                        )
                    }),
            )
            .child(
                form::foot(cx)
                    .when(self.existing.is_some(), |d| {
                        d.child(div().mr_auto().child(Button::new("stop").danger().label("Stop keeping").disabled(busy).on_click(cx.listener(|this, _, window, cx| this.remove(window, cx)))))
                    })
                    .child(Button::new("cancel").outline().label("Cancel").disabled(busy).on_click(|_, window, cx| window.close_dialog(cx)))
                    .child(
                        Button::new("save")
                            .primary()
                            .label(if self.existing.is_some() { "Save" } else { "Keep downloaded" })
                            .disabled(busy || !valid)
                            .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    #[test]
    fn keep_counts_are_one_to_fifty() {
        assert_eq!(keep_count(" 3 "), Some(3));
        assert_eq!(keep_count("0"), None);
        assert_eq!(keep_count("51"), None);
        assert_eq!(keep_count("x"), None);
    }
}
