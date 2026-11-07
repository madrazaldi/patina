//! An item's detail page: artwork, facts, actions, track pickers, its children
//! (seasons, episodes, tracks, collection items), cast and related hubs.

mod children;
pub(crate) use children::{DragLabel, Dragged, moved};
mod streams;


use crate::prelude::*;
use gpui_kit::component::menu::DropdownMenu as _;
use patina_core::downloads::{Download, State};
use patina_core::ops::{library, lists};
use patina_core::{downloads, prefs, servers};
use patina_plex::{Hub, Metadata, MetadataOptions, Tag};

use self::children::Children;
use crate::actions::{self, PlayOptions};
use crate::art::art_of;
use crate::core::{Load, bg, blocking, core};
use crate::dialogs::{self, form};
use crate::format::{self, Shape};
use crate::menu::{self, item_menu};
use crate::nav::{self, Route};
use crate::store::{self, store};
use crate::theme::GUTTER;
use crate::widgets::{self, CardRow, hub::HubRow};

/// Parent links above the title: (label, rating key).
fn crumbs(item: &Metadata) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let k = item.kind();
    if matches!(k, "episode" | "track")
        && let Some(rk) = &item.grandparent_rating_key
    {
        out.push((item.grandparent_title.clone().unwrap_or_default(), rk.clone()));
    }
    if matches!(k, "episode" | "track" | "season" | "album")
        && let Some(rk) = &item.parent_rating_key
    {
        out.push((item.parent_title.clone().unwrap_or_default(), rk.clone()));
    }
    out
}

/// "S01E02 · Mar 4, 2024 · 48m", "2015 · 1h 23m · 2 seasons · 6 episodes".
fn meta_line(item: &Metadata) -> Vec<String> {
    let k = item.kind();
    let mut out = Vec::new();
    if k == "episode" {
        out.push(item.episode_code());
        if let Some(d) = &item.originally_available_at {
            out.push(format::air_date(d));
        }
    } else if let Some(y) = item.year {
        out.push(y.to_string());
    }
    if let Some(d) = item.duration
        && k != "show"
        && k != "season"
    {
        out.push(format::duration(d));
    }
    if k == "show"
        && let Some(n) = item.child_count
    {
        out.push(format!("{n} season{}", if n == 1 { "" } else { "s" }));
    }
    if let Some(n) = item.leaf_count
        && matches!(k, "show" | "season" | "album")
    {
        out.push(format!("{n} {}", if k == "album" { "tracks" } else { "episodes" }));
    }
    out.retain(|s| !s.is_empty());
    out
}

/// Critic and audience ratings: "★ 7.6", or "76% critics" from Rotten Tomatoes.
fn ratings(item: &Metadata) -> Vec<String> {
    let rt = |i: &Option<String>| i.as_deref().is_some_and(|i| i.contains("rottentomatoes"));
    let mut out = Vec::new();
    if let Some(r) = item.rating {
        out.push(if rt(&item.rating_image) { format!("{}% critics", (r * 10.).round()) } else { format!("★ {r:.1}") });
    }
    if let Some(r) = item.audience_rating {
        out.push(if rt(&item.audience_rating_image) { format!("{}% audience", (r * 10.).round()) } else { format!("★ {r:.1}") });
    }
    out
}

/// The tag lists shown beside the summary.
fn tag_facts(item: &Metadata) -> Vec<(&'static str, Vec<Tag>)> {
    [
        ("Directed by", &item.directors),
        ("Written by", &item.writers),
        ("Genres", &item.genres),
        ("Collections", &item.collections),
        ("Moods", &item.moods),
        ("Styles", &item.styles),
        ("Country", &item.countries),
        ("Labels", &item.labels),
    ]
    .into_iter()
    .filter(|(_, l)| !l.is_empty())
    .map(|(k, l)| (k, l.iter().take(8).cloned().collect()))
    .collect()
}

/// The plain facts: studio, file, audio, path, added, last watched.
fn plain_facts(item: &Metadata, owned: bool) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    if let Some(s) = item.studio.clone().filter(|s| !s.is_empty()) {
        out.push(("Studio", s));
    }
    if let Some(m) = item.media.first() {
        let p = m.parts.first();
        let bits: Vec<String> = [
            m.container.as_ref().map(|c| c.to_uppercase()),
            m.width.zip(m.height).filter(|(w, h)| *w > 0 && *h > 0).map(|(w, h)| format!("{w}×{h}")),
            m.bitrate.filter(|b| *b > 0).map(format::bitrate),
            p.and_then(|p| p.size).filter(|s| *s > 0).map(format::bytes),
        ]
        .into_iter()
        .flatten()
        .collect();
        if !bits.is_empty() {
            out.push(("File", bits.join(" · ")));
        }
        if let Some(a) = p.and_then(|p| p.selected_audio()) {
            let ch = a.channels.map(patina_plex::channel_label).unwrap_or_default();
            out.push(("Audio", format!("{} {ch}", a.codec.clone().unwrap_or_default().to_uppercase()).trim().to_string()));
        }
        if owned && let Some(f) = p.and_then(|p| p.file.clone()) {
            out.push(("Path", f));
        }
    }
    if let Some(t) = item.added_at.filter(|t| *t > 0) {
        out.push(("Added", format::date(t)));
    }
    if let Some(t) = item.last_viewed_at.filter(|t| *t > 0) {
        let n = item.view_count.unwrap_or(0);
        out.push(("Last watched", format!("{} · {n} play{}", format::ago(t), if n == 1 { "" } else { "s" })));
    }
    out
}

/// Item-page hubs not already shown under Related.
fn extra_hubs(item: &Metadata, hubs: &[Hub]) -> Vec<Hub> {
    let related = item.related_hubs();
    let seen = |h: &Hub| {
        related.iter().any(|r| {
            (h.hub_identifier.is_some() && r.hub_identifier == h.hub_identifier)
                || h.title.as_ref().is_some_and(|t| r.title.as_ref().is_some_and(|rt| rt.eq_ignore_ascii_case(t)))
        })
    };
    hubs.iter().filter(|h| !h.metadata.is_empty() && !seen(h)).cloned().collect()
}

fn download_label(d: Option<&Download>) -> String {
    match d {
        None => "Download".into(),
        Some(d) => match d.state {
            State::Done => "Downloaded — show downloads".into(),
            State::Paused => "Download paused — resume".into(),
            State::Failed => format!("Download failed{} — retry", d.error.as_ref().map(|e| format!(": {e}")).unwrap_or_default()),
            State::Queued => "Waiting to download".into(),
            State::Downloading => "Downloading".into(),
        },
    }
}

/// The collection's sort (2 is custom), which Plex sends as a string or a number.
fn collection_sort(item: &Metadata) -> Option<String> {
    match item.extra.get("collectionSort")? {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

pub struct Item {
    rating_key: String,
    /// Machine id of another account server the item was found on (cross-server search).
    server: Option<String>,
    data: Load<Option<Metadata>>,
    /// The last good item, shown while reloading or after a failed reload.
    stale: Option<Metadata>,
    extra: Vec<Hub>,
    children: Option<Entity<Children>>,
    media_index: usize,
    /// The item whose preferred version was asked for.
    version_of: String,
    expanded: bool,
    /// Watchlist state on plex.tv; none until known.
    watchlisted: Option<bool>,
    hover_star: Option<u8>,
    revision: u64,
    _subs: Vec<Subscription>,
}

impl Item {
    pub fn new(rating_key: String, server: Option<String>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subs = vec![cx.observe(&store(cx), |this: &mut Self, store, cx| {
            let s = store.read(cx);
            if s.revision != this.revision && s.affects_item(&this.rating_key, this.revision) {
                this.load(cx);
            }
            cx.notify();
        })];
        let mut this = Self {
            rating_key,
            server,
            data: Load::Loading,
            stale: None,
            extra: Vec::new(),
            children: None,
            media_index: 0,
            version_of: String::new(),
            expanded: false,
            watchlisted: None,
            hover_star: None,
            revision: 0,
            _subs: subs,
        };
        this.load(cx);
        this
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        self.revision = store(cx).read(cx).revision;
        let app = core(cx);
        let (rk, on) = (self.rating_key.clone(), self.server.clone());
        cx.spawn(async move |this, cx| {
            let result = bg(async move {
                let found = servers::server_for(&app, on.as_deref()).await?.metadata(&rk, MetadataOptions::full()).await?;
                Ok::<_, anyhow::Error>(found.map(|mut m| {
                    servers::tag(&app, std::slice::from_mut(&mut m), on.as_deref());
                    m
                }))
            })
            .await;
            this.update(cx, |p, cx| p.loaded(result, cx)).ok();
        })
        .detach();
    }

    fn loaded(&mut self, result: anyhow::Result<Option<Metadata>>, cx: &mut Context<Self>) {
        self.data = Load::from_result(result);
        if let Load::Ready(Some(item)) = &self.data {
            let item = item.clone();
            let first = self.stale.is_none();
            self.stale = Some(item.clone());
            let kind = item.kind().to_string();
            if matches!(kind.as_str(), "show" | "season" | "artist" | "album" | "collection" | "photoalbum") {
                let sort = collection_sort(&item);
                match &self.children {
                    Some(c) => c.update(cx, |c, cx| c.set_item(item.clone(), sort, cx)),
                    None => self.children = Some(cx.new(|cx| Children::new(item.clone(), sort, cx))),
                }
            }
            self.load_extra(&kind, cx);
            if first {
                self.pick_version(&item, cx);
                self.check_watchlist(&item, cx);
            }
        }
        cx.notify();
    }

    /// More hubs (sonically similar, similar artists, ...), once the kind is known.
    fn load_extra(&mut self, kind: &str, cx: &mut Context<Self>) {
        let app = core(cx);
        let (rk, on, kind) = (self.rating_key.clone(), self.server.clone(), kind.to_string());
        cx.spawn(async move |this, cx| {
            let result = bg(async move {
                if servers::is_current(&app, on.as_deref()) {
                    return library::item_hubs(&app, &rk, &kind).await;
                }
                let mut hubs = library::item_hubs_on(&servers::server_for(&app, on.as_deref()).await?, &rk, &kind).await?;
                for hub in &mut hubs {
                    servers::tag(&app, &mut hub.metadata, on.as_deref());
                }
                Ok(hubs)
            })
            .await;
            if let Ok(hubs) = result {
                this.update(cx, |p, cx| {
                    p.extra = hubs;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// Start on the version the automatic playback preferences pick (resolution, codec, HDR).
    fn pick_version(&mut self, item: &Metadata, cx: &mut Context<Self>) {
        if item.media.len() < 2 || !matches!(item.kind(), "movie" | "episode") || self.version_of == item.rating_key() {
            return;
        }
        self.version_of = item.rating_key().to_string();
        let (app, it, key) = (core(cx), item.clone(), self.version_of.clone());
        let n = item.media.len();
        cx.spawn(async move |this, cx| {
            let i = blocking(move || prefs::preferred_version(&app, &it)).await;
            this.update(cx, |p, cx| {
                if p.version_of == key && i < n {
                    p.media_index = i;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// The plex.tv id for the watchlist button, if it applies.
    fn discover(&self, item: &Metadata, cx: &App) -> Option<String> {
        (self.server.is_none() && store(cx).read(cx).has_tv() && matches!(item.kind(), "movie" | "show")).then(|| item.discover_id().map(str::to_string)).flatten()
    }

    fn check_watchlist(&mut self, item: &Metadata, cx: &mut Context<Self>) {
        self.watchlisted = None;
        let Some(id) = self.discover(item, cx) else { return };
        let app = core(cx);
        cx.spawn(async move |this, cx| {
            if let Ok(on) = bg(async move { lists::watchlisted(&app, &id).await }).await {
                this.update(cx, |p, cx| {
                    p.watchlisted = Some(on);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn toggle_watchlist(&mut self, id: String, cx: &mut Context<Self>) {
        let on = !self.watchlisted.unwrap_or(false);
        let app = core(cx);
        let me = cx.entity().downgrade();
        store::act_then(
            cx,
            async move {
                let tv = app.plex_tv()?;
                if on { tv.add_to_watchlist(&id).await? } else { tv.remove_from_watchlist(&id).await? }
                Ok(())
            },
            move |(), cx| {
                store::toast(cx, if on { "Added to watchlist" } else { "Removed from watchlist" });
                me.update(cx, |p, cx| {
                    p.watchlisted = Some(on);
                    cx.notify();
                })
                .ok();
            },
        );
    }

    fn on_download(item: &Metadata, d: Option<&Download>, cx: &mut App) {
        match d.map(|d| (&d.state, d.id.clone())) {
            Some((State::Done, _)) => nav::push(Route::Downloads, cx),
            Some((State::Failed | State::Paused, id)) => downloads::retry(&core(cx), &id),
            Some(_) => {}
            None => actions::download(item, cx),
        }
    }

    fn delete_collection(item: &Metadata, window: &mut Window, cx: &mut App) {
        let rk = item.rating_key().to_string();
        widgets::confirm(
            format!("Delete “{}”?", item.title()),
            Some("The collection is removed; its items stay in the library.".into()),
            "Delete",
            true,
            window,
            cx,
            move |_, cx| {
                let (app, rk) = (core(cx), rk.clone());
                store::act_then(cx, async move { Ok::<_, anyhow::Error>(app.server()?.delete_collection(&rk).await?) }, |(), cx| {
                    store::toast(cx, "Collection deleted");
                    store(cx).update(cx, |s, cx| s.refresh(cx));
                    nav::back(cx);
                });
            },
        );
    }

    fn set_collection(rk: String, mode: Option<i32>, sort: Option<i32>, cx: &mut App) {
        let app = core(cx);
        store::act(
            cx,
            async move {
                let s = app.server()?;
                if let Some(m) = mode {
                    s.set_collection_mode(&rk, m).await?;
                }
                if let Some(o) = sort {
                    s.set_collection_sort(&rk, o).await?;
                }
                Ok::<_, anyhow::Error>(())
            },
            Some("Collection updated".into()),
            true,
        );
    }

    fn rate(rk: String, value: Option<f64>, cx: &mut App) {
        let app = core(cx);
        store::act(cx, async move { Ok::<_, anyhow::Error>(app.server()?.rate(&rk, value).await?) }, None, true);
    }

    fn header(&self, item: &Metadata, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let s = store(cx).read(cx);
        let tint = s.tint();
        let kind = item.kind().to_string();
        let shape = format::shape_for(item);
        let spoiler = s.spoilers() && kind == "episode" && !item.is_watched();
        let saved = menu::saved(s, item);
        let server_name = self.server.as_ref().map(|id| s.session.as_ref().and_then(|x| x.servers.iter().find(|v| v.id == *id)).map_or("another server".to_string(), |v| v.name.clone()));
        let foreign = self.server.clone();
        let (pw, ph, source) = match shape {
            Shape::Landscape => (380., 214., item.thumb.as_deref()),
            Shape::Square => (240., 240., item.thumb.as_deref().or(item.parent_thumb.as_deref())),
            Shape::Poster => (210., 315., item.thumb.as_deref()),
        };
        let poster = div()
            .flex_none()
            .w(px(pw))
            .h(px(ph))
            .rounded(px(12.))
            .overflow_hidden()
            .bg(theme.muted)
            .border_1()
            .border_color(theme.border)
            .shadow_2xl()
            .children(art_of(source, pw, ph, item.server_id()).map(|i| i.size_full().object_fit(ObjectFit::Cover).when(spoiler, |i| i.opacity(0.15))));

        let mut bits: Vec<String> = ratings(item);
        bits.extend(meta_line(item));
        let media_label = item.media.get(self.media_index).or(item.media.first()).map(|m| m.label()).unwrap_or_default();
        let meta = h_flex()
            .mt_3p5()
            .flex_wrap()
            .items_center()
            .gap_x_2()
            .gap_y_2()
            .text_sm()
            .text_color(theme.muted_foreground)
            .children(bits.into_iter().enumerate().flat_map(|(i, b)| {
                let dot = (i > 0).then(|| div().text_color(theme.muted_foreground.opacity(0.5)).child("·").into_any_element());
                dot.into_iter().chain(std::iter::once(div().child(b).into_any_element()))
            }))
            .when_some(item.content_rating.clone().filter(|c| !c.is_empty()), |d, c| d.child(div().ml_1().child(form::badge(c, None, cx))))
            .when(!media_label.is_empty(), |d| d.child(div().px_2().h_5().flex().items_center().rounded_full().bg(theme.secondary).text_xs().font_medium().text_color(theme.foreground).child(media_label)))
            .when_some(server_name, |d, n| d.child(form::badge(format!("On {n}"), None, cx)));

        let crumbs = crumbs(item);
        let crumb_row = (!crumbs.is_empty()).then(|| {
            h_flex().mb_3().gap_2().items_center().text_sm().text_color(theme.muted_foreground).children(crumbs.into_iter().enumerate().flat_map(|(i, (label, rk))| {
                let server = foreign.clone();
                let sep = (i > 0).then(|| div().text_color(theme.muted_foreground.opacity(0.5)).child("/").into_any_element());
                let link = div()
                    .id(("crumb", i))
                    .cursor_pointer()
                    .hover(|s| s.underline())
                    .child(label)
                    .on_click(move |_, _, cx| nav::push(Route::Item { rating_key: rk.clone(), server: server.clone() }, cx))
                    .into_any_element();
                sep.into_iter().chain(std::iter::once(link))
            }))
        });

        v_flex()
            .flex_1()
            .min_w_0()
            .pb_1()
            .children(crumb_row)
            .child(div().text_size(px(36.)).line_height(px(39.)).font_semibold().child(item.title().to_string()))
            .when_some(item.original_title.clone().filter(|o| Some(o) != item.title.as_ref() && !o.is_empty()), |d, o| {
                d.child(div().mt_1p5().text_lg().text_color(theme.muted_foreground).child(o))
            })
            .child(meta)
            .child(self.actions(item, saved, tint, cx))
            .into_any_element()
            .map(|right| h_flex().items_end().gap_9().child(poster).child(right).into_any_element())
    }

    fn actions(&self, item: &Metadata, saved: Option<Download>, tint: Hsla, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let kind = item.kind().to_string();
        let foreign = self.server.is_some();
        let progress = item.progress();
        let left = format::remaining(item);
        let watched = item.is_watched();
        let media_index = self.media_index;
        let icon = |id: &'static str, icon: IconName, tip: &str| Button::new(id).outline().large().icon(icon).tooltip(tip.to_string());
        let mut row = h_flex().mt_6().flex_wrap().items_center().gap_2();
        if format::can_play(item) {
            let it = item.clone();
            row = row.child(
                Button::new("play")
                    .primary()
                    .large()
                    .icon(IconName::Play)
                    .label(match &left {
                        Some(l) if progress.is_some() => format!("Resume  ·  {l}"),
                        _ if progress.is_some() => "Resume".into(),
                        _ => "Play".into(),
                    })
                    .on_click(move |_, _, cx| actions::play_with(&it, PlayOptions { media_index: Some(media_index), ..Default::default() }, cx)),
            );
            if progress.is_some() {
                let it = item.clone();
                row = row.child(Button::new("from-start").outline().large().icon(IconName::RotateCcw).label("From start").on_click(move |_, _, cx| {
                    actions::play_with(&it, PlayOptions { resume: false, media_index: Some(media_index), ..Default::default() }, cx)
                }));
            }
            if item.is_container() {
                let it = item.clone();
                row = row.child(Button::new("shuffle").outline().large().icon(IconName::Shuffle).label("Shuffle").on_click(move |_, _, cx| {
                    actions::play_with(&it, PlayOptions { shuffle: true, ..Default::default() }, cx)
                }));
            }
            if matches!(kind.as_str(), "artist" | "album") {
                let it = item.clone();
                row = row.child(Button::new("radio").outline().large().icon(IconName::Radio).label("Radio").on_click(move |_, _, cx| {
                    actions::play_with(&it, PlayOptions { radio: true, ..Default::default() }, cx)
                }));
            }
        }
        let mut group = h_flex().ml_1().gap_1();
        if format::has_watch_state(item) {
            let it = item.clone();
            group = group.child(
                icon("watched", IconName::Check, if watched { "Mark as unwatched" } else { "Mark as watched" })
                    .when(watched, |b| b.text_color(tint))
                    .on_click(move |_, _, cx| actions::set_watched(&it, !watched, cx)),
            );
        }
        if let Some(id) = self.discover(item, cx) {
            let on = self.watchlisted == Some(true);
            group = group.child(
                icon("watchlist", if on { IconName::BookmarkCheck } else { IconName::Bookmark }, if on { "Remove from watchlist" } else { "Add to watchlist" })
                    .when(on, |b| b.text_color(tint))
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_watchlist(id.clone(), cx))),
            );
        }
        if format::supports_tracks(item) && !foreign {
            let (a, b) = (item.clone(), item.clone());
            group = group
                .child(icon("tracks", IconName::Captions, "Audio & subtitles for many episodes").on_click(move |_, window, cx| dialogs::tracks::open(a.clone(), window, cx)))
                .child(icon("prefs", IconName::SlidersHorizontal, "Playback preferences for this show").on_click(move |_, window, cx| dialogs::show_prefs::open(b.clone(), window, cx)));
        }
        if format::can_play(item) && !matches!(kind.as_str(), "photo" | "photoalbum") {
            let it = item.clone();
            group = group.child(icon("next", IconName::ListStart, "Play next").on_click(move |_, _, cx| actions::enqueue(&it, true, cx)));
        }
        if kind != "collection" && kind != "playlist" && !foreign {
            let it = item.clone();
            group = group.child(icon("playlist", IconName::ListPlus, "Add to playlist").on_click(move |_, window, cx| dialogs::playlist::open(vec![it.clone()], window, cx)));
        }
        if menu::DOWNLOADABLE.contains(&kind.as_str()) && !foreign {
            let busy = saved.as_ref().is_some_and(|d| matches!(d.state, State::Queued | State::Downloading));
            let done = saved.as_ref().is_some_and(|d| matches!(d.state, State::Done));
            let tip = download_label(saved.as_ref());
            let it = item.clone();
            let mut b = Button::new("download").outline().large().tooltip(tip);
            b = if done {
                b.icon(IconName::Check).text_color(tint)
            } else if busy {
                let d = saved.as_ref();
                let pct = d.filter(|d| d.size > 0).map_or("…".to_string(), |d| format!("{}%", d.received * 100 / d.size));
                b.label(pct).text_xs().text_color(theme.muted_foreground)
            } else {
                b.icon(IconName::Download)
            };
            group = group.child(b.on_click(move |_, _, cx| {
                if !busy {
                    Self::on_download(&it, saved.as_ref(), cx)
                }
            }));
        }
        if matches!(kind.as_str(), "show" | "season" | "playlist") && !foreign {
            let it = item.clone();
            group = group.child(icon("keep", IconName::CloudDownload, "Keep next unwatched downloaded").on_click(move |_, window, cx| dialogs::keep::open(it.clone(), window, cx)));
        }
        let it = item.clone();
        group = group.child(icon("more", IconName::Ellipsis, "More").dropdown_menu(move |m, window, cx| item_menu(m, &it, window, cx)));
        row = row.child(group);
        if matches!(kind.as_str(), "movie" | "show" | "episode" | "season" | "album" | "track" | "artist") && !foreign {
            row = row.child(self.stars(item, tint, cx));
        }
        row.into_any_element()
    }

    /// Your rating, 1–5 stars (Plex stores 0–10). Clicking the current value clears it.
    fn stars(&self, item: &Metadata, tint: Hsla, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let stars = (item.user_rating.unwrap_or(0.) / 2.).round() as u8;
        let shown = self.hover_star.unwrap_or(stars);
        h_flex()
            .id("stars")
            .ml_1()
            .on_hover(cx.listener(|this, on: &bool, _, cx| {
                if !*on {
                    this.hover_star = None;
                    cx.notify();
                }
            }))
            .children((1..=5u8).map(|n| {
                let rk = item.rating_key().to_string();
                let on = n <= shown;
                div()
                    .id(("star", n as usize))
                    .size_6()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .text_color(if on { tint } else { theme.muted_foreground.opacity(0.6) })
                    .child(Icon::new(if on { IconName::StarFill } else { IconName::Star }).size_4())
                    .on_hover(cx.listener(move |this, hover: &bool, _, cx| {
                        if *hover {
                            this.hover_star = Some(n);
                            cx.notify();
                        }
                    }))
                    .on_click(move |_, _, cx| Self::rate(rk.clone(), if n == stars { None } else { Some(f64::from(n) * 2.) }, cx))
            }))
            .into_any_element()
    }

    fn facts(&self, item: &Metadata, cx: &mut Context<Self>) -> Option<AnyElement> {
        let theme = cx.theme().clone();
        let tags = tag_facts(item);
        let plain = plain_facts(item, store(cx).read(cx).owned());
        if tags.is_empty() && plain.is_empty() {
            return None;
        }
        let section = item.library_section_id.filter(|_| self.server.is_none());
        let row = |label: &str, value: AnyElement| {
            h_flex().gap_5().items_start().child(div().w(px(92.)).flex_none().pt_px().text_xs().font_medium().text_color(theme.muted_foreground).child(label.to_string())).child(div().flex_1().min_w_0().child(value))
        };
        Some(
            v_flex()
                .w(px(360.))
                .flex_none()
                .p_4()
                .gap_2()
                .rounded(px(12.))
                .border_1()
                .border_color(theme.border)
                .bg(theme.colors.group_box.opacity(0.6))
                .text_sm()
                .children(tags.into_iter().map(|(label, list)| {
                    let shown = list.len().min(8);
                    let links = h_flex().flex_wrap().children(list.into_iter().take(8).enumerate().map(|(i, t)| {
                        let name = t.tag.clone().unwrap_or_default();
                        let route = section.zip(t.filter.clone()).map(|(s, f)| Route::browse(format!("/library/sections/{s}/all?{f}"), name.clone()));
                        // A row, so the comma stays inline after its name (a plain div stacks its children).
                        h_flex()
                            .child(
                                div()
                                    .id(SharedString::from(format!("{label}-{i}")))
                                    .child(name.clone())
                                    .when_some(route, |d, r| d.cursor_pointer().hover(|s| s.underline()).on_click(move |_, _, cx| nav::push(r.clone(), cx))),
                            )
                            .when(i + 1 < shown, |d| d.child(div().pr_1().child(",")))
                    }));
                    row(label, links.into_any_element())
                }))
                .children(plain.into_iter().map(|(label, value)| row(label, div().child(value).into_any_element())))
                .into_any_element(),
        )
    }

    fn summary(&self, item: &Metadata, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let spoiler = store(cx).read(cx).spoilers() && item.kind() == "episode" && !item.is_watched();
        let summary = item.summary.clone().unwrap_or_default();
        let long = summary.chars().count() > 420;
        let expanded = self.expanded;
        let mut col = v_flex().flex_1().min_w_0().max_w(px(680.));
        if let Some(t) = item.tagline.clone().filter(|t| !t.is_empty()) {
            col = col.child(div().mb_3().text_lg().italic().text_color(theme.muted_foreground).child(t));
        }
        if !summary.is_empty() {
            col = col.child(
                div()
                    .id("summary")
                    .text_size(px(15.))
                    .line_height(rems(1.625))
                    .when(long && !expanded, |d| d.line_clamp(5).cursor_pointer())
                    .when(spoiler, |d| d.text_color(theme.foreground.opacity(0.08)).hover(|s| s.text_color(theme.foreground)))
                    .child(summary)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.expanded = !this.expanded;
                        cx.notify();
                    })),
            );
        }
        if matches!(item.kind(), "movie" | "episode" | "clip" | "track") && !item.media.is_empty() {
            let me = cx.entity().downgrade();
            col = col.children(streams::pickers(item, self.media_index, move |i, cx| {
                me.update(cx, |p, cx| {
                    p.media_index = i;
                    cx.notify();
                })
                .ok();
            }, cx));
        }
        col.into_any_element()
    }

    fn collection_controls(&self, item: &Metadata, cx: &mut Context<Self>) -> AnyElement {
        let mode = item.collection_mode.unwrap_or(-1).to_string();
        let order = collection_sort(item).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0).to_string();
        let (r1, r2, it) = (item.rating_key().to_string(), item.rating_key().to_string(), item.clone());
        let modes = [("-1", "Library default"), ("0", "Hide collection"), ("1", "Hide items"), ("2", "Show both")];
        let sorts = [("0", "Release date"), ("1", "Title"), ("2", "Custom")];
        let opts = |l: &[(&str, &str)]| l.iter().map(|(v, t)| (v.to_string(), t.to_string())).collect::<Vec<_>>();
        h_flex()
            .mt_8()
            .flex_wrap()
            .items_end()
            .gap_4()
            .child(div().w(px(192.)).child(form::field(
                "Show in library",
                form::select("collection-mode", opts(&modes), &mode, false, move |v, _, cx| Self::set_collection(r1.clone(), v.parse().ok(), None, cx)),
                cx,
            )))
            .child(div().w(px(192.)).child(form::field(
                "Sort",
                form::select("collection-sort", opts(&sorts), &order, false, move |v, _, cx| Self::set_collection(r2.clone(), None, v.parse().ok(), cx)),
                cx,
            )))
            .child(div().flex_1())
            .child(Button::new("delete-collection").danger().icon(IconName::Trash).label("Delete collection").on_click(move |_, window, cx| Self::delete_collection(&it, window, cx)))
            .into_any_element()
    }

    fn cast(&self, item: &Metadata, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let tint = store(cx).read(cx).tint();
        let section = item.library_section_id.filter(|_| self.server.is_none());
        let title = if matches!(item.kind(), "album" | "artist") { "Featuring" } else { "Cast" };
        v_flex()
            .child(widgets::section_head(title, None))
            .child(
                h_flex()
                    .id("cast")
                    .mx(-GUTTER)
                    .px(GUTTER)
                    .pb_1()
                    .gap_4()
                    .items_start()
                    .overflow_x_scroll()
                    .children(item.roles.iter().take(40).enumerate().map(|(i, role)| {
                        let name = role.tag.clone().unwrap_or_default();
                        let route = section.zip(role.filter.clone()).map(|(s, f)| Route::browse(format!("/library/sections/{s}/all?{f}"), name.clone()));
                        let group: SharedString = format!("role-{i}").into();
                        let photo = art_of(role.thumb.as_deref(), 80., 80., item.server_id());
                        v_flex()
                            .id(("role", i))
                            .group(group.clone())
                            .w(px(96.))
                            .flex_none()
                            .items_center()
                            .text_center()
                            .child(
                                div()
                                    .mb_2p5()
                                    .size(px(76.))
                                    .rounded_full()
                                    .overflow_hidden()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(theme.muted)
                                    .border_1()
                                    .border_color(theme.border)
                                    .text_sm()
                                    .font_semibold()
                                    .text_color(theme.muted_foreground)
                                    .when(route.is_some(), |d| d.group_hover(group.clone(), |s| s.border_2().border_color(tint.opacity(0.6))))
                                    .map(|d| match photo {
                                        Some(img) => d.child(img.size_full().object_fit(ObjectFit::Cover)),
                                        None => d.child(format::initials(&name)),
                                    }),
                            )
                            .child(div().w_full().truncate().text_xs().font_medium().child(name.clone()))
                            .when_some(role.role.clone().filter(|r| !r.is_empty()), |d, r| d.child(div().w_full().truncate().text_xs().text_color(theme.muted_foreground).child(r)))
                            .when_some(route, |d, r| d.cursor_pointer().on_click(move |_, _, cx| nav::push(r.clone(), cx)))
                    })),
            )
            .into_any_element()
    }
}

impl Render for Item {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let item = match (&self.data, &self.stale) {
            (Load::Ready(Some(m)), _) => m.clone(),
            (Load::Ready(None), _) => {
                return widgets::page("item").child(widgets::empty("Not found", Some("This item no longer exists on the server.".into()), cx)).into_any_element();
            }
            (_, Some(m)) => m.clone(),
            (Load::Failed(e), None) => {
                return widgets::page("item")
                    .child(widgets::failed(e.clone(), cx).child(Button::new("retry").outline().small().mt_3().label("Try again").on_click(cx.listener(|this, _, _, cx| this.load(cx)))))
                    .into_any_element();
            }
            (Load::Loading, None) => return widgets::page("item").child(widgets::loading()).into_any_element(),
        };
        let kind = item.kind().to_string();
        let owned = store(cx).read(cx).owned();
        let bg = theme.background;
        let backdrop = div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .h(px(520.))
            .overflow_hidden()
            .opacity(0.5)
            .children(art_of(item.backdrop(), 1920., 1080., item.server_id()).map(|i| i.size_full().object_fit(ObjectFit::Cover)))
            // background/10 → background/60 → background downwards, in two halves (GPUI gradients
            // have two stops; angle 0 runs from the bottom up).
            .child(div().absolute().top_0().left_0().right_0().h(px(260.)).bg(linear_gradient(0., linear_color_stop(bg.opacity(0.6), 0.), linear_color_stop(bg.opacity(0.1), 1.))))
            .child(div().absolute().bottom_0().left_0().right_0().h(px(260.)).bg(linear_gradient(0., linear_color_stop(bg, 0.), linear_color_stop(bg.opacity(0.6), 1.))));
        let header = self.header(&item, cx);
        let summary = self.summary(&item, cx);
        let facts = self.facts(&item, cx);
        let mut body = v_flex()
            .relative()
            .px(GUTTER)
            .pt(px(128.))
            .pb(px(64.))
            .child(header)
            .when_some(match &self.data {
                Load::Failed(e) => Some(e.clone()),
                _ => None,
            }, |d, e| d.child(div().mt_4().text_sm().text_color(theme.danger).child(format!("Could not refresh: {e}"))))
            .child(h_flex().mt_10().gap_10().items_start().child(summary).children(facts));
        if kind == "collection" && owned && self.server.is_none() {
            body = body.child(self.collection_controls(&item, cx));
        }
        if let Some(children) = &self.children {
            body = body.child(div().mt_10().h_px().bg(theme.border)).child(children.clone());
        }
        if !item.roles.is_empty() {
            body = body.child(self.cast(&item, cx));
        }
        if !item.extras().is_empty() && self.server.is_none() {
            body = body.child(CardRow::new("extras", "Extras", item.extras().to_vec()).shape(Shape::Landscape));
        }
        for (i, hub) in item.related_hubs().iter().filter(|h| !h.metadata.is_empty()).enumerate() {
            body = body.child(HubRow::new(ElementId::Name(format!("related:{}:{i}", hub.hub_identifier.as_deref().unwrap_or("")).into()), hub.clone()));
        }
        for (i, hub) in extra_hubs(&item, &self.extra).into_iter().enumerate() {
            body = body.child(HubRow::new(ElementId::Name(format!("extra:{}:{i}", hub.hub_identifier.as_deref().unwrap_or("")).into()), hub));
        }
        div().id("item").size_full().overflow_y_scroll().child(div().relative().child(backdrop).child(body)).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    fn meta(json: &str) -> Metadata {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn crumbs_and_meta_lines() {
        let ep = meta(r#"{"type":"episode","index":2,"parentIndex":1,"grandparentRatingKey":"500","grandparentTitle":"Static","parentRatingKey":"601","parentTitle":"Season 1","originallyAvailableAt":"2024-03-04","duration":2880000}"#);
        assert_eq!(crumbs(&ep), vec![("Static".to_string(), "500".to_string()), ("Season 1".to_string(), "601".to_string())]);
        assert_eq!(meta_line(&ep), vec!["S01E02", "Mar 4, 2024", "48m"]);
        let show = meta(r#"{"type":"show","year":2019,"childCount":1,"leafCount":6,"duration":2700000}"#);
        assert_eq!(meta_line(&show), vec!["2019", "1 season", "6 episodes"]);
    }

    #[test]
    fn ratings_use_rotten_tomatoes_percentages() {
        let m = meta(r#"{"rating":7.64,"ratingImage":"rottentomatoes://image.rating.ripe","audienceRating":8.0}"#);
        assert_eq!(ratings(&m), vec!["76% critics", "★ 8.0"]);
    }

    #[test]
    fn facts_and_extra_hubs() {
        let m = meta(r#"{"studio":"Mock","Media":[{"container":"mkv","width":1920,"height":1080,"Part":[{"file":"/m/a.mkv","size":2048}]}],"Related":{"Hub":[{"hubIdentifier":"a","title":"More"}]}}"#);
        let facts = plain_facts(&m, false);
        assert_eq!(facts[0], ("Studio", "Mock".to_string()));
        assert_eq!(facts[1], ("File", "MKV · 1920×1080 · 2.0 KB".to_string()));
        assert!(!facts.iter().any(|(k, _)| *k == "Path"));
        assert!(plain_facts(&m, true).iter().any(|(k, _)| *k == "Path"));
        let hubs: Vec<Hub> = serde_json::from_str(r#"[{"hubIdentifier":"a","title":"x","Metadata":[{"ratingKey":"1"}]},{"hubIdentifier":"b","title":"MORE","Metadata":[{"ratingKey":"1"}]},{"hubIdentifier":"c","title":"Other","Metadata":[{"ratingKey":"1"}]},{"hubIdentifier":"d","title":"Empty"}]"#).unwrap();
        let left: Vec<_> = extra_hubs(&m, &hubs).into_iter().filter_map(|h| h.hub_identifier).collect();
        assert_eq!(left, vec!["c"]);
    }

    #[test]
    fn download_labels() {
        assert_eq!(download_label(None), "Download");
        assert_eq!(collection_sort(&meta(r#"{"collectionSort":2}"#)).as_deref(), Some("2"));
        assert_eq!(collection_sort(&meta(r#"{"collectionSort":"1"}"#)).as_deref(), Some("1"));
    }
}
