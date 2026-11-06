//! TODO: port `web/src/pages/Library.svelte`.

use crate::prelude::*;

use crate::widgets;

pub struct Library {}

impl Library {
    pub fn new(id: String, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        let _ = id;
        Self {}
    }
}

impl Render for Library {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        widgets::page("library").child(widgets::empty("Library", Some("Not built yet.".into()), cx))
    }
}
