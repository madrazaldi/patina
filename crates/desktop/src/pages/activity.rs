//! TODO: port `web/src/pages/Activity.svelte`.

use crate::prelude::*;

use crate::widgets;

pub struct Activity {}

impl Activity {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        
        Self {}
    }
}

impl Render for Activity {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        widgets::page("activity").child(widgets::empty("Activity", Some("Not built yet.".into()), cx))
    }
}
