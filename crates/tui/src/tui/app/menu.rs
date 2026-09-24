use super::*;
use super::actions::smart_playlist_path;

impl App {
    pub(super) fn open_menu(&mut self) {
        let Some(col) = self.columns.last() else { return };
        let Some(item) = self.selected().cloned() else { return };
        use MenuAction::*;
        let mut rows: Vec<(MenuAction, &'static str, &'static str)> = Vec::new();
        let kind = item.kind();
        if item.is_playable() {
            rows.push((Play, "Play / resume", "enter"));
            rows.push((PlayFromStart, "Play from start", "p"));
            rows.push((PlayOptions, "Play with options…", "o"));
            rows.push((Shuffle, "Shuffle from here", "z"));
        } else if item.rating_key.is_some() && matches!(kind, Kind::Show | Kind::Season | Kind::Artist | Kind::Album | Kind::Playlist | Kind::Collection) {
            rows.push((PlayAll, "Play all", "a"));
            rows.push((Shuffle, "Shuffle", "z"));
        }
        if item.is_watchable() {
            rows.push((MarkWatched, "Mark watched", "w"));
            rows.push((MarkUnwatched, "Mark unwatched", "u"));
        }
        // As on the GUI: progress is clearable when there is some, and
        // episodes (next up) or started movies can leave Continue Watching.
        let resume = item.progress().is_some() && item.rating_key.is_some();
        if resume {
            rows.push((ClearProgress, "Clear progress", ""));
        }
        if item.rating_key.is_some() && (kind == Kind::Episode || (kind == Kind::Movie && resume)) {
            rows.push((RemoveFromContinue, "Remove from Continue Watching", ""));
        }
        if detail_kind(&item) {
            rows.push((Rate, "Rate…", "*"));
        }
        if matches!(kind, Kind::Movie | Kind::Show | Kind::Remote) && item.guid.is_some() && self.core.tv().is_some() {
            let on = item.guid.as_ref().is_some_and(|g| self.watchlist.contains(g));
            rows.push((Watchlist, if on { "Remove from watchlist" } else { "Add to watchlist" }, "+"));
        }
        if item.rating_key.is_some() && (item.is_playable() || matches!(kind, Kind::Show | Kind::Season | Kind::Album | Kind::Artist)) {
            rows.push((AddToPlaylist, "Playlists…", "space"));
        }
        if item.rating_key.is_some() && matches!(kind, Kind::Movie | Kind::Show | Kind::Artist | Kind::Album) {
            rows.push((Collections, "Collections…", "C"));
        }
        let detail = self.selected_detail();
        if let Some(n) = detail.and_then(|d| d.extras.as_ref()).and_then(|e| e.size).filter(|n| *n > 0) {
            rows.push((Extras, if n == 1 { "Extras (1)" } else { "Extras (trailers, featurettes…)" }, ""));
        }
        if detail_kind(&item) {
            rows.push((Related, "Related items", ""));
            rows.push((OpenWeb, "Open in Plex Web", "O"));
        }
        if item.is_playable() {
            rows.push((CopyUrl, "Copy stream URL", "y"));
        }
        if item.rating_key.is_some() && (item.is_playable() || matches!(kind, Kind::Show | Kind::Season | Kind::Album | Kind::Artist)) {
            rows.push((Download, "Download for offline", "D"));
        }
        if matches!(kind, Kind::Movie | Kind::Episode) {
            rows.push((SearchSubtitles, "Search subtitles online…", ""));
        }
        if detail_kind(&item) && item.type_number().is_some() {
            rows.push((EditDetails, "Edit details…", "e"));
            rows.push((ChangePoster, "Change poster…", ""));
        }
        if matches!(kind, Kind::Movie | Kind::Show | Kind::Artist | Kind::Album) {
            rows.push((FixMatch, "Fix match…", ""));
            rows.push((Unmatch, "Unmatch", ""));
        }
        if detail_kind(&item) {
            rows.push((RefreshMetadata, "Refresh metadata", ""));
            rows.push((Analyze, "Analyze media", ""));
        }
        if detail_kind(&item) && kind != Kind::Collection {
            rows.push((DeleteMedia, "Delete from disk…", ""));
        }
        if smart_playlist_path(col, &self.sections).is_some() {
            rows.push((SmartPlaylist, "Save view as smart playlist…", ""));
        }
        if kind == Kind::Section || col.section_id().is_some() {
            rows.push((BrowseBy, "Browse by…", "v"));
            rows.push((ScanLibrary, "Scan library files", "S"));
            rows.push((ForceScanLibrary, "Refresh all library metadata", ""));
            rows.push((EmptyTrash, "Empty library trash", ""));
        }
        if kind == Kind::Playlist {
            rows.push((RenamePlaylist, "Rename playlist…", "e"));
            rows.push((DeletePlaylist, "Delete playlist", "x"));
        }
        if col.is_playlist().is_some() && item.playlist_item_id.is_some() {
            rows.push((RemoveFromPlaylist, "Remove from playlist", "x"));
        }
        if col.is_collection().is_some() {
            rows.push((RemoveFromCollection, "Remove from collection", "x"));
        }
        if col.is_sessions() && item.session.is_some() {
            rows.push((TerminateSession, "Stop this stream", "x"));
        }
        if self.core.tv().is_some() {
            rows.push((SwitchUser, "Switch Plex Home user…", "U"));
        }
        self.modal = Some(Modal::Menu(Menu { title: item.full_title(), rows, cursor: 0 }));
    }

    pub(super) fn run_menu(&mut self, action: MenuAction) {
        use MenuAction::*;
        let item = self.selected().cloned();
        match action {
            Play => self.play_selected(false),
            PlayFromStart => self.play_selected(true),
            PlayAll => {
                if let Some(i) = item {
                    self.play_container(i, false)
                }
            }
            Shuffle => {
                if let Some(i) = item {
                    self.play_container(i, true)
                }
            }
            PlayOptions => self.open_play_options(),
            MarkWatched => self.set_watched(true),
            MarkUnwatched => self.set_watched(false),
            ClearProgress => self.clear_progress(),
            RemoveFromContinue => {
                let Some((k, title)) = item.and_then(|i| Some((i.rating_key.clone()?, i.full_title()))) else { return };
                self.server_op(Refresh::WatchState, format!("Removed {title} from Continue Watching"), move |s| async move {
                    s.remove_from_continue_watching(&k).await
                });
            }
            Rate => self.open_rate(),
            AddToPlaylist => self.open_playlist_picker(),
            Related => {
                if let Some(i) = item
                    && let Some(k) = i.rating_key.clone()
                {
                    self.search = None;
                    self.open_column(format!("Related to {}", i.title()), format!("/library/metadata/{k}/related"));
                }
            }
            OpenWeb => self.open_web(),
            CopyUrl => self.copy_url(),
            Download => self.download_selected(),
            RefreshMetadata | Analyze => {
                let Some(i) = item else { return };
                let Some(k) = i.rating_key.clone() else { return };
                let refresh = action == RefreshMetadata;
                self.details.remove(&k);
                let done = format!("{} {}", if refresh { "Refreshing metadata for" } else { "Analyzing" }, i.title());
                self.server_op(Refresh::Nothing, done, move |s| async move {
                    if refresh { s.refresh_metadata(&k, false).await } else { s.analyze_metadata(&k).await }
                });
            }
            ScanLibrary => self.scan_library(false),
            ForceScanLibrary => self.scan_library(true),
            EmptyTrash => {
                let sid = item
                    .as_ref()
                    .filter(|i| i.kind() == Kind::Section)
                    .and_then(|i| i.key.clone())
                    .or_else(|| self.columns.last()?.section_id().map(str::to_string));
                if let Some(section_id) = sid {
                    self.modal = Some(Modal::Confirm(Confirm {
                        message: "Permanently remove this library's missing items?".into(),
                        action: ConfirmAction::EmptyTrash { section_id },
                    }));
                }
            }
            BrowseBy => self.open_browse(),
            RenamePlaylist => self.open_rename(),
            Watchlist => self.toggle_watchlist(),
            Collections => self.open_collections(),
            SmartPlaylist => self.open_smart_playlist(),
            Extras => {
                if let Some(i) = item
                    && let Some(k) = i.rating_key.clone()
                {
                    self.search = None;
                    self.open_column(format!("Extras · {}", i.title()), format!("/library/metadata/{k}/extras"));
                }
            }
            EditDetails => self.open_edit(),
            ChangePoster => self.open_posters(),
            FixMatch => {
                if let Some(i) = self.editable() {
                    self.modal = Some(Modal::Input(Input {
                        title: format!("Fix match · {}", i.title()),
                        hint: "Search title".into(),
                        value: i.title().to_string(),
                        purpose: InputPurpose::MatchQuery { rating_key: i.rating_key.clone().unwrap_or_default(), year: i.year },
                    }));
                }
            }
            Unmatch => {
                if let Some(i) = self.editable() {
                    self.modal = Some(Modal::Confirm(Confirm {
                        message: format!("Unmatch {}? Its metadata will be cleared.", i.title()),
                        action: ConfirmAction::Unmatch { rating_key: i.rating_key.clone().unwrap_or_default() },
                    }));
                }
            }
            DeleteMedia => {
                if let Some(i) = self.editable() {
                    self.modal = Some(Modal::Confirm(Confirm {
                        message: format!("Delete {} and its files from disk? This can't be undone.", i.full_title()),
                        action: ConfirmAction::DeleteMedia { rating_key: i.rating_key.clone().unwrap_or_default(), title: i.full_title() },
                    }));
                }
            }
            SearchSubtitles => {
                if let Some(i) = item.filter(|i| matches!(i.kind(), Kind::Movie | Kind::Episode)) {
                    self.modal = Some(Modal::Input(Input {
                        title: format!("Subtitles · {}", i.full_title()),
                        hint: "Language code (en, es, fr, …)".into(),
                        value: "en".into(),
                        purpose: InputPurpose::SubtitleLanguage { rating_key: i.rating_key.clone().unwrap_or_default() },
                    }));
                }
            }
            SwitchUser => self.open_users(),
            DeletePlaylist | RemoveFromPlaylist | RemoveFromCollection | TerminateSession => self.delete_action(),
        }
    }
}
