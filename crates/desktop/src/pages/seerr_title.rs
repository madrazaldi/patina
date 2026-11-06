//! TODO: port `web/src/pages/SeerrTitle.svelte`.

use crate::prelude::*;

use crate::widgets;

pub struct SeerrTitle {}

impl SeerrTitle {
    pub fn new(media_type: String, tmdb_id: i64, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        let _ = media_type; let _ = tmdb_id;
        Self {}
    }
}

impl Render for SeerrTitle {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        widgets::page("seerr_title").child(widgets::empty("SeerrTitle", Some("Not built yet.".into()), cx))
    }
}
