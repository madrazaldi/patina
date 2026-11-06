//! TODO: port `web/src/pages/Search.svelte`.

use crate::prelude::*;

use crate::widgets;

pub struct Search {}

impl Search {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        
        Self {}
    }
}

impl Render for Search {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        widgets::page("search").child(widgets::empty("Search", Some("Not built yet.".into()), cx))
    }
}
