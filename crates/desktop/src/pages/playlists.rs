//! TODO: port `web/src/pages/Playlists.svelte`.

use crate::prelude::*;

use crate::widgets;

pub struct Playlists {}

impl Playlists {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        
        Self {}
    }
}

impl Render for Playlists {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        widgets::page("playlists").child(widgets::empty("Playlists", Some("Not built yet.".into()), cx))
    }
}
