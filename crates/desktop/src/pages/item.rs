//! TODO: port `web/src/pages/Item.svelte`.

use crate::prelude::*;

use crate::widgets;

pub struct Item {}

impl Item {
    pub fn new(rating_key: String, server: Option<String>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        let _ = rating_key; let _ = server;
        Self {}
    }
}

impl Render for Item {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        widgets::page("item").child(widgets::empty("Item", Some("Not built yet.".into()), cx))
    }
}
