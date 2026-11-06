//! TODO: port `web/src/pages/Requests.svelte`.

use crate::prelude::*;

use crate::widgets;

pub struct Requests {}

impl Requests {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        
        Self {}
    }
}

impl Render for Requests {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        widgets::page("requests").child(widgets::empty("Requests", Some("Not built yet.".into()), cx))
    }
}
