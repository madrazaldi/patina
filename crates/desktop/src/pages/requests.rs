//! Seerr: discover titles to request, and follow (or manage) requests.

pub mod seerr;

use crate::prelude::*;
use std::collections::HashSet;
use std::rc::Rc;

use gpui_kit::component::tab::{Tab as KitTab, TabBar};
use patina_core::seerr::{self as core_seerr, DiscoverList, RequestFilter, SeerrPage, SeerrRequest, SeerrStatus, SeerrTitle};

use self::seerr::RowEvents;
use crate::core::{bg, core};
use crate::format::Shape;
use crate::nav::{self, Route};
use crate::pages::browse::ui;
use crate::store::store;
use crate::widgets;

const LISTS: [(&str, &str); 5] = [
    ("trending", "Trending"),
    ("movies", "Popular movies"),
    ("tv", "Popular shows"),
    ("upcoming-movies", "Upcoming movies"),
    ("upcoming-tv", "Upcoming shows"),
];

const FILTERS: [(RequestFilter, &str); 6] = [
    (RequestFilter::All, "All"),
    (RequestFilter::Pending, "Pending approval"),
    (RequestFilter::Approved, "Approved"),
    (RequestFilter::Processing, "Processing"),
    (RequestFilter::Available, "Available"),
    (RequestFilter::Failed, "Failed"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Discover,
    Requests,
}

pub struct Requests {
    tab: Tab,
    /// The instance and account the status is for.
    status_key: String,
    status: Option<SeerrStatus>,
    status_error: Option<String>,
    list: usize,
    /// Discover pages accumulate with "Load more"; switching lists starts over.
    titles: Vec<SeerrTitle>,
    shown: (Option<usize>, u32, u32),
    loading: bool,
    discover_error: Option<String>,
    generation: u64,
    filter: RequestFilter,
    page: u32,
    requests: Option<SeerrPage<SeerrRequest>>,
    requests_loading: bool,
    requests_error: Option<String>,
    requests_generation: u64,
    busy: HashSet<i64>,
    _subs: Vec<Subscription>,
}

impl Requests {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subs = vec![cx.observe(&store(cx), |this: &mut Self, _, cx| {
            if this.key(cx) != this.status_key {
                this.reset(cx);
            }
            cx.notify();
        })];
        let mut this = Self {
            tab: Tab::Discover,
            status_key: String::new(),
            status: None,
            status_error: None,
            list: 0,
            titles: Vec::new(),
            shown: (None, 0, 0),
            loading: false,
            discover_error: None,
            generation: 0,
            filter: RequestFilter::All,
            page: 1,
            requests: None,
            requests_loading: false,
            requests_error: None,
            requests_generation: 0,
            busy: HashSet::new(),
            _subs: subs,
        };
        this.reset(cx);
        this
    }

    fn key(&self, cx: &App) -> String {
        let s = store(cx).read(cx);
        if s.seerr { format!("{}|{}", s.settings.seerr.url.trim(), s.scope()) } else { String::new() }
    }

    /// Seerr was set up, changed or removed: start over.
    fn reset(&mut self, cx: &mut Context<Self>) {
        self.status_key = self.key(cx);
        self.status = None;
        self.status_error = None;
        self.titles.clear();
        self.shown = (None, 0, 0);
        self.requests = None;
        self.load_status(cx);
        self.ensure(cx);
    }

    fn load_status(&mut self, cx: &mut Context<Self>) {
        let Some(fut) = seerr::status(cx) else { return };
        let key = self.status_key.clone();
        cx.spawn(async move |this, cx| {
            let result = fut.await;
            this.update(cx, |p, cx| {
                if p.status_key != key {
                    return;
                }
                match result {
                    Ok(s) => p.status = Some(s),
                    Err(e) => {
                        seerr::forget_status(cx);
                        p.status_error = Some(e);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Load what the current tab shows, if it isn't loaded yet.
    fn ensure(&mut self, cx: &mut Context<Self>) {
        if !store(cx).read(cx).seerr {
            return;
        }
        match self.tab {
            Tab::Discover if self.shown.0 != Some(self.list) && !self.loading => {
                self.titles.clear();
                self.load(self.list, 1, cx);
            }
            Tab::Requests if self.requests.is_none() && !self.requests_loading => self.load_requests(cx),
            _ => {}
        }
    }

    fn load(&mut self, list: usize, page: u32, cx: &mut Context<Self>) {
        self.generation += 1;
        let mine = self.generation;
        self.loading = true;
        self.discover_error = None;
        let app = core(cx);
        let which = LISTS[list].0;
        cx.spawn(async move |this, cx| {
            let result = bg(async move {
                let list: DiscoverList = which.parse()?;
                core_seerr::discover(&app, list, page).await
            })
            .await;
            this.update(cx, |p, cx| {
                if mine != p.generation {
                    return;
                }
                p.loading = false;
                match result {
                    Ok(got) => {
                        let fresh = fresh_titles(if page == 1 { &[] } else { &p.titles }, got.results);
                        if page == 1 {
                            p.titles = fresh;
                        } else {
                            p.titles.extend(fresh);
                        }
                        p.shown = (Some(list), got.page, got.pages);
                    }
                    Err(e) => p.discover_error = Some(e.to_string()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn load_requests(&mut self, cx: &mut Context<Self>) {
        self.requests_generation += 1;
        let mine = self.requests_generation;
        self.requests_loading = true;
        self.requests_error = None;
        let (filter, page) = (self.filter, self.page);
        let app = core(cx);
        cx.spawn(async move |this, cx| {
            let result = bg(async move { core_seerr::requests(&app, filter, page, false).await }).await;
            this.update(cx, |p, cx| {
                if mine != p.requests_generation {
                    return;
                }
                p.requests_loading = false;
                match result {
                    Ok(r) => {
                        // Deleting the last request of the last page leaves it empty: step back to the new last page.
                        let back = r.page == p.page && r.results.is_empty() && p.page > 1;
                        let last = r.pages;
                        p.requests = Some(r);
                        if back {
                            p.page = (p.page - 1).min(last).max(1);
                            p.load_requests(cx);
                        }
                    }
                    Err(e) => p.requests_error = Some(e.to_string()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn set_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.tab = tab;
        if tab == Tab::Requests {
            // Like the web app's resource: refetched whenever the tab is shown.
            self.load_requests(cx);
        }
        self.ensure(cx);
        cx.notify();
    }

    fn row_events(&self, cx: &mut Context<Self>) -> RowEvents {
        let me = cx.entity().downgrade();
        let me2 = me.clone();
        RowEvents {
            busy: Rc::new(move |id, on, cx| {
                me.update(cx, |p, cx| {
                    if on {
                        p.busy.insert(id);
                    } else {
                        p.busy.remove(&id);
                    }
                    cx.notify();
                })
                .ok();
            }),
            changed: Rc::new(move |cx| {
                me2.update(cx, |p, cx| p.load_requests(cx)).ok();
            }),
        }
    }

    fn discover(&self, cx: &mut Context<Self>) -> AnyElement {
        let me = cx.entity().downgrade();
        let pick = me.clone();
        let mut body = v_flex().child(
            h_flex().mb_5().child(
                TabBar::new("seerr-lists")
                    .segmented()
                    .small()
                    .selected_index(self.list)
                    .on_click(move |ix: &usize, _, cx| {
                        let ix = *ix;
                        pick.update(cx, |p, cx| {
                            if p.list != ix {
                                p.list = ix;
                                p.ensure(cx);
                                cx.notify();
                            }
                        })
                        .ok();
                    })
                    .children(LISTS.iter().map(|(_, label)| KitTab::new().label(*label))),
            ),
        );
        let retry: ui::Handler = {
            let me = me.clone();
            Rc::new(move |_, cx| {
                me.update(cx, |p, cx| {
                    p.titles.clear();
                    p.load(p.list, 1, cx);
                })
                .ok();
            })
        };
        if self.titles.is_empty() {
            return body
                .child(match &self.discover_error {
                    Some(e) => ui::status_error("discover-retry", e.clone(), Some(retry), cx),
                    None if self.loading || self.shown.0 != Some(self.list) => ui::status_loading(),
                    None => ui::status_empty("Nothing here", Some("Seerr returned no titles for this list.".into()), cx),
                })
                .into_any_element();
        }
        let w = ui::card_width(Shape::Poster, cx);
        body = body.child(ui::poster_grid(self.titles.iter().map(|t| seerr::card(ElementId::Name(format!("discover:{}:{}", t.media_type.as_str(), t.tmdb_id).into()), t, w, cx))));
        if let Some(e) = &self.discover_error {
            body = body.child(div().mt_4().text_center().text_sm().text_color(cx.theme().danger).child(e.clone()));
        }
        let (_, page, pages) = self.shown;
        if page < pages {
            let list = self.list;
            body = body.child(
                h_flex().mt_6().justify_center().child(
                    Button::new("more")
                        .outline()
                        .label(if self.loading { "Loading…" } else { "Load more" })
                        .disabled(self.loading)
                        .on_click(move |_, _, cx| {
                            me.update(cx, |p, cx| p.load(list, page + 1, cx)).ok();
                        }),
                ),
            );
        }
        body.into_any_element()
    }

    fn requests_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let me = cx.entity().downgrade();
        let options = FILTERS.iter().map(|(f, l)| (*f, SharedString::from(*l))).collect();
        let pick = me.clone();
        let mut body = v_flex().child(h_flex().mb_4().gap_3().child(ui::select("request-filter", Some(px(208.)), options, &self.filter, false, move |f, _, cx| {
            pick.update(cx, |p, cx| {
                p.filter = f;
                p.page = 1;
                p.load_requests(cx);
                cx.notify();
            })
            .ok();
        })));
        let retry: ui::Handler = {
            let me = me.clone();
            Rc::new(move |_, cx| {
                me.update(cx, |p, cx| p.load_requests(cx)).ok();
            })
        };
        let Some(r) = &self.requests else {
            return body
                .child(match &self.requests_error {
                    Some(e) => ui::status_error("requests-retry", e.clone(), Some(retry), cx),
                    None => ui::status_loading(),
                })
                .into_any_element();
        };
        if let Some(e) = &self.requests_error {
            body = body.child(ui::stale_alert("requests-retry", e.clone(), Some(retry), cx));
        }
        if r.results.is_empty() {
            let text = if self.filter == RequestFilter::All { "Requests made in Patina or on the Seerr website show up here." } else { "No requests match this filter." };
            return body.child(ui::status_empty("No requests", Some(text.into()), cx)).into_any_element();
        }
        let events = self.row_events(cx);
        let can = self.status.as_ref().map(|s| s.can);
        body = body.child(
            widgets::rows(cx).children(r.results.iter().enumerate().map(|(i, req)| seerr::request_row(req, can, true, i == 0, self.busy.contains(&req.id), events.clone(), cx))),
        );
        if r.pages > 1 {
            let (page, pages) = (self.page, r.pages);
            let (older, newer) = (me.clone(), me);
            body = body.child(
                h_flex()
                    .mt_5()
                    .gap_3p5()
                    .items_center()
                    .justify_center()
                    .child(Button::new("newer").ghost().small().icon(IconName::ChevronLeft).label("Newer").disabled(page <= 1).on_click(move |_, _, cx| {
                        newer.update(cx, |p, cx| p.go_page(page - 1, cx)).ok();
                    }))
                    .child(div().text_sm().text_color(cx.theme().muted_foreground).child(format!("Page {} of {}", r.page, r.pages)))
                    .child(
                        Button::new("older")
                            .ghost()
                            .small()
                            .child(h_flex().gap_1().child("Older").child(Icon::new(IconName::ChevronRight).size_4()))
                            .disabled(page >= pages)
                            .on_click(move |_, _, cx| {
                                older.update(cx, |p, cx| p.go_page(page + 1, cx)).ok();
                            }),
                    ),
            );
        }
        body.into_any_element()
    }

    fn go_page(&mut self, page: u32, cx: &mut Context<Self>) {
        self.page = page.max(1);
        self.load_requests(cx);
        cx.notify();
    }
}

/// Titles of a new discover page not already shown: TMDB pages can overlap as popularity shifts.
fn fresh_titles(shown: &[SeerrTitle], got: Vec<SeerrTitle>) -> Vec<SeerrTitle> {
    let seen: HashSet<(&str, i64)> = shown.iter().map(|t| (t.media_type.as_str(), t.tmdb_id)).collect();
    got.into_iter().filter(|t| !seen.contains(&(t.media_type.as_str(), t.tmdb_id))).collect()
}

impl Render for Requests {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (seerr_on, url) = {
            let s = store(cx).read(cx);
            (s.seerr, s.settings.seerr.url.trim().to_string())
        };
        let sub: SharedString = match &self.status {
            Some(s) => format!("Seerr at {} · signed in as {}", s.url, s.user.name).into(),
            None if seerr_on => format!("Seerr at {url}").into(),
            None => "Request movies and shows the server doesn't have.".into(),
        };
        let tabs = seerr_on.then(|| {
            let me = cx.entity().downgrade();
            let mine = if self.status.as_ref().is_some_and(|s| s.can.manage) { "All requests" } else { "My requests" };
            ui::tabs(
                "requests-tab",
                vec!["Discover".into(), mine.into()],
                if self.tab == Tab::Discover { 0 } else { 1 },
                move |i, _, cx| {
                    me.update(cx, |p, cx| p.set_tab(if i == 0 { Tab::Discover } else { Tab::Requests }, cx)).ok();
                },
                cx,
            )
            .into_any_element()
        });
        let mut page = widgets::page("requests").child(widgets::page_head("Requests", Some(sub), tabs, cx));
        if !seerr_on {
            return page.child(
                v_flex()
                    .w_full()
                    .p_6()
                    .py(px(48.))
                    .items_center()
                    .gap_2()
                    .rounded(px(12.))
                    .border_1()
                    .border_color(theme.border)
                    .child(div().size_10().mb_2().rounded(px(8.)).bg(theme.muted).flex().items_center().justify_center().child(Icon::new(IconName::Inbox).size_5()))
                    .child(div().text_lg().font_medium().child("Seerr is not set up"))
                    .child(div().text_sm().text_color(theme.muted_foreground).child("Connect a Seerr, Overseerr or Jellyseerr server in Settings to request titles."))
                    .child(Button::new("open-settings").outline().mt_4().label("Open Settings").on_click(|_, _, cx| nav::push(Route::Settings, cx))),
            );
        }
        if let Some(e) = &self.status_error {
            page = page.child(ui::stale_alert("status-retry", e.clone(), None, cx));
        }
        page.child(match self.tab {
            Tab::Discover => self.discover(cx),
            Tab::Requests => self.requests_view(cx),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use patina_core::seerr::SeerrMediaType;
    use std::prelude::v1::test;

    fn title(media_type: SeerrMediaType, tmdb_id: i64) -> SeerrTitle {
        SeerrTitle { media_type, tmdb_id, title: String::new(), year: None, overview: String::new(), poster: None, backdrop: None, rating: None, status: None, status_4k: None, rating_key: None }
    }

    #[test]
    fn overlapping_discover_pages_add_only_new_titles() {
        let shown = vec![title(SeerrMediaType::Movie, 1), title(SeerrMediaType::Tv, 2)];
        let got = vec![title(SeerrMediaType::Movie, 1), title(SeerrMediaType::Movie, 2), title(SeerrMediaType::Tv, 3)];
        let fresh: Vec<i64> = fresh_titles(&shown, got).iter().map(|t| t.tmdb_id).collect();
        assert_eq!(fresh, vec![2, 3]);
    }
}
