//! Building blocks shared by the pages: page scaffolding, cards, rows of
//! cards, and small dialogs.

pub mod card;
pub mod hub;
pub mod row;

use crate::prelude::*;
use gpui_kit::component::button::Button;
use gpui_kit::component::dialog::AlertDialog;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Icon, h_flex, v_flex};

pub use card::Card;
pub use row::CardRow;

use crate::theme::GUTTER;

/// The scrolling page body: page padding, content stacked vertically.
///
/// Children of a scrolling flex column shrink to fit unless told not to, so
/// give fixed-height children `flex_none()` (or put them in a plain `div()`).
pub fn page(id: impl Into<ElementId>) -> Stateful<Div> {
    div().id(id).size_full().overflow_y_scroll().pt(px(28.)).pb(px(64.)).px(GUTTER)
}

/// A page title with an optional subtitle on the left and actions on the right.
pub fn page_head(title: impl Into<SharedString>, sub: Option<SharedString>, actions: Option<AnyElement>, cx: &App) -> Div {
    h_flex()
        .mb_6()
        .items_end()
        .justify_between()
        .gap_6()
        .child(
            v_flex()
                .min_w_0()
                .child(div().text_3xl().font_semibold().line_height(rems(2.25)).child(title.into()))
                .when_some(sub, |d, sub| d.child(div().mt_1().text_color(cx.theme().muted_foreground).child(sub))),
        )
        .when_some(actions, |d, a| d.child(a))
}

/// A section heading with optional controls on the right.
pub fn section_head(title: impl Into<SharedString>, right: Option<AnyElement>) -> Div {
    h_flex()
        .mt(px(36.))
        .mb_3()
        .items_center()
        .justify_between()
        .gap_4()
        .child(div().text_base().font_semibold().child(title.into()))
        .when_some(right, |d, r| d.child(r))
}

/// Divided rows inside a card (tracks, playlists, history, settings).
pub fn rows(cx: &App) -> Div {
    v_flex()
        .bg(cx.theme().colors.group_box.opacity(0.6))
        .border_1()
        .border_color(cx.theme().border)
        .rounded(px(12.))
        .overflow_hidden()
}

/// One row inside [`rows`]; `first` leaves out the divider above it.
pub fn list_row(first: bool, cx: &App) -> Div {
    h_flex().min_h(px(48.)).px_4().py_2().gap_3().items_center().when(!first, |d| d.border_t_1().border_color(cx.theme().border))
}

/// Centred spinner while a page loads.
pub fn loading() -> Div {
    h_flex().w_full().py(px(80.)).justify_center().child(Spinner::new().large())
}

/// An empty state: a title and an optional explanation.
pub fn empty(title: impl Into<SharedString>, text: Option<SharedString>, cx: &App) -> Div {
    v_flex()
        .w_full()
        .py(px(64.))
        .items_center()
        .gap_1()
        .child(div().font_medium().child(title.into()))
        .when_some(text, |d, t| d.child(div().text_sm().max_w(px(440.)).text_center().text_color(cx.theme().muted_foreground).child(t)))
}

/// An error in place of a page's content.
pub fn failed(message: impl Into<SharedString>, cx: &App) -> Div {
    empty("Something went wrong", Some(message.into()), cx)
}

/// A ghost icon button.
pub fn icon_button(id: impl Into<ElementId>, icon: impl Into<Icon>, tooltip: impl Into<SharedString>) -> Button {
    Button::new(id).ghost().small().icon(icon).tooltip(tooltip.into())
}

/// Ask before something destructive; `on_ok` runs when confirmed.
pub fn confirm(
    title: impl Into<SharedString>,
    text: Option<SharedString>,
    action: impl Into<SharedString>,
    danger: bool,
    window: &mut Window,
    cx: &mut App,
    on_ok: impl Fn(&mut Window, &mut App) + 'static,
) {
    let title = title.into();
    let action = action.into();
    let on_ok = std::rc::Rc::new(on_ok);
    window.open_alert_dialog(cx, move |dialog: AlertDialog, _, _| {
        let on_ok = on_ok.clone();
        dialog
            .title(title.clone())
            .when_some(text.clone(), |d, t| d.description(t))
            .ok_text(action.clone())
            .when(danger, |d| d.ok_variant(gpui_kit::component::button::ButtonVariant::Danger))
            .show_cancel(true)
            .on_ok(move |_, window, cx| {
                on_ok(window, cx);
                true
            })
    });
}

/// An outlined badge (`<Badge variant="outline">`).
pub fn badge(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .flex_none()
        .px_2()
        .py_0p5()
        .rounded(px(6.))
        .border_1()
        .border_color(cx.theme().border)
        .text_xs()
        .font_medium()
        .text_color(cx.theme().foreground)
        .child(text.into())
}

/// A bordered empty state with an icon, an optional title and an optional explanation.
pub fn empty_box(icon: IconName, title: Option<SharedString>, text: Option<SharedString>, cx: &App) -> Div {
    let theme = cx.theme();
    v_flex()
        .w_full()
        .flex_none()
        .items_center()
        .gap_2()
        .p(px(48.))
        .rounded(px(12.))
        .border_1()
        .border_color(theme.border)
        .child(
            div()
                .mb_2()
                .size_10()
                .rounded(px(8.))
                .bg(theme.muted)
                .flex()
                .items_center()
                .justify_center()
                .child(Icon::new(icon).size_5()),
        )
        .when_some(title, |d, t| d.child(div().text_lg().font_medium().child(t)))
        .when_some(text, |d, t| d.child(div().text_sm().max_w(px(380.)).text_center().text_color(theme.muted_foreground).child(t)))
}

/// A settings-style row: a title and an optional description on the left, a control on the right.
pub fn setting(first: bool, title: impl Into<SharedString>, sub: Option<SharedString>, control: Option<AnyElement>, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .flex_none()
        .items_center()
        .justify_between()
        .gap(px(28.))
        .px_4()
        .py_3p5()
        .when(!first, |d| d.border_t_1().border_color(theme.border))
        .child(
            v_flex()
                .min_w_0()
                .child(div().font_medium().child(title.into()))
                .when_some(sub, |d, s| d.child(div().mt_0p5().text_size(px(13.)).text_color(theme.muted_foreground).child(s))),
        )
        .when_some(control, |d, c| d.child(h_flex().flex_none().items_center().gap_2().child(c)))
}
