//! TODO: port `web/src/pages/Playlist.svelte`.

use crate::prelude::*;

use crate::widgets;

pub struct Playlist {}

impl Playlist {
    pub fn new(id: String, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        let _ = id;
        Self {}
    }
}

impl Render for Playlist {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        widgets::page("playlist").child(widgets::empty("Playlist", Some("Not built yet.".into()), cx))
    }
}
