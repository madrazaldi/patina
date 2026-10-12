use super::*;
use futures::StreamExt;
use patina_core::ops::edit;
use patina_core::session;

use crate::tui::column::SortKey;
use crate::tui::source::section_types;

impl App {
    pub(super) fn set_watched(&mut self, watched: bool) {
        let Some(item) = self.selected().cloned() else { return };
        if !item.is_watchable() {
            self.info("That can't be marked watched");
            return;
        }
        let key = item.rating_key.clone().unwrap_or_default();
        let server = item.server_id().map(str::to_string);
        if server.is_none() {
            self.patch(&key, &mut |i| i.set_watched(watched));
        }
        // Core announces it (with Undo) and refreshes the views.
        self.mark_watched(vec![key], server, watched);
    }

    /// Forget the resume position without changing the watched state.
    pub(super) fn clear_progress(&mut self) {
        let Some(item) = self.selected().cloned() else { return };
        let Some(key) = item.rating_key.clone().filter(|_| item.progress().is_some()) else { return };
        self.patch(&key, &mut |i| i.view_offset = None);
        self.forget_progress(key);
    }

    pub(super) fn submit_rating(&mut self, key: String, value: &str) {
        let rating = if value.trim().is_empty() {
            None
        } else {
            match value.trim().parse::<f64>() {
                Ok(v) if (0.0..=10.0).contains(&v) => Some(v),
                _ => {
                    self.error("Rating must be a number from 0 to 10");
                    return;
                }
            }
        };
        self.patch(&key, &mut |i| i.user_rating = rating);
        let done = rating.map_or("Rating cleared".into(), |r| format!("Rated {r}/10"));
        self.server_op(Refresh::Nothing, done, move |s| async move { s.rate(&key, rating).await });
    }

    pub(super) fn open_web(&mut self) {
        let Some(key) = self.selected().and_then(|i| i.rating_key.clone()) else { return };
        let Some(machine) = self.session.as_ref().and_then(|s| s.machine_identifier.clone()) else {
            self.info("The server's identity is unknown");
            return;
        };
        match open::that_detached(web_url(&machine, &key)) {
            Ok(()) => self.success("Opened in Plex Web"),
            Err(e) => self.error(format!("Couldn't open a browser: {e}")),
        }
    }

    pub(super) fn scan_library(&mut self, force: bool) {
        let section = self
            .selected()
            .filter(|i| i.kind() == Kind::Section)
            .and_then(|i| Some((i.key.clone()?, i.title().to_string())))
            .or_else(|| {
                let sid = self.columns.last()?.section_id()?.to_string();
                let title = self.sections.iter().find(|s| s.key.as_deref() == Some(&sid))?.title().to_string();
                Some((sid, title))
            });
        let Some((id, title)) = section else {
            self.info("Select a library (or open one) to scan it");
            return;
        };
        let done = format!("Scanning {title}{}", if force { " (full refresh)" } else { "" });
        self.server_op(Refresh::Nothing, done, move |s| async move { s.refresh_section(&id, force, None).await });
    }

    pub(super) fn open_playlist_picker(&mut self) {
        let Some(item) = self.selected().cloned() else { return };
        if !(item.is_playable() || matches!(item.kind(), Kind::Show | Kind::Season | Kind::Album | Kind::Artist)) || item.rating_key.is_none() {
            self.info("Select something to add to a playlist");
            return;
        }
        let kind = item.playlist_kind();
        let key = item.rating_key.clone().unwrap_or_default();
        self.modal = Some(Modal::Playlists(PlaylistPicker { item, kind, rows: Vec::new(), cursor: 0, loading: true, error: None }));
        let (server, tx) = (self.server(), self.tx.clone());
        tokio::spawn(async move {
            let result = async {
                let server = server?;
                let playlists = server.playlists(Some(kind), None).await.map_err(err)?.into_iter().filter(|p| !p.smart);
                // Check membership concurrently; any failure makes the whole
                // view an error, so "unknown" is never shown as "absent".
                futures::stream::iter(playlists)
                    .map(|p| {
                        let (server, key) = (server.clone(), key.clone());
                        async move {
                            let entries = lists::playlist_entries(&server, p.rating_key()).await.map_err(err)?.0;
                            let entry_id = entries.iter().find(|e| e.rating_key.as_deref() == Some(key.as_str())).and_then(|e| e.playlist_item_id);
                            Ok::<_, String>(PlaylistRow { playlist: Item::from(p), entry_id, busy: false })
                        }
                    })
                    .buffered(4)
                    .collect::<Vec<_>>()
                    .await
                    .into_iter()
                    .collect::<Result<Vec<_>, _>>()
            }
            .await;
            let _ = tx.send(Msg::PlaylistRows(result));
        });
    }

    pub(super) fn toggle_playlist_row(&mut self) {
        let Some(Modal::Playlists(p)) = &mut self.modal else { return };
        let item_key = p.item.rating_key.clone().unwrap_or_default();
        if p.cursor == p.rows.len() {
            let kind = p.kind;
            self.modal = Some(Modal::Input(Input {
                title: "New playlist".into(),
                hint: "Name".into(),
                value: String::new(),
                purpose: InputPurpose::NewPlaylist { rating_keys: vec![item_key], kind },
            }));
            return;
        }
        let Some(row) = p.rows.get_mut(p.cursor) else { return };
        if row.busy {
            return;
        }
        row.busy = true;
        let playlist_key = row.playlist.rating_key.clone().unwrap_or_default();
        let entry = row.entry_id;
        let (server, tx) = (self.server(), self.tx.clone());
        tokio::spawn(async move {
            let result = async {
                let server = server?;
                match entry {
                    Some(entry_id) => {
                        server.remove_from_playlist(&playlist_key, entry_id).await.map_err(err)?;
                        Ok(None)
                    }
                    None => {
                        server.add_to_playlist(&playlist_key, &server.item_uri(&[&item_key])).await.map_err(err)?;
                        let entries = lists::playlist_entries(&server, &playlist_key).await.map_err(err)?.0;
                        Ok(entries.iter().rev().find(|e| e.rating_key.as_deref() == Some(item_key.as_str())).and_then(|e| e.playlist_item_id).or(Some(0)))
                    }
                }
            }
            .await;
            let _ = tx.send(Msg::PlaylistToggled { playlist_key, result });
        });
    }

    pub(super) fn submit_input(&mut self, input: Input) {
        let value = input.value.trim().to_string();
        match input.purpose {
            InputPurpose::SaveView => self.save_view(&value),
            InputPurpose::NewCollectionWith { section_id, kind, rating_keys } => {
                if !value.is_empty() {
                    self.create_collection_with(value, section_id, kind, rating_keys);
                }
            }
            InputPurpose::Rate { rating_key } => self.submit_rating(rating_key, &value),
            InputPurpose::ServerUrl => {
                if value.is_empty() {
                    return;
                }
                self.modal = Some(Modal::Input(Input {
                    title: "Connect to a server".into(),
                    hint: "X-Plex-Token".into(),
                    value: String::new(),
                    purpose: InputPurpose::ServerToken { url: value },
                }));
            }
            InputPurpose::ServerToken { url } => {
                if !value.is_empty() {
                    self.sign_in_manual(url, value);
                }
            }
            InputPurpose::NewPlaylist { rating_keys, kind } => {
                if value.is_empty() {
                    return;
                }
                let server = self.server();
                self.spawn_op(Refresh::Playlists, async move {
                    let server = server?;
                    let keys: Vec<&str> = rating_keys.iter().map(String::as_str).collect();
                    let p = server.create_playlist(&value, kind, &server.item_uri(&keys), false).await.map_err(err)?;
                    Ok(Some(format!("Created playlist {}", p.as_ref().map_or(value.as_str(), |p| p.title()))))
                });
            }
            InputPurpose::NewCollection { rating_key } => {
                if value.is_empty() {
                    return;
                }
                let core = self.core.clone();
                self.spawn_op(Refresh::Item(rating_key.clone()), async move {
                    edit::add_to_collection(&core, &rating_key, None, Some(&value)).await.map_err(err)?;
                    Ok(Some(format!("Created collection {value}")))
                });
            }
            InputPurpose::SmartPlaylist { kind, filter_path } => {
                if value.is_empty() {
                    return;
                }
                let server = self.server();
                self.spawn_op(Refresh::Playlists, async move {
                    let server = server?;
                    server.create_playlist(&value, kind, &server.library_uri(&filter_path), true).await.map_err(err)?;
                    Ok(Some(format!("Created smart playlist {value}")))
                });
            }
            InputPurpose::EditField { edit, field } => self.submit_edit(*edit, field, value),
            InputPurpose::ArtworkUrl { rating_key, art } => {
                if !value.starts_with("http") {
                    self.error("Enter an http(s) image URL");
                    return;
                }
                self.set_artwork(rating_key, art, value);
            }
            InputPurpose::PlaylistSummary { playlist_id } => {
                self.server_op(Refresh::Playlists, "Playlist updated", move |s| async move { s.update_playlist(&playlist_id, None, Some(&value)).await });
            }
            InputPurpose::MatchQuery { rating_key, title, year } => self.search_matches(rating_key, title, year, Some(value)),
            InputPurpose::SubtitleLanguage { rating_key } => {
                let lang = if value.is_empty() { "en".to_string() } else { value };
                self.search_subtitles(rating_key, lang);
            }
            InputPurpose::UserPin { user } => self.switch_user(*user, Some(value)),
            InputPurpose::Login => {
                if !value.is_empty() {
                    self.modal = Some(Modal::Input(Input {
                        title: "Sign in with Plex".into(),
                        hint: "Password".into(),
                        value: String::new(),
                        purpose: InputPurpose::Password { login: value },
                    }));
                }
            }
            InputPurpose::Password { login } => {
                if !input.value.is_empty() {
                    self.modal = Some(Modal::Input(Input {
                        title: "Sign in with Plex".into(),
                        hint: "Two-factor code (if enabled; else enter)".into(),
                        value: String::new(),
                        purpose: InputPurpose::TwoFactor { login, password: input.value },
                    }));
                }
            }
            InputPurpose::TwoFactor { login, password } => self.password_sign_in(login, password, value),
            InputPurpose::Setting(key) => self.save_setting(key, Some(value)),
            InputPurpose::Pick(field) => self.pick_submit(field, &value),
            InputPurpose::Auto { field, show } => self.save_auto(field, show, value),
            InputPurpose::KeepNext { rating_key, title, delete_watched } => self.save_keep(rating_key, title, delete_watched, &value),
            InputPurpose::KeepPolicy { id } => self.save_keep_policy(&id, &value),
            InputPurpose::Budget => self.save_budget(&value),
            InputPurpose::Pref { id, section } => self.save_pref(id, section, value),
            InputPurpose::LibraryName(mut form) => {
                form.name = value;
                self.library_form_step(*form, "folders");
            }
            InputPurpose::LibraryFolders(mut form) => {
                form.locations = value.split(';').map(|f| f.trim().to_string()).filter(|f| !f.is_empty()).collect();
                if form.locations.is_empty() {
                    return self.error("A library needs at least one folder");
                }
                self.library_form_step(*form, "agent");
            }
            InputPurpose::LibraryLanguage(mut form) => {
                form.language = value;
                self.library_form_step(*form, "create");
            }
            InputPurpose::EditLibraryName { id, folders } => {
                if value.is_empty() {
                    return;
                }
                self.modal = Some(Modal::Input(Input {
                    title: format!("Edit {value} · folders"),
                    hint: "Folders on the server, separated by ;".into(),
                    value: folders,
                    purpose: InputPurpose::EditLibraryFolders { id, name: value },
                }));
            }
            InputPurpose::EditLibraryFolders { id, name } => self.edit_library(id, name, &value),
            InputPurpose::RenamePlaylist { playlist_id } => {
                if value.is_empty() {
                    return;
                }
                let done = format!("Renamed to {value}");
                self.server_op(Refresh::Playlists, done, move |s| async move { s.update_playlist(&playlist_id, Some(&value), None).await });
            }
        }
    }

    pub(super) fn run_confirmed(&mut self, action: ConfirmAction) {
        match action {
            ConfirmAction::Logout => session::sign_out(&self.core),
            action @ (ConfirmAction::RemoveMarkedFromPlaylist { .. } | ConfirmAction::RemoveMarkedFromCollection { .. }) => self.run_bulk_remove(action),
            ConfirmAction::ResumeQueue => self.resume_queue(),
            ConfirmAction::DeletePlaylist { playlist_id } => {
                self.server_op(Refresh::Playlists, "Playlist deleted", move |s| async move { s.delete_playlist(&playlist_id).await })
            }
            ConfirmAction::RemoveFromPlaylist { playlist_id, playlist_item_id } => self.remove_playlist_entry(playlist_id, playlist_item_id),
            ConfirmAction::RemoveFromCollection { collection_id, rating_key } => {
                let path = format!("/library/collections/{collection_id}/children");
                self.server_op(Refresh::Paths(vec![path]), "Removed from collection", move |s| async move {
                    s.remove_from_collection(&collection_id, &rating_key).await
                })
            }
            ConfirmAction::TerminateSession { session_id } => self.server_op(Refresh::Source(Source::Sessions), "Stream stopped", move |s| async move {
                s.terminate_session(&session_id, "Stopped by the server owner").await
            }),
            ConfirmAction::Unmatch { rating_key } => {
                self.server_op(Refresh::Item(rating_key.clone()), "Unmatched", move |s| async move { s.unmatch(&rating_key).await })
            }
            ConfirmAction::DeleteMedia { rating_key, title } => {
                let server = self.server();
                self.spawn_op(Refresh::Removed(rating_key.clone()), async move {
                    server?.delete_metadata(&rating_key).await.map_err(|e| match e.status() {
                        Some(401 | 403) => "The server doesn't allow media deletion (Settings → Library)".to_string(),
                        _ => err(e),
                    })?;
                    Ok(Some(format!("Deleted {title}")))
                });
            }
            ConfirmAction::DeleteCollection { rating_key, title } => {
                self.server_op(Refresh::Removed(rating_key.clone()), format!("Deleted {title}"), move |s| async move { s.delete_collection(&rating_key).await })
            }
            ConfirmAction::ClearPlaylist { playlist_id } => {
                let path = format!("/playlists/{playlist_id}/items");
                self.server_op(Refresh::Paths(vec![path, PLAYLISTS.into()]), "Playlist cleared", move |s| async move { s.clear_playlist(&playlist_id).await })
            }
            ConfirmAction::RemoveDownload { id } => downloads::remove(&self.core, &id),
            ConfirmAction::ClearAuto { show } => self.clear_auto(show),
            ConfirmAction::StopKeeping { id, delete_files } => self.stop_keeping(&id, delete_files),
            ConfirmAction::DeleteRule { id } => {
                let server = self.server();
                self.spawn_op(Refresh::Source(Source::Rules), async move {
                    patina_core::livetv::delete_rule(&server?, &id).await.map_err(err)?;
                    Ok(Some("Recording rule deleted".into()))
                });
            }
            ConfirmAction::DeleteLibrary { id, title } => {
                let core = self.core.clone();
                self.spawn_op(Refresh::Nothing, async move {
                    patina_core::ops::server::delete_library(&core, &id).await.map_err(err)?;
                    Ok(Some(format!("Deleted {title}")))
                });
            }
            ConfirmAction::DeleteHistory { key } => {
                let core = self.core.clone();
                let refresh = self.columns.last().map(|c| c.source.clone()).filter(|s| matches!(s, Source::History(_))).map_or(Refresh::Nothing, Refresh::Source);
                self.spawn_op(refresh, async move {
                    lists::delete_history(&core, &key).await.map_err(err)?;
                    Ok(Some("Deleted from history".into()))
                });
            }
            ConfirmAction::EmptyTrash { section_id } => {
                self.server_op(Refresh::Nothing, "Trash emptied", move |s| async move { s.empty_trash(&section_id).await })
            }
        }
    }

    /// `x`: the contextual destructive action.
    pub(super) fn delete_action(&mut self) {
        if let Some(item) = self.selected().cloned()
            && self.row_delete(&item)
        {
            return;
        }
        let Some(col) = self.columns.last() else { return };
        let Some(item) = col.selected().cloned() else { return };
        let action = if col.is_sessions() {
            item.session.as_ref().and_then(|s| s.id.clone()).map(|session_id| Confirm {
                message: format!(
                    "Stop {}'s stream of {}?",
                    item.user.as_ref().and_then(|u| u.title.clone()).unwrap_or_else(|| "this user".into()),
                    item.full_title()
                ),
                action: ConfirmAction::TerminateSession { session_id },
            })
        } else if let Some(pid) = col.is_playlist() {
            item.playlist_item_id.map(|entry| Confirm {
                message: format!("Remove {} from {}?", item.full_title(), col.title),
                action: ConfirmAction::RemoveFromPlaylist { playlist_id: pid.to_string(), playlist_item_id: entry },
            })
        } else if let Some(cid) = col.is_collection().filter(|_| self.owned()) {
            item.rating_key.clone().map(|rk| Confirm {
                message: format!("Remove {} from the {} collection?", item.title(), col.title),
                action: ConfirmAction::RemoveFromCollection { collection_id: cid.to_string(), rating_key: rk },
            })
        } else if matches!(col.source, Source::History(_)) {
            item.history_key.clone().filter(|_| self.owned()).map(|key| Confirm {
                message: format!("Delete {} from the watch history?", item.full_title()),
                action: ConfirmAction::DeleteHistory { key },
            })
        } else if item.kind() == Kind::Playlist {
            item.rating_key.clone().map(|id| Confirm {
                message: format!("Delete playlist {}?", item.title()),
                action: ConfirmAction::DeletePlaylist { playlist_id: id },
            })
        } else if item.kind() == Kind::Collection && self.owned() {
            item.rating_key.clone().map(|rating_key| delete_collection_confirm(rating_key, item.title()))
        } else {
            None
        };
        match action {
            Some(c) => self.modal = Some(Modal::Confirm(c)),
            None => self.info("Nothing to delete here"),
        }
    }

    /// `J`/`K`: move an entry of a playlist, or of a custom-sorted collection.
    pub(super) fn move_entry(&mut self, down: bool) {
        let owned = self.owned();
        let Some(col) = self.columns.last() else { return };
        let (playlist, collection) = (col.is_playlist().map(str::to_string), col.is_collection().map(str::to_string));
        if playlist.is_none() && collection.is_none() {
            return self.info("Reordering works inside a playlist or collection");
        }
        if let Some(pid) = &playlist
            && self.columns.iter().rev().nth(1).and_then(|c| c.items.iter().find(|i| i.rating_key.as_deref() == Some(pid.as_str()))).is_some_and(|p| p.smart)
        {
            return self.info("Smart playlists can't be reordered");
        }
        if let Some(cid) = &collection {
            let sort = self.details.get(cid).and_then(Item::collection_sort).or_else(|| {
                let parent = self.columns.iter().rev().nth(1)?;
                parent.items.iter().find(|i| i.rating_key.as_deref() == Some(cid.as_str()))?.collection_sort()
            });
            if !owned {
                return self.info("Only the server owner can reorder collections");
            }
            if sort != Some(2) {
                return self.info("Set the collection's sort to Custom to reorder it (menu → Collection sort)");
            }
        }
        let Some(col) = self.columns.last_mut() else { return };
        if !col.filter.is_empty() || col.sort != SortKey::Default || col.hide_watched {
            self.info("Clear the filter, sort and hide-watched to reorder");
            return;
        }
        let cur = col.cursor;
        let target = if down { cur + 1 } else { cur.wrapping_sub(1) };
        if target >= col.items.len() {
            return;
        }
        col.items.swap(cur, target);
        col.rebuild(&mut self.fuzzy);
        col.cursor = target;
        let path = col.path.clone().unwrap_or_default();
        let before = target.checked_sub(1).map(|i| col.items[i].meta.clone());
        let moved = col.items[target].meta.clone();
        let server = self.server();
        self.pending += 1;
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = async {
                let s = server?;
                match (playlist, collection) {
                    (Some(pid), _) => {
                        let entry = moved.playlist_item_id.ok_or("Not a playlist entry")?;
                        s.move_playlist_item(&pid, entry, before.and_then(|b| b.playlist_item_id)).await
                    }
                    (_, Some(cid)) => s.move_collection_item(&cid, moved.rating_key(), before.as_ref().map(|b| b.rating_key())).await,
                    _ => Ok(()),
                }
                .map(|_| None)
                .map_err(|e| format!("Reorder failed: {e}"))
            }
            .await;
            let refresh = if result.is_err() { Refresh::Paths(vec![path]) } else { Refresh::Nothing };
            let _ = tx.send(Msg::OpDone { result, refresh });
        });
    }

    // ---- watchlist ---------------------------------------------------------

    pub(super) fn toggle_watchlist(&mut self) {
        let Some(item) = self.selected_detail().cloned() else { return };
        if !matches!(item.kind(), Kind::Movie | Kind::Show | Kind::Remote) {
            self.info("Only movies and shows go on the watchlist");
            return;
        }
        let (Some(guid), Some(id)) = (item.guid.clone(), item.discover_id().map(str::to_string)) else {
            self.info(format!("{} isn't matched with the Plex agent, so it can't be watchlisted", item.title()));
            return;
        };
        let tv = match self.core.plex_tv() {
            Ok(tv) => tv,
            Err(e) => return self.error(err(e)),
        };
        let on = !self.watchlist.contains(&guid);
        if on {
            self.watchlist.insert(guid);
        } else {
            self.watchlist.remove(&guid);
        }
        let title = item.title().to_string();
        self.spawn_op(Refresh::Watchlist, async move {
            if on { tv.add_to_watchlist(&id).await } else { tv.remove_from_watchlist(&id).await }.map_err(err)?;
            Ok(Some(if on { format!("Added {title} to your watchlist") } else { format!("Removed {title} from your watchlist") }))
        });
    }

    // ---- collections -------------------------------------------------------

    pub(super) fn open_collections(&mut self) {
        if !self.owned() {
            return self.info("Only the server owner can manage collections");
        }
        let Some(item) = self.selected_detail().cloned() else { return };
        let section_id = item.section_id().or_else(|| self.columns.last()?.section_id().map(str::to_string));
        let (Some(key), Some(section_id)) = (item.rating_key.clone(), section_id) else {
            self.info("Select a library item to manage its collections");
            return;
        };
        if !matches!(item.kind(), Kind::Movie | Kind::Show | Kind::Artist | Kind::Album) {
            self.info("Collections hold movies, shows, artists and albums");
            return;
        }
        self.modal = Some(Modal::Collections(CollectionPicker {
            item,
            rows: Vec::new(),
            cursor: 0,
            loading: true,
            error: None,
        }));
        let (server, tx) = (self.server(), self.tx.clone());
        tokio::spawn(async move {
            let result = async {
                let server = server?;
                // Fresh metadata for the item's current collection tags.
                let full = server.metadata(&key, MetadataOptions::default()).await.map_err(err)?.ok_or("Item not found")?;
                let rows = server
                    .collections(&section_id)
                    .await
                    .map_err(err)?
                    .into_iter()
                    .filter(|c| !c.smart)
                    .map(|c| CollectionRow {
                        member: full.collections.iter().any(|t| t.name() == c.title()),
                        collection: Item::from(c),
                        busy: false,
                    })
                    .collect();
                Ok::<_, String>(rows)
            }
            .await;
            let _ = tx.send(Msg::CollectionRows(result));
        });
    }

    pub(super) fn toggle_collection_row(&mut self) {
        let Some(Modal::Collections(c)) = &mut self.modal else { return };
        let item_key = c.item.rating_key.clone().unwrap_or_default();
        if c.cursor == c.rows.len() {
            self.modal = Some(Modal::Input(Input {
                title: "New collection".into(),
                hint: "Name".into(),
                value: String::new(),
                purpose: InputPurpose::NewCollection { rating_key: item_key },
            }));
            return;
        }
        let Some(row) = c.rows.get_mut(c.cursor) else { return };
        if row.busy {
            return;
        }
        row.busy = true;
        let collection_key = row.collection.rating_key.clone().unwrap_or_default();
        let member = row.member;
        let (server, tx) = (self.server(), self.tx.clone());
        tokio::spawn(async move {
            let result = async {
                let server = server?;
                if member {
                    server.remove_from_collection(&collection_key, &item_key).await
                } else {
                    server.add_to_collection(&collection_key, &server.item_uri(&[&item_key])).await
                }
                .map_err(err)?;
                Ok(!member)
            }
            .await;
            let _ = tx.send(Msg::CollectionToggled { collection_key, result });
        });
    }

    pub(super) fn open_smart_playlist(&mut self) {
        let Some(col) = self.columns.last() else { return };
        match smart_playlist_path(col, &self.sections) {
            Some((kind, filter_path)) => {
                self.modal = Some(Modal::Input(Input {
                    title: "New smart playlist".into(),
                    hint: format!("Name (items matching {})", col.title),
                    value: col.title.clone(),
                    purpose: InputPurpose::SmartPlaylist { kind, filter_path },
                }));
            }
            None => self.info("Open a library listing (or a browse-by view) to save it"),
        }
    }

    pub(super) fn open_rate(&mut self) {
        let Some(item) = self.selected_detail().cloned() else { return };
        if !detail_kind(&item) {
            self.info("That can't be rated");
            return;
        }
        self.modal = Some(Modal::Input(Input {
            title: format!("Rate {}", item.title()),
            hint: "0–10 (empty clears)".into(),
            value: item.user_rating.map(|r| format!("{r}")).unwrap_or_default(),
            purpose: InputPurpose::Rate { rating_key: item.rating_key.clone().unwrap_or_default() },
        }));
    }

    pub(super) fn open_rename(&mut self) {
        let Some(item) = self.selected().cloned() else { return };
        if item.kind() != Kind::Playlist {
            self.info("Select a playlist to rename it");
            return;
        }
        self.modal = Some(Modal::Input(Input {
            title: "Rename playlist".into(),
            hint: "Name".into(),
            value: item.title().to_string(),
            purpose: InputPurpose::RenamePlaylist { playlist_id: item.rating_key.clone().unwrap_or_default() },
        }));
    }
}

pub(super) fn delete_collection_confirm(rating_key: String, title: &str) -> Confirm {
    Confirm {
        message: format!("Delete the {title} collection? Its items stay in the library."),
        action: ConfirmAction::DeleteCollection { rating_key, title: title.to_string() },
    }
}

/// Plex Web deep link for an item.
pub fn web_url(machine: &str, rating_key: &str) -> String {
    format!("https://app.plex.tv/desktop/#!/server/{machine}/details?key=%2Flibrary%2Fmetadata%2F{rating_key}")
}

/// For a library listing column (`/library/sections/<id>/all?…`), the
/// playlist type and filter path of an equivalent smart playlist.
pub fn smart_playlist_path(col: &Column, sections: &[Item]) -> Option<(&'static str, String)> {
    let path = col.path.as_deref()?;
    let sid = col.section_id()?;
    let rest = path.strip_prefix(&format!("/library/sections/{sid}/"))?;
    let (view, query) = rest.split_once('?').unwrap_or((rest, ""));
    if view != "all" {
        return None;
    }
    let section_kind = sections.iter().find(|s| s.key.as_deref() == Some(sid))?.meta.kind.clone()?;
    // Filters on seasons or episodes don't map onto the leaf-level rules below.
    if col.library().is_some_and(|v| v.kind != section_types(&section_kind).first().map(|t| t.0)) {
        return None;
    }
    // Playlists hold leaves, so show and music filters apply to the parent.
    let (type_num, prefix, kind) = match section_kind.as_str() {
        "movie" => (1, None, "video"),
        "show" => (4, Some("show"), "video"),
        "artist" => (10, Some("artist"), "audio"),
        _ => return None,
    };
    let mut params = vec![format!("type={type_num}")];
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        if k == "type" {
            continue;
        }
        let k = match prefix {
            Some(p) if !k.contains('.') && !matches!(k, "unwatched" | "sort") => format!("{p}.{k}"),
            _ => k.to_string(),
        };
        params.push(format!("{k}={v}"));
    }
    Some((kind, format!("/library/sections/{sid}/all?{}", params.join("&"))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use patina_plex::Directory;

    #[test]
    fn smart_playlist_paths() {
        let section = |key: &str, kind: &str| Item::section(Directory { key: Some(key.into()), kind: Some(kind.into()), ..Default::default() });
        let sections = vec![section("1", "movie"), section("2", "show")];
        let col = |p: &str| Column::new(9, "x", Source::Path(p.into()));
        assert_eq!(
            smart_playlist_path(&col("/library/sections/1/all?genre=18"), &sections),
            Some(("video", "/library/sections/1/all?type=1&genre=18".into()))
        );
        assert_eq!(
            smart_playlist_path(&col("/library/sections/2/all?unwatched=1&genre=5"), &sections).map(|(_, p)| p),
            Some("/library/sections/2/all?type=4&unwatched=1&show.genre=5".into())
        );
        assert!(smart_playlist_path(&col("/library/sections/1/genre"), &sections).is_none());
        assert!(smart_playlist_path(&col("/playlists/3/items"), &sections).is_none());
        let mut shows = crate::tui::source::LibraryView::new("2", "show");
        let lib = |v: &crate::tui::source::LibraryView| Column::new(9, "x", Source::Library(v.clone()));
        assert!(smart_playlist_path(&lib(&shows), &sections).is_some());
        shows.set_kind(4);
        assert!(smart_playlist_path(&lib(&shows), &sections).is_none());
    }

    #[test]
    fn web_links() {
        assert_eq!(web_url("abc", "7"), "https://app.plex.tv/desktop/#!/server/abc/details?key=%2Flibrary%2Fmetadata%2F7");
    }
}
