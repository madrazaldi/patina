//! TODO: port `web/src/pages/Browse.svelte`.

use crate::prelude::*;

use crate::widgets;

pub struct Browse {}

impl Browse {
    pub fn new(path: String, title: String, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        let _ = path; let _ = title;
        Self {}
    }
}

impl Render for Browse {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        widgets::page("browse").child(widgets::empty("Browse", Some("Not built yet.".into()), cx))
    }
}
