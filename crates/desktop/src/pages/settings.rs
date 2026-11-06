//! TODO: port `web/src/pages/Settings.svelte`.

use crate::prelude::*;

use crate::widgets;

pub struct Settings {}

impl Settings {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        
        Self {}
    }
}

impl Render for Settings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        widgets::page("settings").child(widgets::empty("Settings", Some("Not built yet.".into()), cx))
    }
}
