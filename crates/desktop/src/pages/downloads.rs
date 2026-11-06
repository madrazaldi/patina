//! TODO: port `web/src/pages/Downloads.svelte`.

use crate::prelude::*;

use crate::widgets;

pub struct Downloads {}

impl Downloads {
    pub fn new(offline: bool, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        let _ = offline;
        Self {}
    }
}

impl Render for Downloads {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        widgets::page("downloads").child(widgets::empty("Downloads", Some("Not built yet.".into()), cx))
    }
}
