//! TODO: port `web/src/pages/LiveTv.svelte`.

use crate::prelude::*;

use crate::widgets;

pub struct LiveTv {}

impl LiveTv {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        
        Self {}
    }
}

impl Render for LiveTv {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        widgets::page("livetv").child(widgets::empty("LiveTv", Some("Not built yet.".into()), cx))
    }
}
