//! TODO: port `web/src/pages/Server.svelte`.

use crate::prelude::*;

use crate::widgets;

pub struct Server {}

impl Server {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        
        Self {}
    }
}

impl Render for Server {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        widgets::page("server").child(widgets::empty("Server", Some("Not built yet.".into()), cx))
    }
}
