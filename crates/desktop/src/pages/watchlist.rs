//! plex.tv watchlist. Each entry links to the server's own copy when there is one.

use crate::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use patina_core::ops::lists;
use patina_plex::Metadata;

use crate::art::art_of;
use crate::core::{Load, bg, core};
use crate::format;
use crate::nav::{self, Route};
use crate::pages::browse::ui;
use crate::store::{self, store};
use crate::widgets;

/// How many "is it on this server?" lookups run at once.
const LOOKUPS: usize = 5;

pub struct Watchlist {
    list: Load<Vec<Metadata>>,
    stale: Option<Vec<Metadata>>,
    revision: u64,
    connected: bool,
    scope: String,
    /// guid → the server's rating key, or `None` when it doesn't have the title. Missing while unknown;
    /// a failed lookup is not an answer (the entry isn't dimmed) and is retried.
    local: HashMap<String, Option<String>>,
    /// guid → the revision its answer was obtained at; older answers are looked up again.
    answered: HashMap<String, u64>,
    /// guid → the revision it was started at, for lookups in flight.
    running: HashMap<String, u64>,
    queued: VecDeque<String>,
    /// Lookups that failed, as opposed to ones that found the title missing.
    failed: HashSet<String>,
    _subs: Vec<Subscription>,
}

impl Watchlist {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subs = vec![cx.observe(&store(cx), |this: &mut Self, store, cx| {
            let (revision, connected) = (store.read(cx).revision, store.read(cx).connected);
            if revision != this.revision {
                this.load(cx);
            } else if connected != this.connected {
                // Reconnecting makes old answers unreliable too.
                this.connected = connected;
                this.answered.clear();
                this.check(cx);
            }
            cx.notify();
        })];
        let s = store(cx).read(cx);
        let (connected, scope) = (s.connected, s.scope());
        let mut this = Self {
            list: Load::Loading,
            stale: None,
            revision: 0,
            connected,
            scope,
            local: HashMap::new(),
            answered: HashMap::new(),
            running: HashMap::new(),
            queued: VecDeque::new(),
            failed: HashSet::new(),
            _subs: subs,
        };
        this.load(cx);
        this
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        self.revision = store(cx).read(cx).revision;
        let app = core(cx);
        cx.spawn(async move |this, cx| {
            let result = bg(async move { lists::watchlist(&app).await }).await;
            this.update(cx, |w, cx| {
                w.list = Load::from_result(result);
                if let Some(l) = w.list.ready() {
                    w.stale = Some(l.clone());
                }
                w.check(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn entries(&self) -> Option<&Vec<Metadata>> {
        self.list.ready().or(self.stale.as_ref())
    }

    fn guids(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for g in self.entries().into_iter().flatten().filter_map(|m| m.guid.clone()) {
            if !out.contains(&g) {
                out.push(g);
            }
        }
        out
    }

    /// Look up every entry whose answer is missing or from before the latest library change.
    fn check(&mut self, cx: &mut Context<Self>) {
        for guid in self.guids() {
            if self.answered.get(&guid) == Some(&self.revision) || self.running.contains_key(&guid) || self.queued.contains(&guid) {
                continue;
            }
            self.queued.push_back(guid);
        }
        self.pump(cx);
    }

    fn pump(&mut self, cx: &mut Context<Self>) {
        while self.running.len() < LOOKUPS {
            let Some(guid) = self.queued.pop_front() else { break };
            let revision = self.revision;
            self.running.insert(guid.clone(), revision);
            let app = core(cx);
            let scope = self.scope.clone();
            let g = guid.clone();
            cx.spawn(async move |this, cx| {
                let result = bg(async move { Ok::<_, anyhow::Error>(app.server()?.find_by_guid(&g).await?) }).await;
                this.update(cx, |w, cx| {
                    w.running.remove(&guid);
                    if store(cx).read(cx).scope() != scope {
                        return;
                    }
                    match result {
                        Ok(found) => {
                            w.local.insert(guid.clone(), found.first().map(|m| m.rating_key().to_string()));
                            w.answered.insert(guid.clone(), revision);
                            w.failed.remove(&guid);
                        }
                        Err(_) => {
                            w.answered.remove(&guid);
                            w.failed.insert(guid.clone());
                        }
                    }
                    // The library changed while it ran: the answer, or the failure, may be out of date already.
                    if revision != w.revision && !w.queued.contains(&guid) {
                        w.queued.push_back(guid.clone());
                    }
                    w.pump(cx);
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
    }

    fn open(&self, m: &Metadata, cx: &mut App) {
        let answer = m.guid.as_ref().and_then(|g| self.local.get(g));
        match answer {
            Some(Some(rk)) => nav::push(Route::Item { rating_key: rk.clone(), server: None }, cx),
            None if m.guid.is_some() => store::toast(cx, "Still checking this server"),
            _ => store::toast(cx, "Not available on this server"),
        }
    }

    fn entry(&self, i: usize, m: &Metadata, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let missing = m.guid.as_ref().is_some_and(|g| matches!(self.local.get(g), Some(None)));
        let w = ui::card_width(format::Shape::Poster, cx);
        let group: SharedString = format!("watch-{i}").into();
        let item = m.clone();
        let discover_id = m.rating_key().to_string();
        let sub = if missing { "Not on this server".to_string() } else { format::subtitle(m) };
        div()
            .id(("watch", i))
            .group(group.clone())
            .w(px(w))
            .flex_none()
            .min_w_0()
            .cursor_pointer()
            .on_click(cx.listener(move |w, _, _, cx| w.open(&item, cx)))
            .child(
                div()
                    .relative()
                    .w_full()
                    .aspect_ratio(2. / 3.)
                    .overflow_hidden()
                    .rounded(px(10.))
                    .bg(theme.colors.group_box)
                    .border_1()
                    .border_color(theme.foreground.opacity(0.1))
                    .group_hover(group.clone(), |s| s.border_color(theme.foreground.opacity(0.25)))
                    .when(missing, |d| d.opacity(0.4))
                    .children(art_of(m.thumb.as_deref(), 300., 450., None).map(|i| i.size_full().object_fit(ObjectFit::Cover)))
                    .child(
                        div().absolute().top_2().right_2().invisible().group_hover(group, |s| s.visible()).child(
                            Button::new(("watch-remove", i))
                                .ghost()
                                .xsmall()
                                .icon(IconName::X)
                                .bg(theme.background.opacity(0.8))
                                .tooltip("Remove from watchlist")
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    let app = core(cx);
                                    let id = discover_id.clone();
                                    store::act(cx, async move { app.plex_tv()?.remove_from_watchlist(&id).await.map_err(anyhow::Error::from) }, Some("Removed from watchlist".into()), true);
                                }),
                        ),
                    ),
            )
            .child(div().mt_2().text_sm().font_medium().truncate().child(m.title().to_string()))
            .child(div().text_xs().text_color(theme.muted_foreground).truncate().child(sub))
            .into_any_element()
    }
}

impl Render for Watchlist {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let me = cx.entity().downgrade();
        let reload: ui::Handler = Rc::new(move |_, cx| {
            me.update(cx, |w, cx| w.load(cx)).ok();
        });
        let mut page = widgets::page("watchlist").child(widgets::page_head("Watchlist", Some("Synced with your Plex account. Titles this server doesn't have are dimmed.".into()), None, cx));
        let entries = self.entries().cloned();
        if let (Load::Failed(e), Some(_)) = (&self.list, &entries) {
            page = page.child(ui::stale_alert("watchlist-retry", e.clone(), Some(reload.clone()), cx));
        }
        let failures = entries.iter().flatten().filter(|m| m.guid.as_ref().is_some_and(|g| self.failed.contains(g))).count();
        if failures > 0 {
            let text = if failures == 1 { "Couldn't check 1 title against this server, so it isn't dimmed.".to_string() } else { format!("Couldn't check {failures} titles against this server, so they aren't dimmed.") };
            let me = cx.entity().downgrade();
            let retry: ui::Handler = Rc::new(move |_, cx| {
                me.update(cx, |w, cx| w.check(cx)).ok();
            });
            page = page.child(ui::alert("watchlist-check", IconName::TriangleAlert, None, text.into(), Some(("Retry".into(), retry)), cx));
        }
        let Some(list) = entries else {
            return page.child(match &self.list {
                Load::Failed(e) => ui::status_error("watchlist-retry", e.clone(), Some(reload), cx),
                _ => ui::status_loading(),
            });
        };
        if list.is_empty() {
            return page.child(ui::status_empty("Your watchlist is empty", Some("Add movies and shows from their detail page.".into()), cx));
        }
        let cards: Vec<AnyElement> = list.iter().enumerate().map(|(i, m)| self.entry(i, m, cx)).collect();
        page.child(ui::poster_grid(cards))
    }
}
