//! Undo (`I`): reverses the last watch-state change, playlist-entry removal or removal
//! from Continue Watching. The history lives in core ([`patina_core::undo`]), scoped to
//! the signed-in account and server.

use patina_core::undo;

use super::*;

impl App {
    /// `I`: undo the most recent reversible action.
    pub(super) fn undo_last(&mut self) {
        let core = self.core.clone();
        // Core announces what it undid with a toast; only a failure comes back here.
        self.spawn_op(Refresh::Nothing, async move {
            undo::undo(&core, None).await.map_err(err)?;
            Ok(None)
        });
    }

    /// Mark the selection watched or unwatched through core, which remembers how to undo it.
    pub(super) fn mark_watched(&mut self, keys: Vec<String>, server: Option<String>, watched: bool) {
        let core = self.core.clone();
        self.spawn_op(Refresh::Nothing, async move {
            undo::set_watched(&core, server.as_deref(), &keys, watched).await.map_err(err)?;
            Ok(None)
        });
    }

    pub(super) fn forget_progress(&mut self, key: String) {
        let core = self.core.clone();
        self.spawn_op(Refresh::Nothing, async move {
            undo::clear_progress(&core, &key).await.map_err(err)?;
            Ok(None)
        });
    }

    pub(super) fn hide_from_continue(&mut self, key: String) {
        let core = self.core.clone();
        self.spawn_op(Refresh::Nothing, async move {
            undo::remove_from_continue_watching(&core, &key).await.map_err(err)?;
            Ok(None)
        });
    }

    pub(super) fn remove_playlist_entry(&mut self, playlist: String, entry: i64) {
        let core = self.core.clone();
        self.spawn_op(Refresh::Playlists, async move {
            undo::remove_from_playlist(&core, &playlist, entry).await.map_err(err)?;
            Ok(None)
        });
    }
}

/// The notice for a toast that offers Undo.
pub fn undo_notice(text: &str) -> String {
    format!("{text} — I to undo")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_notice_names_the_undo_key() {
        assert_eq!(undo_notice("Marked Heat watched"), "Marked Heat watched — I to undo");
    }
}
