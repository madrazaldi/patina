//! Pieces the listing pages share: poster grids, status placeholders, badges,
//! select buttons and the infinite-scroll sentinel.

use crate::prelude::*;
use std::rc::Rc;

use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::spinner::Spinner;
use patina_plex::Metadata;

use crate::format::Shape;
use crate::store::store;
use crate::widgets::Card;

/// A click handler that can be cloned into several elements.
pub type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// The `.poster-grid`: cards of the user's poster size, wider for landscape ones.
pub fn poster_grid<I: IntoElement>(cards: impl IntoIterator<Item = I>) -> Div {
    h_flex().flex_wrap().items_start().gap_x(px(20.)).gap_y(px(28.)).children(cards)
}

/// The width of one card in a poster grid.
pub fn card_width(shape: Shape, cx: &App) -> f32 {
    let w = store(cx).read(cx).poster_width();
    if shape == Shape::Landscape { w * 1.75 } else { w }
}

/// A grid of item cards, with wider cells when `wide`; `shape` forces one shape for all of them.
pub fn item_grid(id: &str, items: Vec<Metadata>, wide: bool, shape: Option<Shape>, cx: &App) -> Div {
    let w = card_width(if wide { Shape::Landscape } else { Shape::Poster }, cx);
    poster_grid(items.into_iter().enumerate().map(|(i, item)| {
        let card = Card::new(ElementId::Name(format!("{id}:{}:{i}", item.rating_key()).into()), item).width(w);
        match shape {
            Some(s) => card.shape(s),
            None => card,
        }
    }))
}

/// A bordered placeholder box (`Status` with a title and text).
pub fn status_empty(title: impl Into<SharedString>, text: Option<SharedString>, cx: &App) -> Div {
    let theme = cx.theme();
    v_flex()
        .w_full()
        .min_h(px(176.))
        .p_6()
        .items_center()
        .justify_center()
        .gap_1p5()
        .rounded(px(12.))
        .border_1()
        .border_color(theme.border)
        .child(div().text_base().font_medium().child(title.into()))
        .when_some(text, |d, t| d.child(div().max_w(px(440.)).text_center().text_sm().text_color(theme.muted_foreground).child(t)))
}

/// A spinner where a page's content will be.
pub fn status_loading() -> Div {
    h_flex().w_full().min_h(px(176.)).items_center().justify_center().child(Spinner::new())
}

/// The content failed to load: the error and a Retry button.
pub fn status_error(id: &'static str, error: impl Into<SharedString>, retry: Option<Handler>, cx: &App) -> Div {
    let theme = cx.theme();
    v_flex()
        .w_full()
        .min_h(px(176.))
        .p_6()
        .items_center()
        .justify_center()
        .gap_3()
        .rounded(px(12.))
        .border_1()
        .border_color(theme.border)
        .child(
            div()
                .size_10()
                .rounded(px(8.))
                .bg(theme.muted)
                .flex()
                .items_center()
                .justify_center()
                .child(Icon::new(IconName::CircleAlert).size_5()),
        )
        .child(div().text_base().font_medium().child("Could not load this page"))
        .child(
            div()
                .max_w(px(576.))
                .w_full()
                .px_4()
                .py_3()
                .rounded(px(8.))
                .border_1()
                .border_color(theme.danger.opacity(0.4))
                .text_sm()
                .text_color(theme.danger)
                .child(error.into()),
        )
        .when_some(retry, |d, retry| d.child(Button::new(id).outline().label("Retry").on_click(move |_, window, cx| retry(window, cx))))
}

/// Above stale content: it could not be refreshed.
pub fn stale_alert(id: &'static str, error: impl Into<SharedString>, retry: Option<Handler>, cx: &App) -> Div {
    alert(id, IconName::TriangleAlert, Some("Showing saved results".into()), error.into(), retry.map(|r| ("Retry".into(), r)), cx)
}

/// An alert box: icon, optional title, text and an optional action button.
pub fn alert(id: &'static str, icon: IconName, title: Option<SharedString>, text: SharedString, action: Option<(SharedString, Handler)>, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .w_full()
        .mb_4()
        .px_4()
        .py_3()
        .gap_3()
        .items_start()
        .rounded(px(10.))
        .border_1()
        .border_color(theme.border)
        .bg(theme.colors.group_box)
        .child(div().pt_0p5().child(Icon::new(icon).size_4()))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .when_some(title, |d, t| d.child(div().text_sm().font_medium().child(t)))
                .child(div().text_sm().text_color(theme.muted_foreground).child(text)),
        )
        .when_some(action, |d, (label, run)| d.child(Button::new(id).outline().xsmall().label(label).on_click(move |_, window, cx| run(window, cx))))
}

/// How a [`badge`] is coloured.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Plain,
    Tint,
    Danger,
}

/// An outline badge.
pub fn badge(text: impl Into<SharedString>, tone: Tone, cx: &App) -> Div {
    let theme = cx.theme();
    let tint = store(cx).read(cx).tint();
    let d = div().flex_none().px_2().py_0p5().rounded(px(6.)).border_1().text_xs().font_medium().line_height(px(16.)).whitespace_nowrap();
    match tone {
        Tone::Plain => d.border_color(theme.border),
        Tone::Tint => d.border_color(tint.opacity(0.4)).bg(tint.opacity(0.1)).text_color(tint),
        Tone::Danger => d.border_color(theme.danger.opacity(0.4)).text_color(theme.danger),
    }
    .child(text.into())
}

/// A native-select look-alike `width` wide (or as wide as its parent): an outline button showing
/// the choice, with a menu of the options.
pub fn select<V: Clone + PartialEq + 'static>(
    id: impl Into<ElementId>,
    width: Option<Pixels>,
    options: Vec<(V, SharedString)>,
    value: &V,
    disabled: bool,
    on_pick: impl Fn(V, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let label = options.iter().find(|(v, _)| v == value).map(|(_, l)| l.clone()).unwrap_or_default();
    let on_pick = Rc::new(on_pick);
    let current = value.clone();
    Button::new(id)
        .outline()
        .small()
        .map(|b| match width {
            Some(w) => b.w(w),
            None => b.w_full(),
        })
        .disabled(disabled)
        .child(h_flex().w_full().gap_2().justify_between().child(div().truncate().child(label)).child(Icon::new(IconName::ChevronDown).size_3p5()))
        .dropdown_menu(move |mut menu, _, _| {
            for (v, l) in options.clone() {
                let on_pick = on_pick.clone();
                let checked = v == current;
                menu = menu.item(PopupMenuItem::new(l).checked(checked).on_click(move |_, window, cx| on_pick(v.clone(), window, cx)));
            }
            menu.min_w(width.unwrap_or(px(200.))).max_h(px(360.)).scrollable(true)
        })
        .into_any_element()
}

/// Placed after a list: calls `more` once it scrolls within 1200px of the bottom of the window
/// (the infinite-scroll sentinel), with a spinner while `loading`.
pub fn more_sentinel(loading: bool, more: impl Fn(&mut App) + 'static) -> Div {
    let more = Rc::new(more);
    div()
        .relative()
        .flex_none()
        .w_full()
        .h(px(60.))
        .flex()
        .items_center()
        .justify_center()
        .when(loading, |d| d.child(Spinner::new()))
        .when(!loading, |d| {
            d.child(
                canvas(
                    move |bounds, window, cx| {
                        if bounds.origin.y < window.viewport_size().height + px(1200.) {
                            let more = more.clone();
                            cx.defer(move |cx| more(cx));
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
        })
}

/// The `.tabs` pill group; `on_pick` gets the index of the clicked tab.
pub fn tabs(id: &'static str, labels: Vec<SharedString>, selected: usize, on_pick: impl Fn(usize, &mut Window, &mut App) + 'static, cx: &App) -> Div {
    let theme = cx.theme().clone();
    let on_pick = Rc::new(on_pick);
    h_flex().flex_none().h_8().p(px(3.)).gap_0p5().rounded(px(8.)).bg(theme.muted).children(labels.into_iter().enumerate().map(|(i, label)| {
        let on = i == selected;
        let on_pick = on_pick.clone();
        div()
            .id((id, i))
            .h_full()
            .px_2()
            .flex()
            .items_center()
            .rounded(px(6.))
            .border_1()
            .text_sm()
            .font_medium()
            .cursor_pointer()
            .when(on, |d| d.border_color(theme.input).bg(theme.input.opacity(0.3)).text_color(theme.foreground))
            .when(!on, |d| d.border_color(transparent_black()).text_color(theme.muted_foreground).hover(|s| s.text_color(theme.foreground)))
            .on_click(move |_, window, cx| on_pick(i, window, cx))
            .child(label)
    }))
}

/// Bits of text separated by " · ", leaving out empty ones.
pub fn dots<S: AsRef<str>>(bits: impl IntoIterator<Item = Option<S>>) -> String {
    bits.into_iter().flatten().filter(|s| !s.as_ref().is_empty()).map(|s| s.as_ref().to_string()).collect::<Vec<_>>().join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    #[test]
    fn dots_skip_missing_and_empty_bits() {
        assert_eq!(dots([Some("Movie"), None, Some(""), Some("4K")]), "Movie · 4K");
        assert_eq!(dots::<&str>([None, None]), "");
    }
}
