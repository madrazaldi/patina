//! TODO: port `web/src/pages/Watchlist.svelte`.

use crate::prelude::*;

use crate::widgets;

pub struct Watchlist {}

impl Watchlist {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        
        Self {}
    }
}

impl Render for Watchlist {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        widgets::page("watchlist").child(widgets::empty("Watchlist", Some("Not built yet.".into()), cx))
    }
}
