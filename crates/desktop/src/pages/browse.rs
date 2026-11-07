//! Any server listing by key: "See all" for hubs, and tag filters (genre, actor, ...).

pub mod ui;

use crate::prelude::*;
use std::rc::Rc;

use patina_plex::{Metadata, Params};

use crate::core::{bg, core, message};
use crate::format::{self, Shape};
use crate::widgets;

const PAGE: usize = 100;

pub struct Browse {
    path: String,
    title: String,
    items: Vec<Metadata>,
    done: bool,
    loading: bool,
    error: Option<String>,
}

impl Browse {
    pub fn new(path: String, title: String, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut this = Self { path, title, items: Vec::new(), done: false, loading: false, error: None };
        this.load(cx);
        this
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.error = None;
        self.loading = true;
        let app = core(cx);
        let path = self.path.clone();
        let start = self.items.len();
        cx.spawn(async move |this, cx| {
            let result = bg(async move {
                let params = Params::new().set("X-Plex-Container-Start", start).set("X-Plex-Container-Size", PAGE);
                Ok::<_, anyhow::Error>(app.server()?.fetch(&path, &params).await?)
            })
            .await;
            this.update(cx, |b, cx| {
                b.loading = false;
                match result {
                    Ok(mc) => {
                        let got = mc.metadata.len();
                        b.items.extend(mc.metadata);
                        // Some endpoints ignore paging; stop when a page comes back short.
                        b.done = got < PAGE || mc.total_size.is_some_and(|t| b.items.len() as i64 >= t);
                    }
                    Err(e) => b.error = Some(message(&e)),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for Browse {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let me = cx.entity().downgrade();
        let retry: ui::Handler = Rc::new(move |_, cx| {
            me.update(cx, |b, cx| b.load(cx)).ok();
        });
        let mut page = widgets::page("browse").child(widgets::page_head(self.title.clone(), None, None, cx));
        if let Some(e) = &self.error {
            page = page.child(if self.items.is_empty() {
                ui::status_error("browse-retry", e.clone(), Some(retry), cx)
            } else {
                ui::stale_alert("browse-retry", e.clone(), Some(retry), cx)
            });
        }
        if self.items.is_empty() {
            if self.error.is_none() {
                page = page.child(if self.loading { ui::status_loading() } else { ui::status_empty("Nothing here", None, cx) });
            }
            return page;
        }
        let wide = format::shape_for(&self.items[0]) == Shape::Landscape;
        page = page.child(ui::item_grid("browse", self.items.clone(), wide, None, cx));
        if !self.done && self.error.is_none() {
            let me = cx.entity().downgrade();
            page = page.child(ui::more_sentinel(self.loading, move |cx| {
                me.update(cx, |b, cx| b.load(cx)).ok();
            }));
        }
        page
    }
}
