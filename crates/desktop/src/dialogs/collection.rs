//! "Add to collection": an existing collection of the item's library, or a new one.

use crate::prelude::*;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use patina_core::ops::edit;
use patina_plex::{Metadata, MetadataOptions};

use super::form::{self, Guard, Modal};
use super::playlist::thumb;
use crate::core::{Load, bg, core};
use crate::format;
use crate::store;

/// Collections of the item's library, and the titles of those it is already in.
struct Found {
    lists: Vec<Metadata>,
    member: Vec<String>,
}

pub struct CollectionDialog {
    item: Metadata,
    guard: Guard,
    found: Load<Found>,
    name: Entity<InputState>,
    _subs: Vec<Subscription>,
}

pub fn open(item: Metadata, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| CollectionDialog::new(item, window, cx));
    form::show(view, "Add to collection", form::NARROW, window, cx);
}

impl Modal for CollectionDialog {}

impl CollectionDialog {
    fn new(item: Metadata, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("New collection"));
        let subs = vec![cx.subscribe_in(&name, window, |this, _, ev: &InputEvent, window, cx| match ev {
            InputEvent::PressEnter { .. } => this.create(window, cx),
            InputEvent::Change => cx.notify(),
            _ => {}
        })];
        let app = core(cx);
        let target = item.clone();
        cx.spawn(async move |this, cx| {
            let result = bg(async move {
                let server = app.server()?;
                // Menu items may be partial; the full item knows its section and collections.
                let full = if target.library_section_id.is_some() && !target.collections.is_empty() {
                    Some(target.clone())
                } else {
                    server.metadata(target.rating_key(), MetadataOptions::default()).await?
                };
                let section = full.as_ref().and_then(|m| m.library_section_id).or(target.library_section_id);
                let section = section.ok_or_else(|| anyhow::anyhow!("This item isn't in a library."))?;
                let lists = server.collections(&section.to_string()).await?;
                let member = full.map(|m| m.collections.iter().map(|t| t.tag.clone().unwrap_or_default()).collect()).unwrap_or_default();
                Ok::<_, anyhow::Error>(Found { lists, member })
            })
            .await;
            this.update(cx, |d, cx| {
                if d.guard.valid(cx) {
                    d.found = Load::from_result(result);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        Self { item, guard: Guard::new(cx), found: Load::Loading, name, _subs: subs }
    }

    fn add(&mut self, collection: Option<String>, title: Option<String>, ok: String, window: &mut Window, cx: &mut Context<Self>) {
        window.close_dialog(cx);
        if !self.guard.valid(cx) {
            return;
        }
        let rk = self.item.rating_key().to_string();
        let app = core(cx);
        store::act(cx, async move { edit::add_to_collection(&app, &rk, collection.as_deref(), title.as_deref()).await }, Some(ok), true);
    }

    fn create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = self.name.read(cx).value().trim().to_string();
        if !title.is_empty() {
            let ok = format!("Created collection “{title}”");
            self.add(None, Some(title), ok, window, cx);
        }
    }
}

impl Render for CollectionDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let body = match &self.found {
            Load::Failed(e) => form::error(e.clone(), cx).into_any_element(),
            Load::Loading => form::spinner().into_any_element(),
            Load::Ready(f) if f.lists.is_empty() => form::empty("No collections yet", "This library has no collections yet.", cx).into_any_element(),
            Load::Ready(f) => v_flex()
                .id("collections")
                .max_h(px(300.))
                .overflow_y_scroll()
                .gap_1()
                .children(f.lists.iter().enumerate().map(|(i, c)| {
                    let id = c.rating_key().to_string();
                    let title = c.title().to_string();
                    let has = f.member.contains(&title);
                    form::item_row(("collection", i), has, cx)
                        .child(thumb(c.thumb.as_deref().or(c.composite.as_deref()), 32., 48., cx))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(div().text_sm().font_medium().truncate().child(title.clone()))
                                .child(div().text_xs().text_color(theme.muted_foreground).truncate().child(if has {
                                    "Already in this collection".to_string()
                                } else {
                                    format::subtitle(c)
                                })),
                        )
                        .child(Icon::new(if has { IconName::Check } else { IconName::Plus }).size_4().text_color(theme.muted_foreground))
                        .when(!has, |d| {
                            d.on_click(cx.listener(move |this, _, window, cx| {
                                this.add(Some(id.clone()), None, format!("Added to “{title}”"), window, cx)
                            }))
                        })
                }))
                .into_any_element(),
        };
        let empty_name = self.name.read(cx).value().trim().is_empty();
        v_flex()
            .child(form::desc(self.item.full_title(), cx))
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
