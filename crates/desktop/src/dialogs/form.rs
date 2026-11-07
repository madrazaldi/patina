//! Pieces the dialogs share: the modal frame, form fields, pickers and list rows
//! (the webview's `.modal`, `NativeSelect`, `Item` and `Empty`).

use crate::prelude::*;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::spinner::Spinner;

use crate::store::store;

/// Width of a normal and of a wide dialog.
pub const NARROW: f32 = 440.;
pub const WIDE: f32 = 640.;

/// A dialog's view; while `busy` it cannot be dismissed with Escape or a click outside.
pub trait Modal: Render {
    fn busy(&self) -> bool {
        false
    }
}

/// Open `view` as a modal dialog. A tall one scrolls (the dialog body is a scrolling
/// column, so the view must not shrink to fit it).
pub fn show<V: Modal>(view: Entity<V>, title: impl Into<SharedString>, width: f32, window: &mut Window, cx: &mut App) {
    let title = title.into();
    window.open_dialog(cx, move |dialog, _, cx| {
        let busy = view.read(cx).busy();
        dialog.title(title.clone()).w(px(width)).overlay_closable(!busy).keyboard(!busy).child(div().flex_none().child(view.clone()))
    });
}

/// The account and server a dialog opened on: work for any other one is dropped.
#[derive(Clone, Debug)]
pub struct Guard(String);

impl Guard {
    pub fn new(cx: &App) -> Self {
        Self(store(cx).read(cx).scope())
    }

    pub fn valid(&self, cx: &App) -> bool {
        store(cx).read(cx).scope() == self.0
    }
}

/// Run a dialog's operation in the background: toast `ok` (and refresh the views
/// if asked) when it succeeds, or the error; then hand the outcome to `done`.
pub fn run<V: 'static, T: Send + 'static>(
    window: &Window,
    cx: &mut Context<V>,
    fut: impl Future<Output = anyhow::Result<T>> + Send + 'static,
    ok: Option<String>,
    refresh: bool,
    done: impl FnOnce(&mut V, bool, &mut Window, &mut Context<V>) + 'static,
) {
    cx.spawn_in(window, async move |this, cx| {
        let result = crate::core::bg(fut).await;
        let success = result.is_ok();
        // Toast even if the dialog went away meanwhile.
        cx.update(|_, cx| {
            store(cx).update(cx, |s, cx| match result {
                Ok(_) => {
                    if let Some(ok) = ok {
                        s.toast(ok, cx);
                    }
                    if refresh {
                        s.refresh(cx);
                    }
                }
                Err(e) => s.error(crate::core::message(&e), cx),
            })
        })
        .ok();
        this.update_in(cx, |v, window, cx| done(v, success, window, cx)).ok();
    })
    .detach();
}

/// The muted line under a dialog's title.
pub fn desc(text: impl Into<SharedString>, cx: &App) -> Div {
    div().mb_4().text_sm().text_color(cx.theme().muted_foreground).child(text.into())
}

/// The button row at the bottom of a dialog.
pub fn foot(cx: &App) -> Div {
    h_flex().mt_5().pt_4().gap_2().justify_end().items_center().border_t_1().border_color(cx.theme().border)
}

/// A small label above a field.
pub fn label(text: impl Into<SharedString>, cx: &App) -> Div {
    div().text_xs().font_medium().text_color(cx.theme().muted_foreground).child(text.into())
}

/// A labelled field: the label above its control.
pub fn field(text: impl Into<SharedString>, control: impl IntoElement, cx: &App) -> Div {
    v_flex().min_w_0().gap_1p5().child(label(text, cx)).child(control)
}

/// An error box (destructive alert).
pub fn error(text: impl Into<SharedString>, cx: &App) -> Div {
    let danger = cx.theme().danger;
    div().px_4().py_3().rounded(px(8.)).border_1().border_color(danger.opacity(0.4)).bg(danger.opacity(0.08)).text_sm().text_color(danger).child(text.into())
}

/// A centred spinner.
pub fn spinner() -> Div {
    h_flex().w_full().py_6().justify_center().child(Spinner::new())
}

/// A spinner with a line of text beside it.
pub fn waiting(text: impl Into<SharedString>, cx: &App) -> Div {
    h_flex().py_2().gap_2p5().items_center().text_sm().text_color(cx.theme().muted_foreground).child(Spinner::new().small()).child(text.into())
}

/// An empty state in a dashed box.
pub fn empty(title: impl Into<SharedString>, text: impl Into<SharedString>, cx: &App) -> Div {
    v_flex()
        .p_6()
        .gap_1()
        .items_center()
        .rounded(px(10.))
        .border_1()
        .border_dashed()
        .border_color(cx.theme().border)
        .child(div().text_sm().font_medium().child(title.into()))
        .child(div().text_sm().text_color(cx.theme().muted_foreground).child(text.into()))
}

/// A clickable list row with an image, a title and a description (shadcn's `Item`).
pub fn item_row(id: impl Into<ElementId>, disabled: bool, cx: &App) -> Stateful<Div> {
    let hover = cx.theme().muted;
    h_flex()
        .id(id)
        .w_full()
        .gap_3()
        .px_2()
        .py_1p5()
        .rounded(px(8.))
        .items_center()
        .when(disabled, |d| d.opacity(0.55))
        .when(!disabled, |d| d.cursor_pointer().hover(move |s| s.bg(hover)))
}

/// A select (the webview's `NativeSelect`): a button showing the chosen option that opens the list.
pub fn select(
    id: impl Into<ElementId>,
    options: Vec<(String, String)>,
    value: &str,
    disabled: bool,
    on_pick: impl Fn(String, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let current = options.iter().find(|(v, _)| v == value).map(|(_, l)| l.clone()).unwrap_or_default();
    let value = value.to_string();
    let on_pick = std::rc::Rc::new(on_pick);
    let many = options.len() > 12;
    Button::new(id)
        .outline()
        .w_full()
        .label(current)
        .dropdown_caret(true)
        .disabled(disabled)
        .dropdown_menu(move |mut menu, _, _| {
            if many {
                menu = menu.scrollable(true).max_h(px(360.));
            }
            for (v, l) in options.clone() {
                let on_pick = on_pick.clone();
                let checked = v == value;
                menu = menu.item(PopupMenuItem::new(l).checked(checked).on_click(move |_, window, cx| on_pick(v.clone(), window, cx)));
            }
            menu
        })
}

/// A pill toggle (the webview's `.chip`).
pub fn chip(id: impl Into<ElementId>, text: impl Into<SharedString>, on: bool) -> Button {
    let b = Button::new(id).small().rounded_full().label(text);
    if on { b.primary() } else { b.outline() }
}

/// A small outlined badge.
pub fn badge(text: impl Into<SharedString>, color: Option<Hsla>, cx: &App) -> Div {
    let theme = cx.theme();
    let fg = color.unwrap_or(theme.foreground.opacity(0.8));
    div()
        .flex_none()
        .px_2()
        .h_5()
        .flex()
        .items_center()
        .rounded_full()
        .border_1()
        .border_color(color.map_or(theme.border, |c| c.opacity(0.4)))
        .when_some(color, |d, c| d.bg(c.opacity(0.1)))
        .text_xs()
        .font_medium()
        .text_color(fg)
        .child(text.into())
}
