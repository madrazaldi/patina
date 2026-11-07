//! Choose which server's copy of a title to use, from a search across servers.

use crate::prelude::*;
use patina_core::ops::xsearch::{Group, ServerCopy};

use super::form::{self, Guard, Modal};
use crate::actions::{self, PlayOptions};
use crate::nav::{self, Route};

pub struct CopyDialog {
    group: Group,
    /// Playing or opening is dropped if the account or server changed meanwhile.
    guard: Guard,
}

pub fn open(group: Group, window: &mut Window, cx: &mut App) {
    let title = match group.year {
        Some(y) => format!("{} ({y})", group.title),
        None => group.title.clone(),
    };
    let view = cx.new(|cx| CopyDialog { group, guard: Guard::new(cx) });
    form::show(view, title, form::WIDE, window, cx);
}

impl Modal for CopyDialog {}

impl CopyDialog {
    fn play(&self, copy: &ServerCopy, resume: bool, window: &mut Window, cx: &mut App) {
        window.close_dialog(cx);
        if self.guard.valid(cx) {
            actions::play_with(&copy.item, PlayOptions { resume, ..Default::default() }, cx);
        }
    }

    fn details(&self, copy: &ServerCopy, window: &mut Window, cx: &mut App) {
        window.close_dialog(cx);
        if self.guard.valid(cx) {
            nav::push(Route::for_item(&copy.item), cx);
        }
    }
}

impl Render for CopyDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let n = self.group.copies.len();
        let desc = format!("{}. Choose the copy to play.", if n == 1 { "On one server".to_string() } else { format!("On {n} servers") });
        v_flex()
            .child(form::desc(desc, cx))
            .child(crate::widgets::rows(cx).children(self.group.copies.iter().enumerate().map(|(i, copy)| {
                let resumable = copy.item.progress().is_some();
                let (c1, c2, c3) = (copy.clone(), copy.clone(), copy.clone());
                h_flex()
                    .gap_3p5()
                    .p_3()
                    .items_center()
                    .when(i > 0, |d| d.border_t_1().border_color(theme.border))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .text_sm()
                                    .font_medium()
                                    .child(copy.server_name.clone())
                                    .when(copy.current, |d| d.child(div().px_2().rounded_full().bg(theme.secondary).text_xs().child("connected"))),
                            )
                            .when_some(copy.library.clone(), |d, l| d.child(div().text_xs().text_color(theme.muted_foreground).child(l)))
                            .children(copy.versions.iter().map(|v| div().mt_0p5().text_xs().text_color(theme.muted_foreground).child(v.label.clone())))
                            .when(copy.versions.is_empty(), |d| d.child(div().text_xs().text_color(theme.muted_foreground).child("No file information"))),
                    )
                    .child(
                        h_flex()
                            .gap_1p5()
                            .child(
                                Button::new(("play", i))
                                    .primary()
                                    .icon(IconName::Play)
                                    .label(if resumable { "Resume" } else { "Play" })
                                    .on_click(cx.listener(move |this, _, window, cx| this.play(&c1, true, window, cx))),
                            )
                            .when(resumable, |d| {
                                d.child(
                                    Button::new(("start", i))
                                        .outline()
                                        .icon(IconName::RotateCcw)
                                        .tooltip("Play from start")
                                        .on_click(cx.listener(move |this, _, window, cx| this.play(&c2, false, window, cx))),
                                )
                            })
                            .child(Button::new(("details", i)).outline().icon(IconName::Info).tooltip("Details").on_click(cx.listener(move |this, _, window, cx| this.details(&c3, window, cx)))),
                    )
            })))
            .child(form::foot(cx).child(Button::new("cancel").outline().label("Cancel").on_click(|_, window, cx| window.close_dialog(cx))))
    }
}
