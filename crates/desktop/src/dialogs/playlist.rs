//! TODO: port the matching dialog from `web/src/components/`.

use crate::prelude::*;
use patina_plex::Metadata;

/// Placeholder until the playlist dialog lands.
pub fn open(items: Vec<Metadata>, _window: &mut Window, cx: &mut App) {
    let _ = items;
    crate::store::toast(cx, "Playlists are not available yet");
}
