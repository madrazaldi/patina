use super::*;
use super::actions::{delete_collection_confirm, smart_playlist_path};

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
        if super::queue::queueable(&item) {
            rows.push((PlayNext, "Play next", "n"));
            rows.push((AddToQueue, "Add to queue", "N"));
        }
        if item.rating_key.is_some() && matches!(kind, Kind::Artist | Kind::Album | Kind::Track) {
            rows.push((Radio, "Start radio", ""));
        }
        if matches!(kind, Kind::Episode | Kind::Track) && item.grandparent_rating_key.is_some() {
            rows.push((GoToGrandparent, if kind == Kind::Track { "Go to artist" } else { "Go to show" }, "B"));
        }
        if matches!(kind, Kind::Episode | Kind::Track | Kind::Season | Kind::Album) && item.parent_rating_key.is_some() {
            let label = match kind {
                Kind::Track => "Go to album",
                Kind::Season => "Go to show",
                Kind::Album => "Go to artist",
                _ => "Go to season",
            };
            rows.push((GoToParent, label, "b"));
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
        let owned = self.owned();
        if owned && item.rating_key.is_some() && matches!(kind, Kind::Movie | Kind::Show | Kind::Artist | Kind::Album) {
            rows.push((Collections, "Collections…", "C"));
        }
        let detail = self.selected_detail();
        if let Some(n) = detail.and_then(|d| d.extras.as_ref()).and_then(|e| e.size).filter(|n| *n > 0) {
            rows.push((Extras, if n == 1 { "Extras (1)" } else { "Extras (trailers, featurettes…)" }, ""));
        }
        if detail_kind(&item) {
            rows.push((Related, "Related, cast & tags…", "c"));
            rows.push((OpenWeb, "Open in Plex Web", "O"));
        }
        if item.is_playable() {
            rows.push((CopyUrl, "Copy stream URL", "y"));
        }
        if item.rating_key.is_some() && (item.is_playable() || matches!(kind, Kind::Show | Kind::Season | Kind::Album | Kind::Artist)) {
            let saved = self.session.as_ref().and_then(|s| self.downloads.iter().find(|d|
                d.server_id == s.server_id && (d.account.is_empty() || d.account == s.account_id)
                    && d.item.rating_key == item.rating_key
            )).map(|d| d.state);
            use patina_core::downloads::State;
            rows.push((
                Download,
                match saved {
                    Some(State::Done) => "Downloaded ✓",
                    Some(State::Queued | State::Downloading) => "Downloading…",
                    _ => "Download for offline",
                },
                "D",
            ));
        }
        if matches!(kind, Kind::Show | Kind::Season | Kind::Episode) {
            rows.push((Tracks, "Audio & subtitles for the show…", "T"));
            rows.push((ShowPrefs, "Playback preferences for the show…", ""));
        }
        if item.rating_key.is_some() && matches!(kind, Kind::Show | Kind::Season | Kind::Playlist) {
            rows.push((KeepDownloaded, "Keep next unwatched downloaded…", ""));
        }
        if matches!(kind, Kind::Movie | Kind::Episode) {
            rows.push((SearchSubtitles, "Search subtitles online…", ""));
        }
        if owned && detail_kind(&item) && item.type_number().is_some() {
            rows.push((EditDetails, "Edit details…", "e"));
            if kind != Kind::Track {
                rows.push((ChangePoster, "Choose poster…", ""));
            }
            if matches!(kind, Kind::Movie | Kind::Show | Kind::Artist | Kind::Album | Kind::Collection) {
                rows.push((ChangeArt, "Choose background…", ""));
            }
        }
        if owned && kind == Kind::Collection && item.rating_key.is_some() {
            rows.push((CollectionMode, "Show in library…", ""));
            rows.push((CollectionSort, "Collection sort…", ""));
        }
        if owned && matches!(kind, Kind::Movie | Kind::Show | Kind::Artist | Kind::Album) {
            rows.push((FixMatch, "Fix match…", ""));
        }
        if owned && detail_kind(&item) {
            rows.push((RefreshMetadata, "Refresh metadata", ""));
            rows.push((Analyze, "Analyze media", ""));
            rows.push((DeleteMedia, if kind == Kind::Collection { "Delete collection…" } else { "Delete from disk…" }, if kind == Kind::Collection { "x" } else { "" }));
        }
        if smart_playlist_path(col, &self.sections).is_some() {
            rows.push((SmartPlaylist, "Save view as smart playlist…", ""));
        }
        if let Some(v) = col.library() {
            rows.push((Sort, "Sort…", "s"));
            rows.push((Filter, "Filter…", "F"));
            if v.alphabetical() {
                rows.push((Letters, "Jump to letter…", "A"));
            }
            if v.type_label().is_some() {
                rows.push((SwitchType, "Switch type (shows, seasons, episodes…)", "t"));
            }
        }
        if kind == Kind::Section || col.section_id().is_some() {
            rows.push((BrowseBy, "Library views…", "v"));
            rows.push((ScanLibrary, "Scan library files", "S"));
            rows.push((ForceScanLibrary, "Refresh all library metadata", ""));
            rows.push((EmptyTrash, "Empty library trash", ""));
        }
        if kind == Kind::Playlist {
            rows.push((RenamePlaylist, "Rename playlist…", "e"));
            rows.push((EditPlaylistSummary, "Edit description…", ""));
            if !item.smart && item.leaf_count.unwrap_or(0) > 0 {
                rows.push((ClearPlaylist, "Clear playlist…", ""));
            }
            rows.push((DeletePlaylist, "Delete playlist", "x"));
        }
        if col.is_playlist().is_some() && item.playlist_item_id.is_some() {
            rows.push((RemoveFromPlaylist, "Remove from playlist", "x"));
        }
        if owned && col.is_collection().is_some() {
            rows.push((RemoveFromCollection, "Remove from collection", "x"));
        }
        if owned && matches!(col.source, Source::History(_)) && item.history_key.is_some() {
            rows.push((DeleteHistory, "Delete from history…", "x"));
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
            PlayNext => self.enqueue(true),
            AddToQueue => self.enqueue(false),
            Radio => self.start_radio(),
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
            Related => self.open_related(),
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
            Sort => self.open_sort(),
            Filter => self.open_filters(),
            Letters => self.open_letters(),
            SwitchType => self.cycle_type(),
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
            ChangePoster => self.open_artwork(false),
            ChangeArt => self.open_artwork(true),
            CollectionMode => self.open_collection_setting(false),
            CollectionSort => self.open_collection_setting(true),
            ClearPlaylist => {
                if let Some(i) = item.filter(|i| i.kind() == Kind::Playlist) {
                    self.modal = Some(Modal::Confirm(Confirm {
                        message: format!("Clear {}? Every item is removed; the playlist itself stays.", i.title()),
                        action: ConfirmAction::ClearPlaylist { playlist_id: i.rating_key.clone().unwrap_or_default() },
                    }));
                }
            }
            EditPlaylistSummary => {
                if let Some(i) = item.filter(|i| i.kind() == Kind::Playlist) {
                    self.modal = Some(Modal::Input(Input {
                        title: format!("Description · {}", i.title()),
                        hint: "Description".into(),
                        value: i.summary.clone().unwrap_or_default(),
                        purpose: InputPurpose::PlaylistSummary { playlist_id: i.rating_key.clone().unwrap_or_default() },
                    }));
                }
            }
            FixMatch => {
                if let Some(i) = self.editable() {
                    self.search_matches(i.rating_key.clone().unwrap_or_default(), i.title().to_string(), i.year, None);
                }
            }
            DeleteMedia => {
                if let Some(i) = self.editable() {
                    let rating_key = i.rating_key.clone().unwrap_or_default();
                    let confirm = if i.kind() == Kind::Collection {
                        delete_collection_confirm(rating_key, i.title())
                    } else {
                        Confirm {
                            message: format!("Delete {} and its files from disk? This can't be undone.", i.full_title()),
                            action: ConfirmAction::DeleteMedia { rating_key, title: i.full_title() },
                        }
                    };
                    self.modal = Some(Modal::Confirm(confirm));
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
            Tracks => self.open_tracks(),
            ShowPrefs => self.open_show_prefs(),
            KeepDownloaded => self.open_keep(),
            SwitchUser => self.open_users(),
            DeletePlaylist | RemoveFromPlaylist | RemoveFromCollection | TerminateSession | DeleteHistory => self.delete_action(),
            GoToParent => self.go_to_parent(false),
            GoToGrandparent => self.go_to_parent(true),
            Row(op) => self.run_row(op),
            Bulk(op) => self.bulk_run(op),
        }
    }
}
