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
    v_scroll(id).size_full().pt(px(28.)).pb(px(64.)).px(GUTTER)
}

/// A column that scrolls vertically. Restricted to the gesture's axis like [`h_scroll`]:
/// otherwise GPUI turns a sideways swipe over one of its rows into a vertical scroll too.
pub fn v_scroll(id: impl Into<ElementId>) -> Stateful<Div> {
    div().id(id).overflow_y_scroll().restrict_scroll_to_axis()
}

/// A row that scrolls sideways. Restricted to the gesture's axis: otherwise GPUI turns a
/// vertical wheel over the row into a sideways scroll, while the page scrolls as well.
pub fn h_scroll(id: impl Into<ElementId>) -> Stateful<Div> {
    h_flex().id(id).overflow_x_scroll().restrict_scroll_to_axis()
}

/// A box that scrolls vertically inside a scrolling page. See [`keep_wheel`].
pub fn inner_v_scroll(id: impl Into<ElementId>, window: &mut Window, cx: &mut App) -> Stateful<Div> {
    let id = id.into();
    let scroll = window.use_keyed_state(id.clone(), cx, |_, _| ScrollHandle::new()).read(cx).clone();
    keep_wheel(v_scroll(id).track_scroll(&scroll), scroll)
}

/// Makes `el`, a vertical scroller tracked by `scroll`, keep the wheel while it can move that
/// way and pass it on to the page behind at its ends, as a browser does. GPUI hands every wheel
/// event to all the scrollers under the pointer, so the page and the box would move together.
pub fn keep_wheel(el: Stateful<Div>, scroll: ScrollHandle) -> Stateful<Div> {
    el.on_scroll(move |e, window, cx| {
        // The element's own scroll listener has already run and added the delta, unclamped.
        let dy = e.delta.pixel_delta(window.line_height()).y;
        let from_top = dy - scroll.offset().y;
        let max = scroll.max_offset().y;
        let room = if dy < px(0.) {
            from_top < max - px(0.5)
        } else {
            dy > px(0.) && from_top > px(0.5)
        };
        if room {
            cx.stop_propagation();
        }
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    // `#[gpui::test]` expands to `#[test]` and `gpui::` paths, which the prelude's glob makes
    // ambiguous (gpui-kit's `test` macro, and `gpui` as gpui-kit itself); name both outright.
    use ::core::prelude::v1::test;
    use gpui_kit as gpui;

    /// A vertically scrolling page holding one sideways row, like Home.
    struct Page {
        page: ScrollHandle,
        row: ScrollHandle,
    }

    impl Render for Page {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            v_scroll("page")
                .size(px(200.))
                .track_scroll(&self.page)
                .child(
                    h_scroll("row")
                        .w(px(200.))
                        .h(px(100.))
                        .flex_none()
                        .track_scroll(&self.row)
                        .children((0..10usize).map(|i| div().w(px(80.)).h_full().flex_none().child(format!("{i}")))),
                )
                .child(div().h(px(600.)).flex_none())
        }
    }

    /// Wheels once over the row; returns the page's and the row's scroll offsets.
    fn wheel_over_row(cx: &mut TestAppContext, delta: Point<Pixels>) -> (Point<Pixels>, Point<Pixels>) {
        let (page, row) = (ScrollHandle::new(), ScrollHandle::new());
        let window = cx.add_window({
            let (page, row) = (page.clone(), row.clone());
            move |_, _| Page { page, row }
        });
        let cx = VisualTestContext::from_window(*window, cx).into_mut();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let position = point(px(100.), px(50.));
        cx.simulate_mouse_move(position, None, Modifiers::default());
        cx.simulate_event(ScrollEvent { position, delta: ScrollDelta::Pixels(delta), touch_phase: TouchPhase::Moved, ..Default::default() });
        (page.offset(), row.offset())
    }

    /// A vertically scrolling page holding a 100px box of 400px content, like Server's devices.
    struct Nested {
        page: ScrollHandle,
        inner: ScrollHandle,
    }

    impl Render for Nested {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            v_scroll("page")
                .size(px(200.))
                .track_scroll(&self.page)
                .child(keep_wheel(v_scroll("inner").track_scroll(&self.inner), self.inner.clone()).w(px(200.)).h(px(100.)).flex_none().child(div().h(px(400.)).flex_none()))
                .child(div().h(px(600.)).flex_none())
        }
    }

    /// Wheels down over the box in steps of `step`; returns the page's and the box's offsets.
    fn wheel_over_box(cx: &mut TestAppContext, steps: &[f32]) -> (Pixels, Pixels) {
        let (page, inner) = (ScrollHandle::new(), ScrollHandle::new());
        let window = cx.add_window({
            let (page, inner) = (page.clone(), inner.clone());
            move |_, _| Nested { page, inner }
        });
        let cx = VisualTestContext::from_window(*window, cx).into_mut();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let position = point(px(100.), px(50.));
        cx.simulate_mouse_move(position, None, Modifiers::default());
        for dy in steps {
            cx.simulate_event(ScrollEvent { position, delta: ScrollDelta::Pixels(point(px(0.), px(*dy))), touch_phase: TouchPhase::Moved, ..Default::default() });
            cx.update(|window, cx| window.draw(cx).clear(cx));
        }
        (page.offset().y, inner.offset().y)
    }

    #[gpui::test]
    fn a_wheel_over_a_nested_box_scrolls_only_the_box(cx: &mut TestAppContext) {
        let (page, inner) = wheel_over_box(cx, &[-50.]);
        assert_eq!(inner, px(-50.));
        assert_eq!(page, px(0.), "the page must stay put while the box can scroll");
    }

    #[gpui::test]
    fn at_the_end_of_a_nested_box_the_wheel_scrolls_the_page(cx: &mut TestAppContext) {
        // 300px reaches the box's end; the next step goes to the page.
        let (page, inner) = wheel_over_box(cx, &[-300., -40.]);
        assert_eq!(inner, px(-300.));
        assert_eq!(page, px(-40.));
        // And back up: the box takes it again, the page stays.
        let (page, inner) = wheel_over_box(cx, &[-300., 40.]);
        assert_eq!(inner, px(-260.));
        assert_eq!(page, px(0.));
    }

    #[gpui::test]
    fn a_vertical_wheel_over_a_row_scrolls_only_the_page(cx: &mut TestAppContext) {
        let (page, row) = wheel_over_row(cx, point(px(0.), px(-50.)));
        assert_eq!(row.x, px(0.), "the row must not scroll sideways");
        assert!(page.y < px(0.), "the page scrolls down");
    }

    #[gpui::test]
    fn a_sideways_wheel_scrolls_the_row(cx: &mut TestAppContext) {
        let (page, row) = wheel_over_row(cx, point(px(-50.), px(0.)));
        assert!(row.x < px(0.), "the row scrolls sideways");
        assert_eq!(page.y, px(0.), "the page must not scroll down");
    }
}
