//! The playlists on the server, as a grid of square cards.

use crate::prelude::*;
use patina_plex::Metadata;

use crate::core::{Load, bg, core};
use crate::format::Shape;
use crate::store::store;
use crate::widgets::{self, Card};

pub struct Playlists {
    lists: Load<Vec<Metadata>>,
    /// The last good list, shown while reloading or after a failed reload.
    stale: Option<Vec<Metadata>>,
    revision: u64,
    _subs: Vec<Subscription>,
}

impl Playlists {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subs = vec![cx.observe(&store(cx), |this: &mut Self, store, cx| {
            if store.read(cx).revision != this.revision {
                this.load(cx);
            }
        })];
        let mut this = Self { lists: Load::Loading, stale: None, revision: 0, _subs: subs };
        this.load(cx);
        this
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        self.revision = store(cx).read(cx).revision;
        let app = core(cx);
        cx.spawn(async move |this, cx| {
            let result = bg(async move { Ok::<_, anyhow::Error>(app.server()?.playlists(None, None).await?) }).await;
            this.update(cx, |p, cx| {
                p.lists = Load::from_result(result);
                if let Load::Ready(l) = &p.lists {
                    p.stale = Some(l.clone());
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for Playlists {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let width = store(cx).read(cx).poster_width();
        let retry = |cx: &mut Context<Self>| Button::new("retry").outline().small().mt_3().label("Try again").on_click(cx.listener(|this, _, _, cx| this.load(cx)));
        let page = widgets::page("playlists").child(widgets::page_head("Playlists", Some("Right-click anything and choose “Add to playlist…” to build one.".into()), None, cx));
        let list = self.lists.ready().cloned().or_else(|| self.stale.clone());
        let Some(list) = list else {
            return match &self.lists {
                Load::Failed(e) => page.child(widgets::failed(e.clone(), cx).child(retry(cx))),
                _ => page.child(widgets::loading()),
            };
        };
        let page = match &self.lists {
            Load::Failed(e) => page.child(
                h_flex().mb_4().gap_3().items_center().text_sm().text_color(theme.danger).child(format!("Could not refresh: {e}")).child(retry(cx)),
            ),
            _ => page,
        };
        if list.is_empty() {
            return page.child(widgets::empty("No playlists yet", None, cx));
        }
        page.child(h_flex().flex_wrap().items_start().gap_5().w_full().children(
            list.into_iter().enumerate().map(|(i, m)| Card::new(ElementId::Name(format!("playlist:{}:{i}", m.rating_key()).into()), m).shape(Shape::Square).width(width)),
        ))
    }
}
