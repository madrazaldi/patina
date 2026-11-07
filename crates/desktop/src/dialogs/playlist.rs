//! "Add to playlist": add one item (or a bulk selection) to a playlist of its
//! kind, or to a new one.

use crate::prelude::*;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use patina_core::ops::lists;
use patina_plex::Metadata;

use super::form::{self, Guard, Modal};
use crate::art::art_of;
use crate::core::{Load, bg, core};
use crate::format;
use crate::store;

pub struct PlaylistDialog {
    items: Vec<Metadata>,
    guard: Guard,
    lists: Load<Vec<Metadata>>,
    name: Entity<InputState>,
    _subs: Vec<Subscription>,
}

/// Open the dialog for `items` (all of one playlist kind).
pub fn open(items: Vec<Metadata>, window: &mut Window, cx: &mut App) {
    if items.is_empty() {
        return;
    }
    let view = cx.new(|cx| PlaylistDialog::new(items, window, cx));
    form::show(view, "Add to playlist", form::NARROW, window, cx);
}

impl Modal for PlaylistDialog {}

impl PlaylistDialog {
    fn new(items: Vec<Metadata>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("New playlist"));
        let subs = vec![cx.subscribe_in(&name, window, |this, _, ev: &InputEvent, window, cx| match ev {
            InputEvent::PressEnter { .. } => this.create(window, cx),
            InputEvent::Change => cx.notify(),
            _ => {}
        })];
        let kind = format::playlist_kind(&items[0]).to_string();
        let app = core(cx);
        cx.spawn(async move |this, cx| {
            let result = bg(async move { Ok::<_, anyhow::Error>(app.server()?.playlists(Some(&kind), Some(false)).await?) }).await;
            this.update(cx, |d, cx| {
                if d.guard.valid(cx) {
                    d.lists = Load::from_result(result);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        Self { items, guard: Guard::new(cx), lists: Load::Loading, name, _subs: subs }
    }

    fn add(&mut self, playlist: Option<String>, title: Option<String>, ok: String, window: &mut Window, cx: &mut Context<Self>) {
        window.close_dialog(cx);
        if !self.guard.valid(cx) {
            return;
        }
        let keys = self.items.iter().map(|m| m.rating_key()).collect::<Vec<_>>().join(",");
        let kind = format::playlist_kind(&self.items[0]);
        let app = core(cx);
        store::act(cx, async move { lists::add_to_playlist(&app, &keys, playlist.as_deref(), title.as_deref(), kind).await }, Some(ok), false);
    }

    fn create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = self.name.read(cx).value().trim().to_string();
        if !title.is_empty() {
            let ok = format!("Created playlist “{title}”");
            self.add(None, Some(title), ok, window, cx);
        }
    }
}

/// The square artwork beside a row.
pub(super) fn thumb(source: Option<&str>, w: f32, h: f32, cx: &App) -> Div {
    div()
        .flex_none()
        .w(px(w))
        .h(px(h))
        .rounded(px(4.))
        .overflow_hidden()
        .bg(cx.theme().muted)
        .children(art_of(source, w, h, None).map(|i| i.size_full().object_fit(ObjectFit::Cover)))
}

impl Render for PlaylistDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let desc = if self.items.len() > 1 { format!("{} selected titles", self.items.len()) } else { self.items[0].full_title() };
        let body = match &self.lists {
            Load::Failed(e) => form::error(e.clone(), cx).into_any_element(),
            Load::Loading => form::spinner().into_any_element(),
            Load::Ready(l) if l.is_empty() => form::empty("No playlists yet", "No playlists of this kind yet.", cx).into_any_element(),
            Load::Ready(l) => v_flex()
                .id("playlists")
                .max_h(px(300.))
                .overflow_y_scroll()
                .gap_1()
                .children(l.iter().enumerate().map(|(i, p)| {
                    let id = p.rating_key().to_string();
                    let title = p.title().to_string();
                    form::item_row(("playlist", i), false, cx)
                        .child(thumb(p.composite.as_deref().or(p.thumb.as_deref()), 40., 40., cx))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(div().text_sm().font_medium().truncate().child(title.clone()))
                                .child(div().text_xs().text_color(theme.muted_foreground).truncate().child(format::subtitle(p))),
                        )
                        .child(Icon::new(IconName::Plus).size_4().text_color(theme.muted_foreground))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.add(Some(id.clone()), None, format!("Added to “{title}”"), window, cx)
                        }))
                }))
                .into_any_element(),
        };
        let empty_name = self.name.read(cx).value().trim().is_empty();
        v_flex()
            .child(form::desc(desc, cx))
            .child(body)
            .child(
                h_flex()
                    .mt_4()
                    .gap_2()
                    .child(div().flex_1().child(Input::new(&self.name)))
                    .child(Button::new("create").outline().label("Create").disabled(empty_name).on_click(cx.listener(|this, _, window, cx| this.create(window, cx)))),
            )
            .child(form::foot(cx).child(Button::new("cancel").outline().label("Cancel").on_click(|_, window, cx| window.close_dialog(cx))))
    }
}
