//! Seasons, episodes, albums, tracks or collection items below a detail header.

use crate::prelude::*;
use gpui_kit::component::menu::ContextMenuExt as _;
use patina_core::servers;
use patina_plex::Metadata;

use crate::actions;
use crate::art::art_of;
use crate::core::{Load, bg, core, message};
use crate::format;
use crate::menu::item_menu;
use crate::nav::{self, Route};
use crate::store::{self, store};
use crate::widgets::{self, Card, CardRow};

/// `list` with the entry at `from` moved to index `to`.
pub fn moved<T: Clone>(list: &[T], from: usize, to: usize) -> Vec<T> {
    let mut out = list.to_vec();
    if from < out.len() && to < out.len() && from != to {
        let x = out.remove(from);
        out.insert(to, x);
    }
    out
}

/// What is being dragged: the index of an entry in a reorderable list.
#[derive(Clone, Copy, Debug)]
pub struct Dragged(pub usize);

/// The title that follows the pointer while dragging.
pub struct DragLabel(pub SharedString);

impl Render for DragLabel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().px_3().py_1p5().rounded(px(8.)).bg(cx.theme().popover).border_1().border_color(cx.theme().border).text_sm().shadow_lg().child(self.0.clone())
    }
}

/// A wrapping grid of cards.
pub fn grid() -> Div {
    h_flex().flex_wrap().items_start().gap_5().w_full()
}

pub struct Children {
    item: Metadata,
    /// Collection sort: 2 is custom.
    sort: Option<String>,
    items: Load<Vec<Metadata>>,
    revision: u64,
    _subs: Vec<Subscription>,
}

impl Children {
    pub fn new(item: Metadata, sort: Option<String>, cx: &mut Context<Self>) -> Self {
        let subs = vec![cx.observe(&store(cx), |this: &mut Self, store, cx| {
            let s = store.read(cx);
            if s.revision != this.revision && s.affects_item(this.item.rating_key(), this.revision) {
                this.load(cx);
            }
        })];
        let mut this = Self { item, sort, items: Load::Loading, revision: 0, _subs: subs };
        this.load(cx);
        this
    }

    /// The item changed (reloaded); keep the list, refetch it.
    pub fn set_item(&mut self, item: Metadata, sort: Option<String>, cx: &mut Context<Self>) {
        self.item = item;
        self.sort = sort;
        cx.notify();
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        self.revision = store(cx).read(cx).revision;
        let app = core(cx);
        let rk = self.item.rating_key().to_string();
        let on = self.item.server_id().map(str::to_string);
        let collection = self.item.kind() == "collection";
        cx.spawn(async move |this, cx| {
            let result = bg(async move {
                let s = servers::server_for(&app, on.as_deref()).await?;
                let mc = if collection { s.collection_items(&rk).await? } else { s.children(&rk).await? };
                let mut items = mc.metadata;
                servers::tag(&app, &mut items, on.as_deref());
                Ok::<_, anyhow::Error>(items)
            })
            .await;
            this.update(cx, |c, cx| {
                // Keep the old list if a reload fails.
                match (result, &c.items) {
                    (Ok(v), _) => c.items = Load::Ready(v),
                    (Err(_), Load::Ready(_)) => {}
                    (Err(e), _) => c.items = Load::Failed(message(&e)),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn editable(&self, cx: &App) -> bool {
        self.item.kind() == "collection" && store(cx).read(cx).owned() && self.item.server_id().is_none()
    }

    fn remove(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(list) = self.items.ready_mut() else { return };
        if index >= list.len() {
            return;
        }
        // Optimistically, until the refetch lands.
        let m = list.remove(index);
        cx.notify();
        let (app, id, rk) = (core(cx), self.item.rating_key().to_string(), m.rating_key().to_string());
        store::act(cx, async move { Ok::<_, anyhow::Error>(app.server()?.remove_from_collection(&id, &rk).await?) }, Some("Removed from collection".into()), true);
    }

    fn reorder(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        let Some(list) = self.items.ready_mut() else { return };
        if from == to || from >= list.len() || to >= list.len() {
            return;
        }
        let next = moved(list, from, to);
        let rk = next[to].rating_key().to_string();
        let after = (to > 0).then(|| next[to - 1].rating_key().to_string());
        *list = next;
        cx.notify();
        let (app, id) = (core(cx), self.item.rating_key().to_string());
        store::act(cx, async move { Ok::<_, anyhow::Error>(app.server()?.move_collection_item(&id, &rk, after.as_deref()).await?) }, None, true);
    }

    fn episodes(&self, list: &[Metadata], cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let tint = store(cx).read(cx).tint();
        let spoilers = store(cx).read(cx).spoilers();
        v_flex()
            .child(widgets::section_head("Episodes", None))
            .child(widgets::rows(cx).children(list.iter().enumerate().map(|(i, ep)| {
                let spoiler = spoilers && !ep.is_watched();
                let group: SharedString = format!("ep-{i}").into();
                let (open, play, menu) = (ep.clone(), ep.clone(), ep.clone());
                let meta = [ep.originally_available_at.as_deref().map(format::air_date), ep.duration.filter(|d| *d > 0).map(format::duration)]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" · ");
                h_flex()
                    .id(("episode", i))
                    .group(group.clone())
                    .items_start()
                    .gap_5()
                    .p_4()
                    .cursor_pointer()
                    .when(i > 0, |d| d.border_t_1().border_color(theme.border))
                    .hover(|s| s.bg(theme.muted.opacity(0.5)))
                    .on_click(move |_, _, cx| nav::push(Route::for_item(&open), cx))
                    .context_menu(move |m, window, cx| item_menu(m, &menu, window, cx))
                    .child(div().w(px(28.)).flex_none().pt_0p5().text_right().text_sm().text_color(theme.muted_foreground).child(ep.index.map(|n| n.to_string()).unwrap_or_default()))
                    .child(
                        div()
                            .relative()
                            .flex_none()
                            .w(px(220.))
                            .h(px(124.))
                            .rounded(px(8.))
                            .overflow_hidden()
                            .bg(theme.muted)
                            .border_1()
                            .border_color(theme.foreground.opacity(0.1))
                            .children(art_of(ep.thumb.as_deref(), 220., 124., ep.server_id()).map(|i| i.size_full().object_fit(ObjectFit::Cover).when(spoiler, |i| i.opacity(0.15))))
                            .when_some(ep.progress(), |d, p| {
                                d.child(div().absolute().bottom_0().left_0().right_0().h(px(3.)).bg(hsla(0., 0., 0., 0.55)).child(div().h_full().w(relative(p as f32)).bg(tint)))
                            })
                            .child(
                                div().absolute().bottom(px(10.)).left_2().invisible().group_hover(group.clone(), |s| s.visible()).child(
                                    Button::new(("play", i)).primary().small().rounded_full().icon(IconName::Play).tooltip("Play").on_click(move |_, _, cx| {
                                        cx.stop_propagation();
                                        actions::play(&play, cx);
                                    }),
                                ),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_sm().font_medium().child(ep.title().to_string()))
                            .child(
                                h_flex()
                                    .mt_1()
                                    .gap_1()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(meta)
                                    .when(ep.is_watched(), |d| d.child(div().text_color(tint).child("· Watched"))),
                            )
                            .when_some(ep.summary.clone().filter(|s| !s.is_empty()), |d, s| {
                                d.child(
                                    div()
                                        .mt_2()
                                        .max_w(px(640.))
                                        .text_sm()
                                        .line_clamp(3)
                                        .text_color(if spoiler { theme.muted_foreground.opacity(0.12) } else { theme.muted_foreground })
                                        .when(spoiler, |d| d.hover(|s| s.text_color(theme.muted_foreground)))
                                        .child(s),
                                )
                            }),
                    )
            })))
            .into_any_element()
    }

    fn tracks(&self, list: &[Metadata], cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        v_flex()
            .child(widgets::section_head("Tracks", None))
            .child(widgets::rows(cx).children(list.iter().enumerate().map(|(i, t)| {
                let (play, menu) = (t.clone(), t.clone());
                widgets::list_row(i == 0, cx)
                    .id(("track", i))
                    .px_3()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.muted.opacity(0.5)))
                    .on_click(move |_, _, cx| actions::play(&play, cx))
                    .context_menu(move |m, window, cx| item_menu(m, &menu, window, cx))
                    .child(div().w(px(28.)).flex_none().text_right().text_sm().text_color(theme.muted_foreground).child(t.index.map(|n| n.to_string()).unwrap_or_default()))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().truncate().child(t.title().to_string()))
                            .when_some(t.original_title.clone(), |d, o| d.child(div().text_xs().truncate().text_color(theme.muted_foreground).child(o))),
                    )
                    .child(div().text_sm().text_color(theme.muted_foreground).child(t.duration.filter(|d| *d > 0).map(format::clock).unwrap_or_default()))
            })))
            .into_any_element()
    }

    fn collection(&self, list: &[Metadata], cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let tint = store(cx).read(cx).tint();
        let width = store(cx).read(cx).poster_width();
        let editable = self.editable(cx);
        let reorderable = editable && self.sort.as_deref() == Some("2") && list.len() > 1;
        let n = list.len();
        v_flex()
            .child(widgets::section_head(
                format!("{n} item{}", if n == 1 { "" } else { "s" }),
                reorderable.then(|| div().text_xs().text_color(theme.muted_foreground).child("Drag to reorder").into_any_element()),
            ))
            .child(grid().children(list.iter().enumerate().map(|(i, m)| {
                let group: SharedString = format!("cell-{i}").into();
                let title: SharedString = m.title().to_string().into();
                div()
                    .id(("cell", i))
                    .group(group.clone())
                    .relative()
                    .w(px(width))
                    .flex_none()
                    .child(Card::new(ElementId::Name(format!("child:{}:{i}", m.rating_key()).into()), m.clone()).width(width))
                    .when(editable, |d| {
                        d.child(
                            div().absolute().top(px(6.)).right(px(6.)).invisible().group_hover(group.clone(), |s| s.visible()).child(
                                Button::new(("remove", i)).ghost().xsmall().icon(IconName::X).bg(theme.background.opacity(0.8)).tooltip("Remove from collection").on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.remove(i, cx);
                                    },
                                )),
                            ),
                        )
                    })
                    .when(reorderable, |d| {
                        d.on_drag(Dragged(i), move |_, _, _, cx| cx.new(|_| DragLabel(title.clone())))
                            .drag_over::<Dragged>(move |s, _, _, _| s.border_l_2().border_color(tint))
                            .on_drop(cx.listener(move |this, d: &Dragged, _, cx| this.reorder(d.0, i, cx)))
                    })
            })))
            .into_any_element()
    }

    fn cards(&self, title: String, list: &[Metadata], cx: &mut Context<Self>) -> AnyElement {
        let width = store(cx).read(cx).poster_width();
        v_flex()
            .child(widgets::section_head(title, None))
            .child(grid().children(list.iter().enumerate().map(|(i, m)| Card::new(ElementId::Name(format!("child:{}:{i}", m.rating_key()).into()), m.clone()).width(width))))
            .into_any_element()
    }
}

impl Render for Children {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let list = match &self.items {
            Load::Loading => return widgets::loading().into_any_element(),
            Load::Failed(e) => {
                return widgets::failed(e.clone(), cx).child(Button::new("retry").outline().small().mt_3().label("Try again").on_click(cx.listener(|this, _, _, cx| this.load(cx)))).into_any_element();
            }
            Load::Ready(l) if l.is_empty() => return div().into_any_element(),
            Load::Ready(l) => l.clone(),
        };
        match self.item.kind() {
            "show" => CardRow::new("seasons", "Seasons", list).into_any_element(),
            "season" => self.episodes(&list, cx),
            "album" => self.tracks(&list, cx),
            "collection" => self.collection(&list, cx),
            "artist" => self.cards("Albums".into(), &list, cx),
            _ => {
                let n = list.len();
                self.cards(format!("{n} items"), &list, cx)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    #[test]
    fn moving_entries() {
        assert_eq!(moved(&[1, 2, 3, 4], 0, 2), vec![2, 3, 1, 4]);
        assert_eq!(moved(&[1, 2, 3, 4], 3, 0), vec![4, 1, 2, 3]);
        assert_eq!(moved(&[1, 2, 3], 1, 1), vec![1, 2, 3]);
        assert_eq!(moved(&[1, 2, 3], 5, 0), vec![1, 2, 3]);
    }
}
