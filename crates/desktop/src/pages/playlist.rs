//! One playlist: its header and actions, and its entries, which can be dragged
//! into a new order (unless it is a smart playlist).

use crate::prelude::*;
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _};
use patina_core::ops::lists;
use patina_core::undo;
use patina_plex::Metadata;

use super::item::{DragLabel, Dragged, moved};
use crate::actions::{self, PlayOptions};
use crate::art::art_of;
use crate::core::{Load, bg, core};
use crate::dialogs::{self, form};
use crate::format;
use crate::menu::item_menu;
use crate::nav::{self, Route};
use crate::store::{self, store};
use crate::widgets;

/// `n` with thousands separators, like `toLocaleString()`.
fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// "12 items · 3h 4m", or "Showing the first 5,000 of 6,200 items · …" when loading was cut short.
fn summary_line(shown: usize, total: usize, ms: i64) -> String {
    let count = if total > shown { format!("Showing the first {} of {} items", thousands(shown), thousands(total)) } else { format!("{shown} items") };
    format!("{count} · {}", format::duration(ms))
}

/// The entry id to place a moved entry after, once `from` moved to `to`.
fn after_of(list: &[Metadata], to: usize) -> Option<i64> {
    to.checked_sub(1).and_then(|i| list.get(i)).and_then(|m| m.playlist_item_id)
}

/// A loaded playlist (`lists::PlaylistView`, which is not `Clone`).
#[derive(Clone)]
struct PlaylistView {
    playlist: Metadata,
    items: Vec<Metadata>,
    total: usize,
}

pub struct Playlist {
    id: String,
    data: Load<Option<PlaylistView>>,
    stale: Option<PlaylistView>,
    revision: u64,
    _subs: Vec<Subscription>,
}

impl Playlist {
    pub fn new(id: String, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subs = vec![cx.observe(&store(cx), |this: &mut Self, store, cx| {
            let s = store.read(cx);
            if s.revision != this.revision && s.affects_item(&this.id, this.revision) {
                this.load(cx);
            }
        })];
        let mut this = Self { id, data: Load::Loading, stale: None, revision: 0, _subs: subs };
        this.load(cx);
        this
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        self.revision = store(cx).read(cx).revision;
        let (app, id) = (core(cx), self.id.clone());
        cx.spawn(async move |this, cx| {
            let result = bg(async move { lists::playlist(&app, &id).await }).await.map(|v| v.map(|v| PlaylistView { playlist: v.playlist, items: v.items, total: v.total }));
            this.update(cx, |p, cx| {
                p.data = Load::from_result(result);
                if let Load::Ready(Some(v)) = &p.data {
                    p.stale = Some(v.clone());
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn view_mut(&mut self) -> Option<&mut PlaylistView> {
        match &mut self.data {
            Load::Ready(Some(v)) => Some(v),
            _ => self.stale.as_mut(),
        }
    }

    fn reorder(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        let Some(view) = self.view_mut() else { return };
        if from == to || from >= view.items.len() || to >= view.items.len() {
            return;
        }
        let next = moved(&view.items, from, to);
        let Some(item_id) = next[to].playlist_item_id else { return };
        let after = after_of(&next, to);
        // Optimistically, until the reload lands.
        view.items = next;
        cx.notify();
        let (app, id) = (core(cx), self.id.clone());
        let me = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = bg(async move { Ok::<_, anyhow::Error>(app.server()?.move_playlist_item(&id, item_id, after).await?) }).await;
            cx.update(|cx| {
                if let Err(e) = result {
                    store::error(cx, crate::core::message(&e));
                }
                me.update(cx, |p, cx| p.load(cx)).ok();
            });
        })
        .detach();
    }

    fn clear(&self, window: &mut Window, cx: &mut Context<Self>) {
        let (id, me) = (self.id.clone(), cx.entity().downgrade());
        widgets::confirm("Clear this playlist?", Some("Every item is removed; the playlist itself stays.".into()), "Clear", true, window, cx, move |_, cx| {
            let (app, id, me) = (core(cx), id.clone(), me.clone());
            store::act_then(cx, async move { Ok::<_, anyhow::Error>(app.server()?.clear_playlist(&id).await?) }, move |(), cx| {
                store::toast(cx, "Playlist cleared");
                me.update(cx, |p, cx| p.load(cx)).ok();
            });
        });
    }

    fn delete(&self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.id.clone();
        widgets::confirm(
            "Delete this playlist?",
            Some("The playlist is removed from the server. Its items are not affected.".into()),
            "Delete",
            true,
            window,
            cx,
            move |_, cx| {
                let (app, id) = (core(cx), id.clone());
                store::act_then(cx, async move { Ok::<_, anyhow::Error>(app.server()?.delete_playlist(&id).await?) }, |(), cx| {
                    store::toast(cx, "Playlist deleted");
                    nav::nav(cx).update(cx, |n, cx| n.replace(Route::Playlists, cx));
                });
            },
        );
    }

    fn header(&self, view: &PlaylistView, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let p = &view.playlist;
        let items = &view.items;
        let total: i64 = items.iter().filter_map(|m| m.duration).sum();
        let empty = items.is_empty();
        let (play, shuffle, edit, keep) = (p.clone(), p.clone(), p.clone(), p.clone());
        let me = cx.entity().downgrade();
        h_flex()
            .flex_none()
            .mb_9()
            .items_end()
            .gap_8()
            .child(
                div()
                    .flex_none()
                    .size(px(200.))
                    .rounded(px(12.))
                    .overflow_hidden()
                    .bg(theme.muted)
                    .border_1()
                    .border_color(theme.border)
                    .shadow_xl()
                    .children(art_of(p.composite.as_deref().or(p.thumb.as_deref()), 480., 480., p.server_id()).map(|i| i.size_full().object_fit(ObjectFit::Cover))),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(h_flex().child(form::badge(if p.smart { "Smart playlist" } else { "Playlist" }, None, cx)))
                    .child(div().mt_3().mb_2().text_size(px(36.)).line_height(px(39.)).font_semibold().child(p.title().to_string()))
                    .child(div().text_sm().text_color(theme.muted_foreground).child(summary_line(items.len(), view.total, total)))
                    .when_some(p.summary.clone().filter(|s| !s.is_empty()), |d, s| d.child(div().mt_2p5().max_w(px(640.)).text_color(theme.muted_foreground).child(s)))
                    .child(
                        h_flex()
                            .mt_5()
                            .flex_wrap()
                            .items_center()
                            .gap_2()
                            .child(Button::new("play").primary().large().icon(IconName::Play).label("Play").disabled(empty).on_click(move |_, _, cx| actions::play(&play, cx)))
                            .child(Button::new("shuffle").outline().large().icon(IconName::Shuffle).label("Shuffle").disabled(empty).on_click(move |_, _, cx| {
                                actions::play_with(&shuffle, PlayOptions { shuffle: true, ..Default::default() }, cx)
                            }))
                            .child(Button::new("edit").ghost().large().label("Edit").on_click(move |_, window, cx| EditPlaylist::open(&edit, me.clone(), window, cx)))
                            .when(!empty, |d| d.child(Button::new("keep").ghost().large().label("Keep downloaded").on_click(move |_, window, cx| dialogs::keep::open(keep.clone(), window, cx))))
                            .when(!p.smart && !empty, |d| d.child(Button::new("clear").ghost().large().label("Clear").on_click(cx.listener(|this, _, window, cx| this.clear(window, cx)))))
                            .child(Button::new("delete").danger().large().label("Delete").on_click(cx.listener(|this, _, window, cx| this.delete(window, cx)))),
                    ),
            )
            .into_any_element()
    }

    fn entries(&self, view: &PlaylistView, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let tint = store(cx).read(cx).tint();
        let smart = view.playlist.smart;
        let reorderable = !smart;
        widgets::rows(cx)
            .children(view.items.iter().enumerate().map(|(i, m)| {
                let group: SharedString = format!("entry-{i}").into();
                let (open, play, next, menu, more) = (m.clone(), m.clone(), m.clone(), m.clone(), m.clone());
                let title: SharedString = m.full_title().into();
                let entry_id = m.playlist_item_id.filter(|_| !smart);
                let id = self.id.clone();
                // The grip replaces the number on hover. Fade rather than hide: a hover style that
                // changes layout makes GPUI paint an element it never laid out, and panic.
                let number = div()
                    .relative()
                    .w(px(28.))
                    .flex_none()
                    .flex()
                    .justify_end()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(div().when(reorderable, |d| d.group_hover(group.clone(), |s| s.opacity(0.))).child((i + 1).to_string()))
                    .when(reorderable, |d| {
                        d.child(
                            div()
                                .absolute()
                                .top_0()
                                .right_0()
                                .h_full()
                                .flex()
                                .items_center()
                                .opacity(0.)
                                .cursor_grab()
                                .group_hover(group.clone(), |s| s.opacity(1.))
                                .child(Icon::new(IconName::GripVertical).size_3p5()),
                        )
                    });
                widgets::list_row(i == 0, cx)
                    .id(("entry", i))
                    .group(group.clone())
                    .px_3()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.muted.opacity(0.5)))
                    .on_click(move |_, _, cx| nav::push(Route::for_item(&open), cx))
                    .when(reorderable, |d| {
                        d.on_drag(Dragged(i), move |_, _, _, cx| cx.new(|_| DragLabel(title.clone())))
                            .drag_over::<Dragged>(move |s, _, _, _| s.border_t_2().border_color(tint))
                            .on_drop(cx.listener(move |this, d: &Dragged, _, cx| this.reorder(d.0, i, cx)))
                    })
                    .child(number)
                    .child(
                        div()
                            .flex_none()
                            .w(px(32.))
                            .h(px(48.))
                            .rounded(px(4.))
                            .overflow_hidden()
                            .bg(theme.muted)
                            .children(art_of(m.poster(), 96., 144., m.server_id()).map(|i| i.size_full().object_fit(ObjectFit::Cover))),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().truncate().text_sm().font_medium().child(m.full_title()))
                            .child(div().truncate().text_xs().text_color(theme.muted_foreground).child(format::subtitle(m))),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .gap_0p5()
                            .items_center()
                            .text_color(theme.muted_foreground)
                            .when_some(m.duration.filter(|d| *d > 0), |d, ms| d.child(div().mr_2().text_xs().child(format::duration(ms))))
                            .child(widgets::icon_button(("play", i), IconName::Play, "Play").on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                actions::play(&play, cx);
                            }))
                            .child(widgets::icon_button(("next", i), IconName::ListStart, "Play next").on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                actions::enqueue(&next, true, cx);
                            }))
                            .when_some(entry_id, |d, entry| {
                                d.child(widgets::icon_button(("remove", i), IconName::X, "Remove from playlist").on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    let (app, id) = (core(cx), id.clone());
                                    store::act(cx, async move { undo::remove_from_playlist(&app, &id, entry).await }, None, true);
                                }))
                            })
                            .child(widgets::icon_button(("more", i), IconName::Ellipsis, "More").dropdown_menu(move |menu_, window, cx| item_menu(menu_, &more, window, cx))),
                    )
                    .context_menu(move |menu_, window, cx| item_menu(menu_, &menu, window, cx))
            }))
            .into_any_element()
    }
}

impl Render for Playlist {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let retry = |cx: &mut Context<Self>| Button::new("retry").outline().small().mt_3().label("Try again").on_click(cx.listener(|this, _, _, cx| this.load(cx)));
        let view = match (&self.data, &self.stale) {
            (Load::Ready(Some(v)), _) => v.clone(),
            (Load::Ready(None), _) => return widgets::page("playlist").child(widgets::empty("Playlist not found", None, cx)),
            (_, Some(v)) => v.clone(),
            (Load::Failed(e), None) => return widgets::page("playlist").child(widgets::failed(e.clone(), cx).child(retry(cx))),
            (Load::Loading, None) => return widgets::page("playlist").child(widgets::loading()),
        };
        let header = self.header(&view, cx);
        let mut page = widgets::page("playlist");
        if let Load::Failed(e) = &self.data {
            page = page.child(h_flex().mb_4().gap_3().items_center().text_sm().text_color(theme.danger).child(format!("Could not refresh: {e}")).child(retry(cx)));
        }
        page = page.child(header);
        if view.items.is_empty() {
            page.child(widgets::empty("This playlist is empty", None, cx))
        } else {
            let rows = self.entries(&view, cx);
            page.child(rows)
        }
    }
}

/// Rename a playlist and change its description.
struct EditPlaylist {
    id: String,
    title: Entity<InputState>,
    summary: Entity<TextareaState>,
    page: WeakEntity<Playlist>,
    _subs: Vec<Subscription>,
}

impl form::Modal for EditPlaylist {}

impl EditPlaylist {
    fn open(p: &Metadata, page: WeakEntity<Playlist>, window: &mut Window, cx: &mut App) {
        let (id, title, summary) = (p.rating_key().to_string(), p.title.clone().unwrap_or_default(), p.summary.clone().unwrap_or_default());
        let view = cx.new(|cx| {
            let title = cx.new(|cx| InputState::new(window, cx).default_value(title));
            let summary = cx.new(|cx| TextareaState::new(window, cx).auto_grow(4, 10).default_value(summary));
            title.update(cx, |s, cx| s.focus(window, cx));
            let subs = vec![
                cx.observe(&title, |_, _, cx| cx.notify()),
                cx.subscribe_in(&title, window, |this: &mut Self, _, e: &InputEvent, window, cx| {
                    if let InputEvent::PressEnter { .. } = e {
                        this.save(window, cx);
                    }
                }),
            ];
            Self { id, title, summary, page, _subs: subs }
        });
        form::show(view, "Edit playlist", form::NARROW, window, cx);
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = self.title.read(cx).value().trim().to_string();
        if title.is_empty() {
            return;
        }
        let summary = self.summary.read(cx).value().trim().to_string();
        window.close_dialog(cx);
        let (app, id, page) = (core(cx), self.id.clone(), self.page.clone());
        store::act_then(cx, async move { Ok::<_, anyhow::Error>(app.server()?.update_playlist(&id, Some(&title), Some(&summary)).await?) }, move |(), cx| {
            store::toast(cx, "Playlist updated");
            page.update(cx, |p, cx| p.load(cx)).ok();
        });
    }
}

impl Render for EditPlaylist {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let blank = self.title.read(cx).value().trim().is_empty();
        v_flex()
            .child(form::field("Title", Input::new(&self.title), cx))
            .child(div().mt_4().child(form::field("Description", Textarea::new(&self.summary), cx)))
            .child(
                form::foot(cx)
                    .child(Button::new("cancel").ghost().label("Cancel").on_click(|_, window, cx| window.close_dialog(cx)))
                    .child(Button::new("save").primary().label("Save").disabled(blank).on_click(cx.listener(|this, _, window, cx| this.save(window, cx)))),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    #[test]
    fn summary_lines() {
        assert_eq!(summary_line(3, 3, 3_600_000 + 4 * 60_000), "3 items · 1h 4m");
        assert_eq!(summary_line(5000, 6200, 0).split(" · ").next(), Some("Showing the first 5,000 of 6,200 items"));
    }

    #[test]
    fn moved_entries_go_after_their_new_neighbour() {
        let list: Vec<Metadata> = serde_json::from_str(r#"[{"playlistItemID":1},{"playlistItemID":2},{"playlistItemID":3}]"#).unwrap();
        let next = moved(&list, 2, 0);
        assert_eq!(after_of(&next, 0), None);
        let next = moved(&list, 0, 2);
        assert_eq!(next[2].playlist_item_id, Some(1));
        assert_eq!(after_of(&next, 2), Some(3));
    }
}
