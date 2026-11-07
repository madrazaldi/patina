//! Downloads: items kept for offline playback, grouped by show / artist, and
//! the smart-download policies. Works without a server session (the offline
//! screen shows it when the server is down), so it only uses download state.

use crate::prelude::*;
use std::cmp::Ordering;
use std::collections::HashMap;

use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::progress::Progress;
use patina_core::downloads::{self, Download, State};
use patina_core::ops;
use patina_core::smart::{self, Overview, Patch, PolicyView};
use patina_plex::Metadata;

use crate::art::{ArtKey, art};
use crate::core::{bg, blocking, core, message};
use crate::format;
use crate::nav::{self, Route};
use crate::store::{self, store};
use crate::widgets;

const GIB: f64 = 1024. * 1024. * 1024.;

/// Downloads of one show, artist, or of all movies (or other items) of a server.
#[derive(Debug)]
struct Group {
    key: String,
    title: String,
    kind: String,
    items: Vec<Download>,
}

fn group_of(d: &Download) -> (String, String) {
    let k = d.item.kind();
    let scope = format!("{}\u{1}{}\u{1}{k}", d.account, d.server_id);
    match k {
        "episode" | "track" => {
            let t = d.item.grandparent_title.clone().unwrap_or_else(|| "Unknown".into());
            (format!("{scope}\u{1}{t}"), t)
        }
        "movie" => (scope, "Movies".into()),
        _ => (scope, "Other".into()),
    }
}

#[derive(Debug, PartialEq, PartialOrd)]
enum Part {
    Num(i64),
    Text(String),
}

/// Episodes by season/episode, tracks by album/disc/track, the rest by title.
fn sort_key(m: &Metadata) -> Vec<Part> {
    match m.kind() {
        "episode" => vec![Part::Num(m.parent_index.unwrap_or(0)), Part::Num(m.index.unwrap_or(0))],
        "track" => vec![Part::Text(m.parent_title.clone().unwrap_or_default().to_lowercase()), Part::Num(m.parent_index.unwrap_or(0)), Part::Num(m.index.unwrap_or(0))],
        _ => vec![Part::Text(m.title_sort.clone().unwrap_or_else(|| m.title().to_string()).to_lowercase())],
    }
}

/// Movies first, then shows and artists by name, anything else last.
fn rank(g: &Group) -> u8 {
    match g.kind.as_str() {
        "movie" => 0,
        "episode" | "track" => 1,
        _ => 2,
    }
}

fn groups(list: &[Download]) -> Vec<Group> {
    let mut out: Vec<Group> = Vec::new();
    for d in list {
        let (key, title) = group_of(d);
        match out.iter_mut().find(|g| g.key == key) {
            Some(g) => g.items.push(d.clone()),
            None => out.push(Group { key, title, kind: d.item.kind().to_string(), items: vec![d.clone()] }),
        }
    }
    for g in &mut out {
        g.items.sort_by(|a, b| sort_key(&a.item).partial_cmp(&sort_key(&b.item)).unwrap_or(Ordering::Equal));
    }
    out.sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase())));
    out
}

/// Inside a group the show / artist name is already in the heading.
fn row_title(m: &Metadata) -> String {
    match m.kind() {
        "episode" => format!("{} · {}", m.episode_code(), m.title()),
        "track" => m.title().to_string(),
        _ => m.full_title(),
    }
}

/// "Album · 1.2 GB of 2.9 GB · Server"
fn row_sub(d: &Download) -> String {
    let size = if d.state == State::Downloading {
        format!("{} of {}", format::bytes(d.received), format::bytes(d.size))
    } else if d.size > 0 {
        format::bytes(d.size)
    } else {
        String::new()
    };
    let album = if d.item.kind() == "track" { d.item.parent_title.clone().unwrap_or_default() } else { String::new() };
    [album, size, d.server_name.clone()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ")
}

fn group_sub(g: &Group) -> String {
    let size: i64 = g.items.iter().map(|d| d.size).sum();
    let what = match g.kind.as_str() {
        "episode" => "episode",
        "track" => "track",
        "movie" => "movie",
        _ => "item",
    };
    format!("{} · {}", format::plural(g.items.len() as i64, what), format::bytes(size))
}

fn pct(d: &Download) -> i64 {
    if d.size > 0 { (d.received * 100 / d.size).min(100) } else { 0 }
}

/// A storage budget typed in GB: none when empty or not positive.
fn parse_budget(text: &str) -> Option<u64> {
    let gb: f64 = text.trim().parse().ok()?;
    (gb > 0.).then(|| (gb * GIB).round() as u64)
}

fn budget_text(budget: Option<u64>) -> String {
    budget.map(|b| format!("{}", (b as f64 / GIB * 10.).round() / 10.)).unwrap_or_default()
}

pub struct Downloads {
    offline: bool,
    smart: Option<Overview>,
    /// The downloads event the smart overview was loaded for: it reloads on each one, as policy
    /// and budget changes come with them.
    seen: u64,
    busy: bool,
    keep: HashMap<String, Entity<InputState>>,
    budget: Option<Entity<InputState>>,
    /// Put the loaded values into the inputs on the next render.
    sync_inputs: bool,
    _subs: Vec<Subscription>,
}

impl Downloads {
    pub fn new(offline: bool, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subs = vec![cx.observe(&store(cx), |this: &mut Self, store, cx| {
            if store.read(cx).downloads_tick != this.seen {
                this.load_smart(cx);
            }
            cx.notify();
        })];
        let mut this = Self { offline, smart: None, seen: 0, busy: false, keep: HashMap::new(), budget: None, sync_inputs: false, _subs: subs };
        this.load_smart(cx);
        this
    }

    fn load_smart(&mut self, cx: &mut Context<Self>) {
        self.seen = store(cx).read(cx).downloads_tick;
        let app = core(cx);
        cx.spawn(async move |this, cx| {
            let overview = blocking(move || smart::overview(&app)).await;
            this.update(cx, |d, cx| {
                d.smart = Some(overview);
                d.sync_inputs = true;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn patch(id: String, patch: Patch, cx: &mut App) {
        let app = core(cx);
        store::act(cx, async move { smart::update_policy(&app, &id, patch) }, None, false);
    }

    fn run_now(&mut self, cx: &mut Context<Self>) {
        self.busy = true;
        cx.notify();
        let app = core(cx);
        cx.spawn(async move |this, cx| {
            let result = bg(async move { smart::reconcile(&app).await }).await;
            this.update(cx, |d, cx| {
                d.busy = false;
                match result {
                    Ok(r) => {
                        let mut bits = Vec::new();
                        if r.added > 0 {
                            bits.push(format!("{} queued", format::plural(r.added as i64, "item")));
                        }
                        if r.removed > 0 {
                            bits.push(format!("{} removed", format::plural(r.removed as i64, "download")));
                        }
                        if r.over_budget > 0 {
                            bits.push(format!("{} left out (storage budget)", r.over_budget));
                        }
                        bits.extend(r.errors);
                        let text = if bits.is_empty() { "Downloads already match the policies".to_string() } else { bits.join(" · ") };
                        store::toast(cx, text);
                    }
                    Err(e) => store::error(cx, message(&e)),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn drop_policy(p: &PolicyView, window: &mut Window, cx: &mut App) {
        let text = if p.downloads > 0 {
            format!("The {} it fetched stay on this computer until you delete them.", format::plural(p.downloads as i64, "download"))
        } else {
            "Nothing it fetched is on this computer.".to_string()
        };
        let id = p.policy.id.clone();
        widgets::confirm(format!("Stop keeping “{}” downloaded?", p.policy.title), Some(text.into()), "Stop keeping", false, window, cx, move |_, cx| {
            let (app, id) = (core(cx), id.clone());
            store::act(cx, async move { smart::remove_policy(&app, &id, false) }, None, false);
        });
    }

    fn remove(d: &Download, window: &mut Window, cx: &mut App) {
        let done = d.state == State::Done;
        let name = d.item.full_title();
        let (title, text, action) = if done {
            ("Delete download?", format!("{name} will be removed from this computer."), "Delete")
        } else {
            ("Remove from downloads?", format!("{name} will stop downloading."), "Remove")
        };
        let id = d.id.clone();
        widgets::confirm(title, Some(text.into()), action, true, window, cx, move |_, cx| {
            let (app, id) = (core(cx), id.clone());
            store::act(
                cx,
                async move {
                    downloads::remove(&app, &id);
                    Ok(())
                },
                None,
                false,
            );
        });
    }

    fn play(ids: Vec<String>, cx: &mut App) {
        let app = core(cx);
        store::act(cx, async move { ops::playback::play_downloads(&app, &ids) }, None, false);
    }

    fn open_folder(cx: &mut App) {
        let dir = core(cx).settings().player.downloads_dir();
        match std::fs::create_dir_all(&dir) {
            Ok(()) => cx.open_with_system(&dir),
            Err(e) => store::error(cx, format!("Could not open {}: {e}", dir.display())),
        }
    }

    /// The keep-count input of a policy, created on first use.
    fn keep_input(&mut self, p: &PolicyView, window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
        if let Some(input) = self.keep.get(&p.policy.id) {
            return input.clone();
        }
        let input = cx.new(|cx| InputState::new(window, cx).default_value(p.policy.keep.to_string()));
        let id = p.policy.id.clone();
        self._subs.push(cx.subscribe_in(&input, window, move |this, state, e: &InputEvent, window, cx| {
            if !matches!(e, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                return;
            }
            let Some(keep) = this.smart.as_ref().and_then(|o| o.policies.iter().find(|p| p.policy.id == id)).map(|p| p.policy.keep) else { return };
            match state.read(cx).value().trim().parse::<f64>().map(|n| n.floor() as u32) {
                Ok(n) if (1..=50).contains(&n) => {
                    if n != keep {
                        Self::patch(id.clone(), Patch { keep: Some(n), ..Default::default() }, cx);
                    }
                }
                _ => state.update(cx, |s, cx| s.set_value(keep.to_string(), window, cx)),
            }
        }));
        self.keep.insert(p.policy.id.clone(), input.clone());
        input
    }

    fn budget_input(&mut self, budget: Option<u64>, window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
        if let Some(input) = &self.budget {
            return input.clone();
        }
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("No limit").default_value(budget_text(budget)));
        self._subs.push(cx.subscribe_in(&input, window, |this, state, e: &InputEvent, _, cx| {
            if !matches!(e, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                return;
            }
            let value = parse_budget(&state.read(cx).value());
            if this.smart.as_ref().is_some_and(|o| o.budget == value) {
                return;
            }
            let app = core(cx);
            store::act(cx, async move { smart::set_budget(&app, value) }, None, false);
        }));
        self.budget = Some(input.clone());
        input
    }

    fn smart_card(&mut self, overview: &Overview, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        if self.sync_inputs {
            self.sync_inputs = false;
            for p in &overview.policies {
                if let Some(input) = self.keep.get(&p.policy.id) {
                    let (input, keep) = (input.clone(), p.policy.keep.to_string());
                    if !input.read(cx).focus_handle(cx).is_focused(window) && input.read(cx).value() != keep {
                        input.update(cx, |s, cx| s.set_value(keep, window, cx));
                    }
                }
            }
            if let Some(input) = self.budget.clone() {
                let text = budget_text(overview.budget);
                if !input.read(cx).focus_handle(cx).is_focused(window) && parse_budget(&input.read(cx).value()) != overview.budget {
                    input.update(cx, |s, cx| s.set_value(text, window, cx));
                }
            }
        }

        let active = overview.policies.iter().any(|p| p.active);
        let header = h_flex()
            .px_6()
            .pb_6()
            .border_b_1()
            .border_color(theme.border)
            .items_start()
            .justify_between()
            .gap_4()
            .child(
                v_flex()
                    .gap_1p5()
                    .child(div().text_base().font_semibold().child("Smart downloads"))
                    .child(div().text_sm().text_color(muted).child("Keep the next unwatched episodes downloaded")),
            )
            .when(active, |d| d.child(Button::new("smart-run").outline().small().label("Run now").disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.run_now(cx)))));

        let mut rows = v_flex();
        for (i, p) in overview.policies.iter().enumerate() {
            let keep = self.keep_input(p, window, cx);
            let pol = &p.policy;
            let dim = !pol.enabled || !p.active;
            let mut desc = format!("{} · {}", pol.target.label(), pol.server_name);
            if !p.active {
                desc.push_str(" (another account or server)");
            }
            desc.push_str(&format!(" · {}", format::plural(p.downloads as i64, "download")));
            if p.bytes > 0 {
                desc.push_str(&format!(", {}", format::bytes(p.bytes)));
            }
            if !pol.enabled {
                desc.push_str(" · paused");
            }
            let (id1, id2) = (pol.id.clone(), pol.id.clone());
            let (enabled, delete_watched) = (pol.enabled, pol.delete_watched);
            let view = p.clone();
            rows = rows.child(
                h_flex()
                    .px_4()
                    .py_3p5()
                    .gap_6()
                    .justify_between()
                    .when(i > 0, |d| d.border_t_1().border_color(theme.border))
                    .child(
                        v_flex()
                            .min_w_0()
                            .gap_1()
                            .child(div().text_sm().font_medium().when(dim, |d| d.text_color(muted)).child(pol.title.clone()))
                            .child(div().text_size(px(13.)).text_color(muted).child(desc)),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .items_center()
                            .gap_4()
                            .child(h_flex().gap_2().text_xs().text_color(muted).child("Keep").child(div().w_16().child(Input::new(&keep).small())))
                            .child(
                                Checkbox::new(("remove-watched", i))
                                    .label("Remove watched")
                                    .tooltip("Delete downloads once they are watched")
                                    .checked(delete_watched)
                                    .on_click(move |on, _, cx| Self::patch(id1.clone(), Patch { delete_watched: Some(*on), ..Default::default() }, cx)),
                            )
                            .child(
                                h_flex()
                                    .gap_0p5()
                                    .child(
                                        widgets::icon_button(("smart-pause", i), if enabled { IconName::Pause } else { IconName::Play }, if enabled { "Pause" } else { "Resume" })
                                            .on_click(move |_, _, cx| Self::patch(id2.clone(), Patch { enabled: Some(!enabled), ..Default::default() }, cx)),
                                    )
                                    .child(widgets::icon_button(("smart-drop", i), IconName::Trash, "Stop keeping downloaded").on_click(move |_, window, cx| Self::drop_policy(&view, window, cx))),
                            ),
                    ),
            );
        }
        let budget = self.budget_input(overview.budget, window, cx);
        let first = overview.policies.is_empty();
        rows = rows.child(
            h_flex()
                .px_4()
                .py_3p5()
                .gap_6()
                .justify_between()
                .when(!first, |d| d.border_t_1().border_color(theme.border))
                .child(
                    v_flex().min_w_0().gap_1().child(div().text_sm().font_medium().child("Storage budget")).child(div().text_size(px(13.)).text_color(muted).child(format!(
                        "{} used by downloads{}. Smart downloads stop adding when the budget is full.",
                        format::bytes(overview.used as i64),
                        overview.budget.map(|b| format!(" of {}", format::bytes(b as i64))).unwrap_or_default()
                    ))),
                )
                .child(h_flex().flex_none().gap_2().text_sm().text_color(muted).child(div().w_24().child(Input::new(&budget).small())).child("GB")),
        );
        if first {
            rows = rows.child(
                div()
                    .px_4()
                    .py_3p5()
                    .border_t_1()
                    .border_color(theme.border)
                    .text_size(px(13.))
                    .text_color(muted)
                    .child("Open a show, season or playlist and choose “Keep next unwatched downloaded”."),
            );
        }
        v_flex()
            .flex_none()
            .mb_2()
            .pt_6()
            .rounded(px(12.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.colors.group_box)
            .overflow_hidden()
            .child(header)
            .child(rows)
            .into_any_element()
    }

    fn row(&self, d: &Download, first: bool, linkable: bool, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let tint = store(cx).read(cx).tint();
        let muted = theme.muted_foreground;
        let m = &d.item;
        let track = m.kind() == "track";
        let dim = matches!(d.state, State::Queued | State::Failed);
        let rk = m.rating_key().to_string();
        let title = div().truncate().child(row_title(m));
        let title = if linkable && !rk.is_empty() {
            title
                .id(SharedString::from(format!("dl-title-{}", d.id)))
                .cursor_pointer()
                .hover(|s| s.underline())
                .on_click(move |_, _, cx| nav::push(Route::Item { rating_key: rk.clone(), server: None }, cx))
                .into_any_element()
        } else {
            title.into_any_element()
        };
        let p = pct(d);
        let id = d.id.clone();
        let mut actions = h_flex().flex_none().items_center().gap_1();
        match d.state {
            State::Queued | State::Downloading => {
                let id = id.clone();
                actions = actions.child(widgets::icon_button(SharedString::from(format!("dl-pause-{id}")), IconName::Pause, "Pause download").on_click(move |_, _, cx| {
                    let (app, id) = (core(cx), id.clone());
                    store::act(
                        cx,
                        async move {
                            downloads::pause(&app, &id);
                            Ok(())
                        },
                        None,
                        false,
                    );
                }));
            }
            State::Paused => {
                let id = id.clone();
                actions = actions.child(div().px_2().py_0p5().rounded(px(6.)).bg(theme.secondary).text_xs().font_medium().child("Paused")).child(
                    widgets::icon_button(SharedString::from(format!("dl-resume-{id}")), IconName::Play, "Resume download").on_click(move |_, _, cx| retry(id.clone(), cx)),
                );
            }
            _ => {}
        }
        match d.state {
            State::Downloading => actions = actions.child(div().min_w_12().pr_1().text_right().text_xs().text_color(muted).child(format!("{p}%"))),
            State::Queued => actions = actions.child(div().min_w_12().pr_1().text_right().text_xs().text_color(muted).child("Waiting")),
            State::Failed => {
                let id = id.clone();
                actions = actions.child(Button::new(SharedString::from(format!("dl-retry-{id}"))).ghost().small().icon(IconName::RotateCw).label("Retry").on_click(move |_, _, cx| retry(id.clone(), cx)));
            }
            State::Done => {
                let id = id.clone();
                actions = actions.child(widgets::icon_button(SharedString::from(format!("dl-play-{id}")), IconName::Play, "Play").on_click(move |_, _, cx| Self::play(vec![id.clone()], cx)));
            }
            State::Paused => {}
        }
        let dl = d.clone();
        actions = actions.child(
            widgets::icon_button(SharedString::from(format!("dl-remove-{id}")), IconName::Trash, if d.state == State::Done { "Delete download" } else { "Remove" })
                .on_click(move |_, window, cx| Self::remove(&dl, window, cx)),
        );

        h_flex()
            .px_4()
            .py_3()
            .gap_4()
            .items_center()
            .when(!first, |r| r.border_t_1().border_color(theme.border))
            .child(
                div()
                    .h_12()
                    .w(px(if track { 48. } else { 32. }))
                    .flex_none()
                    .rounded(px(4.))
                    .overflow_hidden()
                    .bg(theme.muted)
                    .when(d.poster.is_some(), |m| m.child(art(ArtKey::download(d.id.clone())).size_full().object_fit(ObjectFit::Cover))),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(div().text_sm().font_medium().max_w_full().when(dim, |t| t.text_color(muted)).child(title))
                    .child(match d.state {
                        State::Failed => div().text_xs().truncate().text_color(theme.danger.opacity(0.8)).child(d.error.clone().unwrap_or_else(|| "Download failed".into())),
                        _ => div().text_xs().truncate().text_color(muted).child(row_sub(d)),
                    })
                    .when(d.state == State::Downloading, |c| c.child(div().mt_1p5().max_w(px(360.)).child(Progress::new(SharedString::from(format!("dl-progress-{}", d.id))).value(p as f32).color(tint)))),
            )
            .child(actions)
            .into_any_element()
    }
}

fn retry(id: String, cx: &mut App) {
    let app = core(cx);
    store::act(
        cx,
        async move {
            downloads::retry(&app, &id);
            Ok(())
        },
        None,
        false,
    );
}

impl Render for Downloads {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let s = store(cx).read(cx);
        let list = s.downloads.clone();
        let server_id = s.session.as_ref().map(|x| x.server_id.clone());
        let on_disk: i64 = list.iter().map(|d| if d.state == State::Done { d.size } else { d.received }).sum();
        let active = list.iter().filter(|d| matches!(d.state, State::Queued | State::Downloading)).count();
        let sub = if list.is_empty() {
            "Saved on this computer for offline playback.".to_string()
        } else {
            let mut s = format!("{} · {} on disk", format::plural(list.len() as i64, "item"), format::bytes(on_disk));
            if active > 0 {
                s.push_str(&format!(" · {active} in progress"));
            }
            s
        };
        let mut page = widgets::page(if self.offline { "offline-downloads" } else { "downloads" }).child(widgets::page_head(
            "Downloads",
            Some(sub.into()),
            Some(Button::new("open-folder").outline().icon(IconName::FolderOpen).label("Open folder").on_click(|_, _, cx| Self::open_folder(cx)).into_any_element()),
            cx,
        ));
        if let Some(overview) = self.smart.clone() {
            page = page.child(self.smart_card(&overview, window, cx));
        }
        if list.is_empty() {
            return page
                .child(div().mt_8().child(widgets::empty_box(
                    IconName::Download,
                    Some("Nothing downloaded".into()),
                    Some("Choose “Download” from any movie, episode, season or album menu to keep a copy on this computer and play it without a connection.".into()),
                    cx,
                )))
                .into_any_element();
        }
        for g in groups(&list) {
            let done: Vec<String> = g.items.iter().filter(|d| d.state == State::Done).map(|d| d.id.clone()).collect();
            let playable = matches!(g.kind.as_str(), "episode" | "track") && !done.is_empty();
            page = page.child(
                h_flex()
                    .mt(px(36.))
                    .mb_3()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(div().text_base().font_semibold().child(g.title.clone()))
                    .child(
                        h_flex()
                            .items_center()
                            .gap_3()
                            .child(div().text_xs().text_color(theme.muted_foreground).child(group_sub(&g)))
                            .when(playable, |d| {
                                d.child(Button::new(SharedString::from(format!("play-all-{}", g.key))).outline().small().label("Play all").on_click(move |_, _, cx| Self::play(done.clone(), cx)))
                            }),
                    ),
            );
            let mut card = v_flex().flex_none().rounded(px(12.)).border_1().border_color(theme.border).bg(theme.colors.group_box).overflow_hidden();
            for (i, d) in g.items.iter().enumerate() {
                let linkable = !self.offline && server_id.as_deref() == Some(d.server_id.as_str());
                card = card.child(self.row(d, i == 0, linkable, cx));
            }
            page = page.child(card);
        }
        page.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    fn dl(id: &str, kind: &str, title: &str, show: Option<&str>, season: i64, index: i64) -> Download {
        let item = Metadata {
            rating_key: Some(id.into()),
            kind: Some(kind.into()),
            title: Some(title.into()),
            grandparent_title: show.map(str::to_string),
            parent_index: Some(season),
            index: Some(index),
            ..Default::default()
        };
        Download {
            id: id.into(),
            server_id: "s".into(),
            server_name: "Server".into(),
            item,
            file: None,
            poster: None,
            size: 100,
            received: 50,
            state: State::Done,
            error: None,
            added_at: 0,
            account: "a".into(),
            validator: None,
            source_key: None,
            policy: None,
        }
    }

    #[test]
    fn groups_sort_movies_first_then_shows() {
        let list = vec![
            dl("1", "episode", "B", Some("Zed"), 1, 2),
            dl("2", "movie", "Beta", None, 0, 0),
            dl("3", "episode", "A", Some("Zed"), 1, 1),
            dl("4", "movie", "alpha", None, 0, 0),
            dl("5", "episode", "C", Some("Abe"), 2, 1),
            dl("6", "photo", "P", None, 0, 0),
        ];
        let g = groups(&list);
        let titles: Vec<&str> = g.iter().map(|g| g.title.as_str()).collect();
        assert_eq!(titles, ["Movies", "Abe", "Zed", "Other"]);
        assert_eq!(g[0].items.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(), ["4", "2"]);
        assert_eq!(g[2].items.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(), ["3", "1"]);
        assert_eq!(group_sub(&g[2]), "2 episodes · 200 B");
    }

    #[test]
    fn row_text() {
        let mut d = dl("1", "track", "Song", Some("Band"), 1, 1);
        d.item.parent_title = Some("Album".into());
        d.state = State::Downloading;
        d.size = 2048;
        d.received = 1024;
        assert_eq!(row_sub(&d), "Album · 1.0 KB of 2.0 KB · Server");
        assert_eq!(pct(&d), 50);
        assert_eq!(row_title(&d.item), "Song");
    }

    #[test]
    fn budgets() {
        assert_eq!(parse_budget(""), None);
        assert_eq!(parse_budget("0"), None);
        assert_eq!(parse_budget("x"), None);
        assert_eq!(parse_budget("2"), Some(2 * 1024 * 1024 * 1024));
        assert_eq!(budget_text(Some(1610612736)), "1.5");
        assert_eq!(budget_text(None), "");
    }
}
