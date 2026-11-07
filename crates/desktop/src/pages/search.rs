//! Search: the top bar's query across this server's libraries (or every server
//! of the account), plus titles to request through Seerr when it is set up.

use crate::prelude::*;
use std::rc::Rc;
use std::time::Duration;

use futures::StreamExt as _;
use gpui_kit::component::switch::Switch;
use patina_core::ops::xsearch::{self, Group, Results, ServerCopy, ServerState, ServerStatus};
use patina_core::seerr::{self, Availability, SeerrTitle};
use patina_plex::{Directory, Hub};

use crate::art::art_of;
use crate::core::{bg, core, message};
use crate::format::{self, Shape};
use crate::nav::{self, Route};
use crate::pages::browse::ui::{self, Tone};
use crate::pages::requests::seerr as s;
use crate::store::store;
use crate::widgets;

const PEOPLE: [&str; 6] = ["actor", "director", "writer", "producer", "person", "tag"];

/// What the page last searched for.
#[derive(Clone, PartialEq, Default)]
struct Asked {
    query: String,
    all: bool,
    seerr: bool,
    retry: u64,
}

pub struct Search {
    asked: Asked,
    retry: u64,
    results: Option<(String, Result<Vec<Hub>, String>)>,
    /// Results of a search across every server; they grow as servers answer.
    everywhere: Option<(String, Option<Results>)>,
    requestable: Option<(String, Result<Vec<SeerrTitle>, String>)>,
    generation: u64,
    _subs: Vec<Subscription>,
}

impl Search {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subs = vec![cx.observe(&store(cx), |this: &mut Self, _, cx| {
            this.update(cx);
            cx.notify();
        })];
        let mut this = Self { asked: Asked::default(), retry: 0, results: None, everywhere: None, requestable: None, generation: 0, _subs: subs };
        this.asked.query = "\0".into();
        this.update(cx);
        this
    }

    fn query(cx: &App) -> String {
        store(cx).read(cx).search.trim().to_string()
    }

    /// Searching every server needs a second one on the account.
    fn can_all(cx: &App) -> bool {
        store(cx).read(cx).session.as_ref().is_some_and(|s| s.servers.len() > 1)
    }

    fn all(cx: &App) -> bool {
        store(cx).read(cx).search_all && Self::can_all(cx)
    }

    /// Start a new search when the query (or where to look) changed: debounced, so a newer
    /// keystroke cancels the pending one.
    fn update(&mut self, cx: &mut Context<Self>) {
        let asked = Asked { query: Self::query(cx), all: Self::all(cx), seerr: store(cx).read(cx).seerr, retry: self.retry };
        if asked == self.asked {
            return;
        }
        self.asked = asked.clone();
        self.generation += 1;
        let mine = self.generation;
        let q = asked.query.clone();
        if q.chars().count() < 2 {
            self.results = None;
            self.everywhere = None;
            self.requestable = None;
            return;
        }
        let app = core(cx);
        let wide = asked.all;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(220)).await;
            if this.update(cx, |p, _| p.generation != mine).unwrap_or(true) {
                return;
            }
            if wide {
                this.update(cx, |p, cx| {
                    p.everywhere = Some((q.clone(), None));
                    cx.notify();
                })
                .ok();
                let (tx, mut rx) = futures::channel::mpsc::unbounded::<Results>();
                let app = app.clone();
                let query = q.clone();
                let done = bg(async move {
                    xsearch::search_all(&app, &query, move |r| {
                        let _ = tx.unbounded_send(r.clone());
                    })
                    .await
                });
                while let Some(r) = rx.next().await {
                    let stale = this
                        .update(cx, |p, cx| {
                            if p.generation != mine {
                                return true;
                            }
                            p.everywhere = Some((q.clone(), Some(r)));
                            cx.notify();
                            false
                        })
                        .unwrap_or(true);
                    if stale {
                        return;
                    }
                }
                let r = done.await;
                this.update(cx, |p, cx| {
                    if p.generation == mine {
                        p.everywhere = Some((q.clone(), Some(r)));
                        cx.notify();
                    }
                })
                .ok();
                return;
            }
            let query = q.clone();
            let result = bg(async move { Ok::<_, anyhow::Error>(app.server()?.search_hubs(&query, 24, None).await?) }).await;
            this.update(cx, |p, cx| {
                if p.generation == mine {
                    let hubs = result.map(|h| h.into_iter().filter(|h| !h.metadata.is_empty() || !h.directory.is_empty()).collect()).map_err(|e| message(&e));
                    p.results = Some((q.clone(), hubs));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();

        // Titles to request through Seerr, when it is set up. Ones already on the server are left out.
        if !asked.seerr {
            self.requestable = None;
            return;
        }
        let app = core(cx);
        let q = asked.query;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(320)).await;
            if this.update(cx, |p, _| p.generation != mine).unwrap_or(true) {
                return;
            }
            let query = q.clone();
            let result = bg(async move { seerr::search(&app, &query, 1).await }).await;
            this.update(cx, |p, cx| {
                if p.generation == mine {
                    let titles = result.map(|r| r.results.into_iter().filter(|t| t.status != Some(Availability::Available)).take(12).collect()).map_err(|e| e.to_string());
                    p.requestable = Some((q.clone(), titles));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn retry_handler(&self, cx: &mut Context<Self>) -> ui::Handler {
        let me = cx.entity().downgrade();
        Rc::new(move |_, cx| {
            me.update(cx, |p, cx| {
                p.retry += 1;
                p.update(cx);
                cx.notify();
            })
            .ok();
        })
    }

    fn everywhere_view(&self, query: &str, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let Some((q, Some(found))) = &self.everywhere else { return ui::status_loading().into_any_element() };
        if q != query {
            return ui::status_loading().into_any_element();
        }
        let strip = h_flex().mb_4().flex_wrap().gap_x_4().gap_y_1p5().text_xs().text_color(theme.muted_foreground).children(found.servers.iter().map(|srv| {
            let failed = matches!(srv.state, ServerState::Failed | ServerState::TimedOut);
            h_flex()
                .gap_1()
                .when(failed, |d| d.text_color(theme.danger))
                .child(div().font_medium().when(!failed, |d| d.text_color(theme.foreground.opacity(0.8))).child(srv.name.clone()))
                .child(state_label(srv))
        }));
        let body = if !found.groups.is_empty() {
            widgets::rows(cx).children(found.groups.iter().enumerate().map(|(i, g)| group_row(i, g, cx))).into_any_element()
        } else if found.done {
            ui::status_empty("No results", Some("None of your servers has it. Check the spelling or try another word.".into()), cx).into_any_element()
        } else {
            ui::status_loading().into_any_element()
        };
        v_flex().child(strip).child(body).into_any_element()
    }

    fn hubs_view(&self, query: &str, cx: &mut Context<Self>) -> AnyElement {
        let Some((q, result)) = &self.results else { return ui::status_loading().into_any_element() };
        if q != query {
            return ui::status_loading().into_any_element();
        }
        let hubs = match result {
            Err(e) => return ui::status_error("search-retry", e.clone(), Some(self.retry_handler(cx)), cx).into_any_element(),
            Ok(h) if h.is_empty() => return ui::status_empty("No results", Some("Check the spelling or try another word.".into()), cx).into_any_element(),
            Ok(h) => h,
        };
        v_flex()
            .children(hubs.iter().enumerate().map(|(i, hub)| {
                let wide = matches!(hub.kind.as_deref(), Some("episode" | "clip"));
                let id = format!("search:{}:{i}", hub.hub_identifier.as_deref().unwrap_or(""));
                v_flex()
                    .child(widgets::section_head(hub.title.clone().unwrap_or_default(), None).when(i == 0, |d| d.mt_0()))
                    .when(!hub.metadata.is_empty(), |d| d.child(ui::item_grid(&id, hub.metadata.clone(), wide, wide.then_some(Shape::Landscape), cx)))
                    .when(!hub.directory.is_empty(), |d| {
                        d.child(h_flex().flex_wrap().gap_x_2p5().gap_y_4().children(hub.directory.iter().enumerate().map(|(j, dir)| tag(&format!("{id}:{j}"), dir, cx))))
                    })
            }))
            .into_any_element()
    }
}

/// What a server's search came to, for the status strip.
fn state_label(s: &ServerStatus) -> String {
    match s.state {
        ServerState::Pending => "searching…".into(),
        ServerState::Done => format::plural(s.count as i64, "title"),
        ServerState::TimedOut => "too slow to answer".into(),
        ServerState::Offline => "offline".into(),
        ServerState::Failed => "unavailable".into(),
    }
}

fn rank(resolution: &str) -> u32 {
    match resolution {
        "4K" => 2160,
        "SD" => 480,
        r => r.trim_end_matches(|c: char| !c.is_ascii_digit()).parse().unwrap_or(0),
    }
}

/// The best resolution among a copy's files ("4K", "1080p"), if the server reports any.
fn quality(copy: &ServerCopy) -> Option<&str> {
    copy.versions.iter().filter_map(|v| v.resolution.as_deref()).fold(None, |best, r| match best {
        Some(b) if rank(b) >= rank(r) => Some(b),
        _ => Some(r),
    })
}

/// A copy needs no choice when the title is on the connected server only, in one file.
fn needs_choice(g: &Group) -> bool {
    g.copies.len() > 1 || g.copies.first().is_some_and(|c| c.versions.len() > 1 || !c.current)
}

fn group_row(i: usize, g: &Group, cx: &App) -> AnyElement {
    let theme = cx.theme().clone();
    let Some(first) = g.copies.first() else { return div().into_any_element() };
    // Several copies (or files) to choose from: ask which, else open the only one.
    let choose = needs_choice(g).then(|| g.clone());
    let target = first.item.clone();
    div()
        .id(("search-group", i))
        .child(
            widgets::list_row(i == 0, cx)
                .cursor_pointer()
                .hover(|s| s.bg(theme.muted.opacity(0.4)))
                .child(
                    div()
                        .flex_none()
                        .w(px(32.))
                        .h(px(48.))
                        .overflow_hidden()
                        .rounded(px(4.))
                        .bg(theme.muted)
                        .children(art_of(first.item.poster(), 48., 72., first.item.server_id()).map(|i| i.size_full().object_fit(ObjectFit::Cover))),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            h_flex()
                                .text_sm()
                                .child(div().truncate().child(g.title.clone()))
                                .when_some(g.year, |d, y| d.child(div().ml_1().flex_none().text_color(theme.muted_foreground).child(format!("({y})")))),
                        )
                        .child(div().text_xs().truncate().text_color(theme.muted_foreground).child(g.subtitle.clone().unwrap_or_else(|| g.kind.clone()))),
                )
                .child(h_flex().max_w(relative(0.5)).flex_wrap().justify_end().gap_1().children(g.copies.iter().map(|c| {
                    let label = match quality(c) {
                        Some(q) => format!("{} {q}", c.server_name),
                        None => c.server_name.clone(),
                    };
                    ui::badge(label, if c.current { Tone::Tint } else { Tone::Plain }, cx)
                }))),
        )
        .on_click(move |_, window, cx| match &choose {
            Some(g) => crate::dialogs::copy::open(g.clone(), window, cx),
            None => nav::push(Route::for_item(&target), cx),
        })
        .into_any_element()
}

/// A tag result: a person (round photo or initials) or a pill button.
fn tag(id: &str, d: &Directory, cx: &App) -> AnyElement {
    let theme = cx.theme().clone();
    let name = d.title.clone().or_else(|| d.tag.clone()).unwrap_or_default();
    let key = d.key.clone();
    let title = name.clone();
    let open = move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
        if let Some(k) = &key {
            nav::push(Route::browse(k.clone(), title.clone()), cx);
        }
    };
    let id = ElementId::Name(id.to_string().into());
    if !PEOPLE.contains(&d.kind.as_deref().unwrap_or("")) {
        return Button::new(id).outline().small().rounded_full().label(name).on_click(open).into_any_element();
    }
    let group: SharedString = format!("person-{id:?}").into();
    v_flex()
        .id(id)
        .group(group.clone())
        .w(px(96.))
        .items_center()
        .gap_0p5()
        .text_center()
        .text_xs()
        .cursor_pointer()
        .on_click(open)
        .child(
            div()
                .mb_1p5()
                .size(px(72.))
                .rounded_full()
                .overflow_hidden()
                .bg(theme.muted)
                .border_1()
                .border_color(theme.foreground.opacity(0.1))
                .group_hover(group.clone(), |s| s.border_color(theme.foreground.opacity(0.3)))
                .flex()
                .items_center()
                .justify_center()
                .font_semibold()
                .text_color(theme.muted_foreground)
                .child(match art_of(d.thumb.as_deref(), 80., 80., None) {
                    Some(img) => img.size_full().object_fit(ObjectFit::Cover).into_any_element(),
                    None => format::initials(&name).into_any_element(),
                }),
        )
        .child(div().font_medium().group_hover(group, |s| s.underline()).child(name))
        .when_some(d.count, |c, n| c.child(div().text_color(theme.muted_foreground).child(format!("{n} titles"))))
        .into_any_element()
}

impl Render for Search {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let query = Self::query(cx);
        let all = Self::all(cx);
        let title = if query.is_empty() { "Search".to_string() } else { format!("“{query}”") };
        let sub = format!("{} Press / anywhere to search.", if all { "Every library on every server of your account." } else { "Every library on this server." });
        let toggle = Self::can_all(cx).then(|| {
            let on = store(cx).read(cx).search_all;
            Switch::new("search-all-servers")
                .checked(on)
                .label("Search all servers")
                .on_click(|on, _, cx| {
                    let on = *on;
                    store(cx).update(cx, |s, cx| {
                        s.search_all = on;
                        cx.notify();
                    });
                })
                .into_any_element()
        });
        let mut page = widgets::page("search").child(widgets::page_head(title, Some(sub.into()), toggle, cx));
        page = page.child(if all && query.chars().count() >= 2 {
            self.everywhere_view(&query, cx)
        } else if query.chars().count() < 2 {
            ui::status_empty("Find something to watch", Some("Movies, shows, episodes, music, actors, directors, genres…".into()), cx).into_any_element()
        } else {
            self.hubs_view(&query, cx)
        });
        if let Some((q, found)) = &self.requestable
            && *q == query
            && (found.is_err() || found.as_ref().is_ok_and(|t| !t.is_empty()))
        {
            page = page.child(widgets::section_head("Request with Seerr", None)).child(match found {
                Err(e) => div().text_sm().text_color(theme.muted_foreground).child(format!("Seerr could not search: {e}")).into_any_element(),
                Ok(titles) => {
                    let w = ui::card_width(Shape::Poster, cx);
                    ui::poster_grid(titles.iter().map(|t| s::card(ElementId::Name(format!("seerr:{}:{}", t.media_type.as_str(), t.tmdb_id).into()), t, w, cx))).into_any_element()
                }
            });
        }
        page
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use patina_core::ops::xsearch::Version;
    use patina_plex::Metadata;
    use std::prelude::v1::test;

    fn copy(current: bool, resolutions: &[&str]) -> ServerCopy {
        ServerCopy {
            server_id: "s".into(),
            server_name: "Alpha".into(),
            current,
            library: None,
            item: Metadata::default(),
            versions: resolutions
                .iter()
                .map(|r| Version { label: String::new(), resolution: Some(r.to_string()), video_codec: None, container: None, bitrate: None, size: None, hdr: None })
                .collect(),
        }
    }

    #[test]
    fn quality_is_the_best_resolution() {
        assert_eq!(quality(&copy(true, &["720p", "4K", "1080p"])), Some("4K"));
        assert_eq!(quality(&copy(true, &["SD", "1080p"])), Some("1080p"));
        assert_eq!(quality(&copy(true, &[])), None);
    }

    #[test]
    fn one_local_file_needs_no_choice() {
        let group = |copies| Group { key: String::new(), title: String::new(), year: None, kind: String::new(), subtitle: None, copies };
        assert!(!needs_choice(&group(vec![copy(true, &["1080p"])])));
        assert!(needs_choice(&group(vec![copy(false, &["1080p"])])));
        assert!(needs_choice(&group(vec![copy(true, &["1080p", "4K"])])));
        assert!(needs_choice(&group(vec![copy(true, &[]), copy(false, &[])])));
    }
}
