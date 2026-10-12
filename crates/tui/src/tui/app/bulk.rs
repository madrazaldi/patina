//! Bulk selection: mark items in a column (`tab`), then queue, download, mark
//! watched, or add all of them to a playlist or collection. The desktop app's
//! selection mode does the same (`BulkActions.svelte`).

use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use futures::StreamExt;

use crate::tui::marks::{self, BulkOp, markable};

impl App {
    fn marked(&self) -> Vec<Item> {
        self.columns.last().map(|c| c.marks.items()).unwrap_or_default()
    }

    /// Keys of selection mode, ahead of the ordinary ones: `tab` marks, and
    /// while anything is marked `M` opens the actions (otherwise it mutes) and
    /// the usual action keys (`n N w u D space C x`) apply to all of it. Returns whether the key was used.
    pub(super) fn marks_key(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let Some(col) = self.columns.last() else { return false };
        let (any, unfiltered) = (!col.marks.is_empty(), col.filter.is_empty());
        let op = match key.code {
            KeyCode::Tab => return self.toggle_mark(),
            KeyCode::Char('a') if ctrl => return self.mark_all(),
            KeyCode::Char('M') if any && !ctrl => return self.open_bulk_menu(),
            KeyCode::Esc if any && unfiltered => BulkOp::Clear,
            _ if !any || ctrl => return false,
            KeyCode::Char('n') => BulkOp::PlayNext,
            KeyCode::Char('N') => BulkOp::AddToQueue,
            KeyCode::Char('w') => BulkOp::Watched,
            KeyCode::Char('u') => BulkOp::Unwatched,
            KeyCode::Char('D') => BulkOp::Download,
            KeyCode::Char(' ') => BulkOp::Playlist,
            KeyCode::Char('C') => BulkOp::Collection,
            KeyCode::Char('x') | KeyCode::Delete => BulkOp::Remove,
            _ => return false,
        };
        self.bulk_run(op);
        true
    }

    /// `tab`: mark or unmark the selected item and move to the next.
    fn toggle_mark(&mut self) -> bool {
        let Some(col) = self.columns.last_mut() else { return true };
        let Some(item) = col.selected().cloned() else { return true };
        if !markable(&item) {
            self.info("Only library items can be marked");
            return true;
        }
        col.marks.toggle(&item);
        col.move_by(1);
        true
    }

    /// `^a`: mark everything shown. A library shown a page at a time is
    /// loaded in full first.
    fn mark_all(&mut self) -> bool {
        let Some(idx) = self.columns.len().checked_sub(1) else { return true };
        self.go_full(idx);
        let col = &mut self.columns[idx];
        if col.pager.is_some() || col.loading || !col.loaded {
            col.mark_all_on_load = true;
            self.info("Marking everything once the listing has loaded…");
            return true;
        }
        let n = col.mark_visible();
        let total = col.marks.len();
        self.info(if n == 0 { "Nothing more to mark here".to_string() } else { format!("Marked {n} · {total} in all") });
        true
    }

    pub(super) fn clear_marks(&mut self) {
        if let Some(col) = self.columns.last_mut() {
            col.marks.clear();
            col.mark_all_on_load = false;
        }
    }

    /// A listing finished loading: mark all of it if `^a` asked for that.
    pub(super) fn finish_mark_all(&mut self, idx: usize) {
        let col = &mut self.columns[idx];
        if !col.mark_all_on_load {
            return;
        }
        col.mark_all_on_load = false;
        let total = col.mark_visible();
        if idx + 1 == self.columns.len() {
            self.info(format!("Marked {total}"));
        }
    }

    /// `M`: the actions for everything marked.
    fn open_bulk_menu(&mut self) -> bool {
        let Some(col) = self.columns.last() else { return true };
        let n = col.marks.len();
        if n == 0 {
            self.info("Nothing is marked · tab marks the selected item");
            return true;
        }
        let mut rows: Vec<(MenuAction, &'static str, &'static str)> = vec![
            (MenuAction::Bulk(BulkOp::PlayNext), "Play next", "n"),
            (MenuAction::Bulk(BulkOp::AddToQueue), "Add to queue", "N"),
            (MenuAction::Bulk(BulkOp::Download), "Download for offline", "D"),
            (MenuAction::Bulk(BulkOp::Watched), "Mark watched", "w"),
            (MenuAction::Bulk(BulkOp::Unwatched), "Mark unwatched", "u"),
            (MenuAction::Bulk(BulkOp::Playlist), "Add to a playlist…", "space"),
        ];
        if self.owned() {
            rows.push((MenuAction::Bulk(BulkOp::Collection), "Add to a collection…", "C"));
        }
        if col.is_playlist().is_some() {
            rows.push((MenuAction::Bulk(BulkOp::Remove), "Remove from this playlist…", "x"));
        } else if self.owned() && col.is_collection().is_some() {
            rows.push((MenuAction::Bulk(BulkOp::Remove), "Remove from this collection…", "x"));
        }
        rows.push((MenuAction::Bulk(BulkOp::Clear), "Clear the marks", "esc"));
        let title = format!("{n} marked");
        self.modal = Some(Modal::Menu(Menu { title, rows, cursor: 0 }));
        true
    }

    pub(super) fn bulk_run(&mut self, op: BulkOp) {
        let items = self.marked();
        if items.is_empty() {
            return self.info("Nothing is marked · tab marks the selected item");
        }
        match op {
            BulkOp::PlayNext => self.bulk_queue(items, true),
            BulkOp::AddToQueue => self.bulk_queue(items, false),
            BulkOp::Download => self.bulk_download(items),
            BulkOp::Watched => self.bulk_watched(items, true),
            BulkOp::Unwatched => self.bulk_watched(items, false),
            BulkOp::Playlist => self.bulk_playlist(items),
            BulkOp::Collection => self.bulk_collection(items),
            BulkOp::Remove => self.bulk_remove(items),
            BulkOp::Clear => self.clear_marks(),
        }
    }

    fn bulk_queue(&mut self, items: Vec<Item>, next: bool) {
        let items: Vec<Item> = items.into_iter().filter(super::queue::queueable).collect();
        if items.is_empty() {
            return self.info("Nothing marked can be queued");
        }
        let n = items.len();
        let idle = self.now_playing.is_none();
        let metas: Vec<Metadata> = items.into_iter().map(|i| i.meta).collect();
        if next && !idle {
            // Each lands right after the current item, so go backwards to keep the order marked.
            for m in metas.into_iter().rev() {
                player::queue(self.core.clone(), QueueCommand::Add { item: Box::new(m), next: true });
            }
        } else if let Err(e) = player::queue_items(self.core.clone(), metas) {
            return self.error(err(e));
        }
        self.success(match (idle, next) {
            (true, _) => format!("Starting {n} items…"),
            (false, true) => format!("{n} items play next"),
            (false, false) => format!("Added {n} items to the queue"),
        });
        self.clear_marks();
    }

    fn bulk_download(&mut self, items: Vec<Item>) {
        let list: Vec<(String, String)> = items
            .iter()
            .filter(|i| i.is_playable() || matches!(i.kind(), Kind::Show | Kind::Season | Kind::Album | Kind::Artist))
            .filter_map(|i| Some((i.rating_key.clone()?, i.full_title())))
            .collect();
        if list.is_empty() {
            return self.info("Nothing marked can be downloaded");
        }
        let (core, total) = (self.core.clone(), list.len());
        self.spawn_op(Refresh::Nothing, async move {
            let (mut added, mut failures) = (0usize, Vec::new());
            for (key, title) in list {
                match downloads::enqueue(&core, &key).await {
                    Ok(n) => added += n,
                    Err(e) => failures.push(format!("{title}: {}", err(e))),
                }
            }
            let done = match added {
                0 => "Everything marked is already downloaded".to_string(),
                n => format!("Downloading {n} item{} to {}", if n == 1 { "" } else { "s" }, core.settings().player.downloads_dir().display()),
            };
            marks::outcome(&done, total, &failures)
        });
        self.clear_marks();
    }

    fn bulk_watched(&mut self, items: Vec<Item>, watched: bool) {
        let keys: Vec<(String, String)> = items.iter().filter(|i| i.is_watchable()).filter_map(|i| Some((i.rating_key.clone()?, i.title().to_string()))).collect();
        if keys.is_empty() {
            return self.info("Nothing marked can be marked watched");
        }
        for (key, _) in &keys {
            self.patch(key, &mut |i| i.set_watched(watched));
        }
        let (server, total) = (self.server(), keys.len());
        let done = format!("{total} title{} marked {}", if total == 1 { "" } else { "s" }, if watched { "watched" } else { "unwatched" });
        self.spawn_op(Refresh::WatchState, async move {
            let server = server?;
            let failures: Vec<String> = futures::stream::iter(keys)
                .map(|(key, title)| {
                    let server = server.clone();
                    async move {
                        let result = if watched { server.scrobble(&key).await } else { server.unscrobble(&key).await };
                        result.err().map(|e| format!("{title}: {}", err(e)))
                    }
                })
                .buffer_unordered(4)
                .filter_map(|failure| async move { failure })
                .collect()
                .await;
            marks::outcome(&done, total, &failures)
        });
        self.clear_marks();
    }

    // ---- playlists and collections ---------------------------------------

    fn bulk_playlist(&mut self, items: Vec<Item>) {
        let items: Vec<Item> = items
            .into_iter()
            .filter(|i| i.rating_key.is_some() && (i.is_playable() || matches!(i.kind(), Kind::Show | Kind::Season | Kind::Album | Kind::Artist)))
            .collect();
        let kind = match marks::playlist_kind(&items) {
            Ok(kind) => kind,
            Err(why) => return self.error(why),
        };
        let keys: Vec<String> = items.iter().filter_map(|i| i.rating_key.clone()).collect();
        let title = format!("Add {} item{} to a playlist", keys.len(), if keys.len() == 1 { "" } else { "s" });
        let server = self.server();
        self.open_picker(title, PickKind::BulkPlaylist { keys, kind }, " enter add · esc close ", async move {
            let mut rows = vec![(PickEntry::NewTarget, "+ New playlist…".to_string())];
            for p in server?.playlists(Some(kind), None).await.map_err(err)?.into_iter().filter(|p| !p.smart) {
                let count = p.leaf_count.map(|n| format!("  {n}")).unwrap_or_default();
                rows.push((PickEntry::Target(p.rating_key().to_string(), p.title().to_string()), format!("{}{count}", p.title())));
            }
            Ok(rows)
        });
    }

    fn bulk_collection(&mut self, items: Vec<Item>) {
        if !self.owned() {
            return self.info("Only the server owner can manage collections");
        }
        let items: Vec<Item> = items.into_iter().filter(|i| i.rating_key.is_some()).collect();
        let kind = match marks::collection_kind(&items) {
            Ok(kind) => kind.to_string(),
            Err(why) => return self.error(why),
        };
        let column_section = self.columns.last().and_then(|c| c.section_id()).map(str::to_string);
        let sections: HashSet<String> = items.iter().filter_map(Item::section_id).collect();
        let section_id = match (sections.len(), column_section) {
            (1, _) => sections.into_iter().next().unwrap_or_default(),
            (0, Some(section)) => section,
            _ => return self.error("Mark items of one library for a collection"),
        };
        let type_id = patina_plex::MediaType::from_name(&kind).map_or(1, patina_plex::MediaType::id);
        let keys: Vec<String> = items.iter().filter_map(|i| i.rating_key.clone()).collect();
        let title = format!("Add {} item{} to a collection", keys.len(), if keys.len() == 1 { "" } else { "s" });
        let (server, section) = (self.server(), section_id.clone());
        self.open_picker(title, PickKind::BulkCollection { keys, section_id, kind: type_id }, " enter add · esc close ", async move {
            let mut rows = vec![(PickEntry::NewTarget, "+ New collection…".to_string())];
            for c in server?.collections(&section).await.map_err(err)?.into_iter().filter(|c| !c.smart) {
                rows.push((PickEntry::Target(c.rating_key().to_string(), c.title().to_string()), c.title().to_string()));
            }
            Ok(rows)
        });
    }

    /// A playlist or collection (or "new") was chosen for the marked items.
    pub(super) fn pick_bulk_target(&mut self, kind: PickKind, entry: PickEntry) {
        match (kind, entry) {
            (PickKind::BulkPlaylist { keys, .. }, PickEntry::Target(id, title)) => {
                let done = format!("Added {} item{} to {title}", keys.len(), if keys.len() == 1 { "" } else { "s" });
                self.server_op(Refresh::Playlists, done, move |s| async move {
                    let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
                    s.add_to_playlist(&id, &s.item_uri(&refs)).await
                });
            }
            (PickKind::BulkPlaylist { keys, kind }, PickEntry::NewTarget) => {
                self.modal = Some(Modal::Input(Input {
                    title: format!("New playlist · {} items", keys.len()),
                    hint: "Name".into(),
                    value: String::new(),
                    purpose: InputPurpose::NewPlaylist { rating_keys: keys, kind },
                }));
            }
            (PickKind::BulkCollection { keys, .. }, PickEntry::Target(id, title)) => {
                let done = format!("Added {} item{} to {title}", keys.len(), if keys.len() == 1 { "" } else { "s" });
                self.server_op(Refresh::Nothing, done, move |s| async move {
                    let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
                    s.add_to_collection(&id, &s.item_uri(&refs)).await
                });
            }
            (PickKind::BulkCollection { keys, section_id, kind }, PickEntry::NewTarget) => {
                self.modal = Some(Modal::Input(Input {
                    title: format!("New collection · {} items", keys.len()),
                    hint: "Name".into(),
                    value: String::new(),
                    purpose: InputPurpose::NewCollectionWith { section_id, kind, rating_keys: keys },
                }));
            }
            _ => return,
        }
        // The choice is made; the marks have done their job.
        self.clear_marks();
    }

    pub(super) fn create_collection_with(&mut self, name: String, section_id: String, kind: u32, rating_keys: Vec<String>) {
        let (server, n) = (self.server(), rating_keys.len());
        self.spawn_op(Refresh::Nothing, async move {
            let server = server?;
            let refs: Vec<&str> = rating_keys.iter().map(String::as_str).collect();
            server.create_collection(&section_id, &name, kind, Some(&server.item_uri(&refs)), false).await.map_err(err)?;
            Ok(Some(format!("Created collection {name} with {n} item{}", if n == 1 { "" } else { "s" })))
        });
        self.clear_marks();
    }

    // ---- removing from the playlist or collection being shown -------------

    fn bulk_remove(&mut self, items: Vec<Item>) {
        let owned = self.owned();
        let Some(col) = self.columns.last() else { return };
        let n = items.len();
        let confirm = if let Some(playlist) = col.is_playlist() {
            let entries: Vec<i64> = items.iter().filter_map(|i| i.playlist_item_id).collect();
            Some((entries.len(), ConfirmAction::RemoveMarkedFromPlaylist { playlist_id: playlist.to_string(), entries }))
        } else if let Some(collection) = col.is_collection().filter(|_| owned) {
            let rating_keys: Vec<String> = items.iter().filter_map(|i| i.rating_key.clone()).collect();
            Some((rating_keys.len(), ConfirmAction::RemoveMarkedFromCollection { collection_id: collection.to_string(), rating_keys }))
        } else {
            None
        };
        let title = col.title.clone();
        match confirm {
            Some((0, _)) => self.info("Nothing marked can be removed here"),
            Some((count, action)) => {
                let plural = if count == 1 { "" } else { "s" };
                let message = if count == n {
                    format!("Remove {count} marked item{plural} from {title}?")
                } else {
                    format!("Remove {count} of the {n} marked items from {title}? The rest can't be removed here.")
                };
                self.modal = Some(Modal::Confirm(Confirm { message, action }));
            }
            None => self.info("x removes marked items inside a playlist or collection"),
        }
    }

    pub(super) fn run_bulk_remove(&mut self, action: ConfirmAction) {
        let (refresh, count, work): (Refresh, usize, _) = match action {
            ConfirmAction::RemoveMarkedFromPlaylist { playlist_id, entries } => {
                let refresh = Refresh::Paths(vec![format!("/playlists/{playlist_id}/items"), PLAYLISTS.into()]);
                let count = entries.len();
                (refresh, count, Removal::Playlist(playlist_id, entries))
            }
            ConfirmAction::RemoveMarkedFromCollection { collection_id, rating_keys } => {
                let refresh = Refresh::Paths(vec![format!("/library/collections/{collection_id}/children")]);
                let count = rating_keys.len();
                (refresh, count, Removal::Collection(collection_id, rating_keys))
            }
            _ => return,
        };
        let server = self.server();
        self.spawn_op(refresh, async move {
            let server = server?;
            let mut failures = Vec::new();
            match work {
                Removal::Playlist(id, entries) => {
                    for entry in entries {
                        if let Err(e) = server.remove_from_playlist(&id, entry).await {
                            failures.push(err(e));
                        }
                    }
                }
                Removal::Collection(id, keys) => {
                    for key in keys {
                        if let Err(e) = server.remove_from_collection(&id, &key).await {
                            failures.push(err(e));
                        }
                    }
                }
            }
            marks::outcome(&format!("Removed {count} item{}", if count == 1 { "" } else { "s" }), count, &failures)
        });
        self.clear_marks();
    }
}

/// What a confirmed bulk removal does.
enum Removal {
    Playlist(String, Vec<i64>),
    Collection(String, Vec<String>),
}
