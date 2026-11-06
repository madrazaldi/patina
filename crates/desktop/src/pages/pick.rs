//! TODO: port `web/src/pages/Pick.svelte`.

use crate::prelude::*;

use crate::widgets;

pub struct Pick {}

impl Pick {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        
        Self {}
    }
}

impl Render for Pick {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        widgets::page("pick").child(widgets::empty("Pick", Some("Not built yet.".into()), cx))
    }
}
