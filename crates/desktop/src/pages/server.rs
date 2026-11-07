//! Server tools for the owner: overview and streaming stats, libraries,
//! maintenance, server settings, and users & devices.

use crate::prelude::*;
use std::collections::HashMap;
use std::rc::Rc;

use chrono::{Local, TimeZone};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::progress::Progress;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tab::{Tab, TabBar};
use patina_core::ops::server::{self as ops, Bucket, Maintenance, Overview, Pref, PrefGroup, Stats, Usage, UsersDevices};
use patina_core::session;
use patina_plex::Directory;

use crate::art::art_of;
use crate::core::{bg, core, message};
use crate::dialogs::server_library;
use crate::format;
use crate::store::{self, store};
use crate::theme;
use crate::widgets;

// Shared pieces (also used by the library settings dialog) ------------------------

/// Something a page fetches: the last good data, the last error, and whether a fetch is running.
pub(crate) struct Res<T> {
    pub data: Option<T>,
    pub error: Option<String>,
    pub loading: bool,
}

impl<T> Default for Res<T> {
    fn default() -> Self {
        Self { data: None, error: None, loading: false }
    }
}

impl<T> Res<T> {
    pub fn set(&mut self, result: anyhow::Result<T>) {
        self.loading = false;
        match result {
            Ok(v) => {
                self.data = Some(v);
                self.error = None;
            }
            Err(e) => self.error = Some(message(&e)),
        }
    }
}

/// Saves a setting's new value.
pub(crate) type Save = Rc<dyn Fn(String, &mut App)>;

pub(crate) fn is_on(v: &str) -> bool {
    v == "true" || v == "1"
}

/// The text inputs of text settings, one per setting, kept across renders.
#[derive(Default)]
pub(crate) struct TextPrefs {
    inputs: HashMap<String, Entity<InputState>>,
    subs: Vec<Subscription>,
}

impl TextPrefs {
    /// The input for `pref`; Enter calls `save` with the typed value.
    pub fn input<V: 'static>(
        &mut self,
        pref: &Pref,
        window: &mut Window,
        cx: &mut Context<V>,
        save: impl Fn(&mut V, String, String, &mut Context<V>) + 'static,
    ) -> Entity<InputState> {
        if let Some(i) = self.inputs.get(&pref.id) {
            return i.clone();
        }
        let input = cx.new(|cx| InputState::new(window, cx).default_value(pref.value.clone()));
        let id = pref.id.clone();
        self.subs.push(cx.subscribe_in(&input, window, move |this, state, e: &InputEvent, _, cx| match e {
            InputEvent::Change => cx.notify(),
            InputEvent::PressEnter { .. } => save(this, id.clone(), state.read(cx).value().to_string(), cx),
            _ => {}
        }));
        self.inputs.insert(pref.id.clone(), input.clone());
        input
    }
}

/// One server or library setting: a switch, a menu of choices, or text with Save.
pub(crate) fn pref_row(pref: &Pref, show_id: bool, first: bool, input: Option<Entity<InputState>>, save: Save, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let id = SharedString::from(format!("pref-{}", pref.id));
    let control: AnyElement = if pref.kind == "bool" {
        let save = save.clone();
        Switch::new(id).checked(is_on(&pref.value)).on_click(move |on: &bool, _, cx| save(if *on { "true".into() } else { "false".into() }, cx)).into_any_element()
    } else if !pref.options.is_empty() {
        let current = pref.options.iter().find(|(v, _)| *v == pref.value).map_or(pref.value.clone(), |(_, l)| l.clone());
        let (options, value) = (pref.options.clone(), pref.value.clone());
        Button::new(id)
            .outline()
            .small()
            .w_full()
            .justify_between()
            .child(div().flex_1().min_w_0().truncate().text_left().child(current))
            .dropdown_caret(true)
            .dropdown_menu(move |mut menu, _, _| {
                for (v, l) in &options {
                    let (v, save) = (v.clone(), save.clone());
                    menu = menu.item(PopupMenuItem::new(l.clone()).checked(v == value).on_click(move |_, _, cx| save(v.clone(), cx)));
                }
                menu
            })
            .into_any_element()
    } else {
        match input {
            Some(input) => {
                let typed = input.read(cx).value().to_string();
                let dirty = typed != pref.value;
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(div().flex_1().min_w_0().child(Input::new(&input).small()))
                    .when(dirty, |d| d.child(Button::new(id).primary().small().label("Save").on_click(move |_, _, cx| save(typed.clone(), cx))))
                    .into_any_element()
            }
            None => div().into_any_element(),
        }
    };
    h_flex()
        .flex_none()
        .px_4()
        .py_3p5()
        .gap_6()
        .items_center()
        .when(!first, |d| d.border_t_1().border_color(theme.border))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(
                    h_flex()
                        .gap_2()
                        .items_baseline()
                        .text_sm()
                        .font_medium()
                        .child(pref.label.clone())
                        .when(show_id, |d| d.child(div().text_size(px(11.)).font_normal().text_color(muted).child(pref.id.clone()))),
                )
                .when_some(pref.summary.clone().filter(|s| !s.is_empty()), |d, s| d.child(div().mt_0p5().text_size(px(13.)).text_color(muted).child(s))),
        )
        .child(h_flex().w(px(260.)).flex_none().justify_end().child(control))
        .into_any_element()
}

/// A card with a bordered title row, holding divided rows.
fn titled_card(title: &str, cx: &App) -> Div {
    let theme = cx.theme();
    v_flex()
        .flex_none()
        .rounded(px(12.))
        .border_1()
        .border_color(theme.border)
        .bg(theme.colors.group_box)
        .overflow_hidden()
        .child(div().px_6().py_4().border_b_1().border_color(theme.border).text_base().font_semibold().child(title.to_string()))
}

/// A padded card.
fn card(cx: &App) -> Div {
    let theme = cx.theme();
    v_flex().p_6().gap_6().rounded(px(12.)).border_1().border_color(theme.border).bg(theme.colors.group_box)
}

fn card_title(text: &str) -> Div {
    div().text_base().font_semibold().child(text.to_string())
}

// Statistics ---------------------------------------------------------------------

const H: f32 = 168.;
const TOP: f32 = 8.;
const LEFT: f32 = 60.;
const RIGHT: f32 = 12.;
const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];

#[derive(Clone, Copy, Debug, PartialEq)]
struct Slot {
    at: i64,
    lan: i64,
    wan: i64,
}

/// Buckets on a continuous axis aligned to the server's own boundaries, so quiet periods show as gaps.
fn slots(buckets: &[Bucket], since: i64, step: i64, now: i64) -> Vec<Slot> {
    let end = now.max(buckets.last().map_or(0, |b| b.at));
    let base = buckets.first().map_or(since, |b| b.at);
    let start = base - ((base - since) / step).max(0) * step;
    let n = (((end - start) / step) + 1).max(1) as usize;
    let mut out: Vec<Slot> = (0..n).map(|i| Slot { at: start + i as i64 * step, lan: 0, wan: 0 }).collect();
    for b in buckets {
        let i = (b.at - start).div_euclid(step);
        if i >= 0 && (i as usize) < n {
            out[i as usize].lan += b.lan;
            out[i as usize].wan += b.wan;
        }
    }
    out
}

/// The y axis in the largest binary unit that fits, with 1/2/2.5/5 steps: the top value and the ticks.
fn scale(max: i64) -> (f64, Vec<(f64, String)>) {
    let max = max.max(1) as f64;
    let mut k = 0;
    while k < UNITS.len() - 1 && max / 1024f64.powi(k as i32 + 1) >= 1. {
        k += 1;
    }
    let unit = 1024f64.powi(k as i32);
    let raw = max / unit / 3.;
    let mag = 10f64.powf(raw.log10().floor());
    let inc = [1., 2., 2.5, 5., 10.].iter().map(|m| m * mag).find(|s| *s >= raw).unwrap_or(10. * mag);
    let count = ((max / unit / inc - 1e-9).ceil() as usize).max(1);
    let ticks = (0..=count)
        .map(|i| {
            let v = ((i as f64 * inc) * 1000.).round() / 1000.;
            (v * unit, if v == 0. { "0".to_string() } else { format!("{v} {}", UNITS[k]) })
        })
        .collect();
    (count as f64 * inc * unit, ticks)
}

/// "12.5" hours below ten, whole hours above.
fn hours(secs: i64) -> String {
    let h = secs as f64 / 3600.;
    if h < 10. { format!("{h:.1}") } else { format::count(h.round() as i64) }
}

fn local(ts: i64, fmt: &str) -> String {
    Local.timestamp_opt(ts, 0).single().map(|d| d.format(fmt).to_string()).unwrap_or_default()
}

fn every(days: Option<i64>) -> String {
    match days {
        None | Some(0) => String::new(),
        Some(1) => "Daily".into(),
        Some(n) => format!("Every {n} days"),
    }
}

// The page -------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pane {
    Overview,
    Libraries,
    Maintenance,
    Settings,
    Users,
}

const PANES: [(Pane, &str); 5] = [
    (Pane::Overview, "Overview"),
    (Pane::Libraries, "Libraries"),
    (Pane::Maintenance, "Maintenance"),
    (Pane::Settings, "Settings"),
    (Pane::Users, "Users & devices"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Period {
    Day,
    Month,
}

struct StatsView {
    stats: Stats,
    since: i64,
    step: i64,
}

pub struct Server {
    pane: Pane,
    period: Period,
    overview: Res<Rc<Overview>>,
    stats: Res<Rc<StatsView>>,
    maintenance: Res<Maintenance>,
    prefs: Res<Vec<PrefGroup>>,
    people: Res<Rc<UsersDevices>>,
    filter: Entity<InputState>,
    advanced: bool,
    group: Option<String>,
    saving: Option<&'static str>,
    table: bool,
    hover: Option<usize>,
    text_prefs: TextPrefs,
    _subs: Vec<Subscription>,
}

impl Server {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Search all settings"));
        let subs = vec![cx.subscribe_in(&filter, window, |_, _, e: &InputEvent, _, cx| {
            if matches!(e, InputEvent::Change) {
                cx.notify();
            }
        })];
        let mut this = Self {
            pane: Pane::Overview,
            period: Period::Day,
            overview: Res::default(),
            stats: Res::default(),
            maintenance: Res::default(),
            prefs: Res::default(),
            people: Res::default(),
            filter,
            advanced: false,
            group: None,
            saving: None,
            table: false,
            hover: None,
            text_prefs: TextPrefs::default(),
            _subs: subs,
        };
        this.load(cx);
        this
    }

    /// Fetch what the open tab shows.
    fn load(&mut self, cx: &mut Context<Self>) {
        let app = core(cx);
        match self.pane {
            Pane::Overview => {
                self.overview.loading = true;
                let a = app.clone();
                cx.spawn(async move |this, cx| {
                    let result = bg(async move { ops::server_overview(&a).await }).await;
                    this.update(cx, |s, cx| {
                        s.overview.set(result.map(Rc::new));
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
                self.load_stats(cx);
            }
            Pane::Libraries => {}
            Pane::Maintenance => {
                self.maintenance.loading = true;
                cx.spawn(async move |this, cx| {
                    let result = bg(async move { ops::maintenance(&app).await }).await;
                    this.update(cx, |s, cx| {
                        s.maintenance.set(result);
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
            Pane::Settings => {
                self.prefs.loading = true;
                cx.spawn(async move |this, cx| {
                    let result = bg(async move { ops::settings(&app).await }).await;
                    this.update(cx, |s, cx| {
                        s.prefs.set(result);
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
            Pane::Users => {
                self.people.loading = true;
                cx.spawn(async move |this, cx| {
                    let result = bg(async move { ops::users_devices(&app).await }).await;
                    this.update(cx, |s, cx| {
                        s.people.set(result.map(Rc::new));
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
        }
    }

    fn load_stats(&mut self, cx: &mut Context<Self>) {
        self.stats.loading = true;
        let app = core(cx);
        let now = format::now();
        // Hourly buckets for a day, daily ones for a month.
        let (timespan, step, since) = match self.period {
            Period::Day => (6, 3600, now - 86_400),
            Period::Month => (4, 86_400, now - 30 * 86_400),
        };
        let period = self.period;
        cx.spawn(async move |this, cx| {
            let result = bg(async move { ops::server_stats(&app, timespan, since).await }).await;
            this.update(cx, |s, cx| {
                if s.period == period {
                    s.stats.set(result.map(|stats| Rc::new(StatsView { stats, since, step })));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn set_pane(&mut self, pane: Pane, cx: &mut Context<Self>) {
        if self.pane != pane {
            self.pane = pane;
            self.load(cx);
            cx.notify();
        }
    }

    /// Save a server setting and keep the shown value in step.
    fn save(&mut self, id: String, value: String, cx: &mut Context<Self>) {
        let app = core(cx);
        let weak = cx.entity().downgrade();
        let (i, v) = (id.clone(), value.clone());
        store::act_then(cx, async move { Ok::<_, anyhow::Error>(app.server()?.set_pref(&i, &v).await?) }, move |_, cx| {
            store::toast(cx, "Setting saved");
            weak.update(cx, |s, cx| {
                s.saved(&id, &value);
                cx.notify();
            })
            .ok();
        });
    }

    fn saved(&mut self, id: &str, value: &str) {
        let set = |p: &mut Pref| {
            if p.id == id {
                p.value = value.to_string();
            }
        };
        if let Some(groups) = &mut self.prefs.data {
            groups.iter_mut().flat_map(|g| g.prefs.iter_mut()).for_each(set);
        }
        if let Some(m) = &mut self.maintenance.data {
            m.schedule.iter_mut().for_each(set);
            for t in &mut m.tasks {
                if let Some(p) = &mut t.pref
                    && p.id == id
                {
                    p.value = value.to_string();
                    t.task.enabled = is_on(value);
                }
            }
        }
    }

    fn saver(&self, id: &str, cx: &mut Context<Self>) -> Save {
        let (id, weak) = (id.to_string(), cx.entity().downgrade());
        Rc::new(move |value, cx| {
            weak.update(cx, |s, cx| s.save(id.clone(), value, cx)).ok();
        })
    }

    fn row(&mut self, pref: &Pref, show_id: bool, first: bool, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let input = (pref.kind != "bool" && pref.options.is_empty()).then(|| self.text_prefs.input(pref, window, cx, |s, id, value, cx| s.save(id, value, cx)));
        let save = self.saver(&pref.id, cx);
        pref_row(pref, show_id, first, input, save, cx)
    }

    /// Loading, or the error with a way to try again.
    fn status<T>(&self, res: &Res<T>, cx: &mut Context<Self>) -> AnyElement {
        match &res.error {
            Some(e) => widgets::failed(e.clone(), cx).child(Button::new("retry").outline().small().mt_3().label("Try again").on_click(cx.listener(|this, _, _, cx| this.load(cx)))).into_any_element(),
            None => widgets::loading().into_any_element(),
        }
    }

    fn act(cx: &mut App, fut: impl Future<Output = anyhow::Result<()>> + Send + 'static, ok: &str) {
        store::act(cx, fut, Some(ok.to_string()), false);
    }

    // Overview -------------------------------------------------------------------

    fn overview_view(&self, o: &Overview, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let tint = store(cx).read(cx).tint();
        let muted = theme.muted_foreground;
        let wide = window.viewport_size().width >= px(1024.);
        let gauges = [("Host CPU", o.host_cpu), ("Plex CPU", o.plex_cpu), ("Host memory", o.host_memory), ("Plex memory", o.plex_memory)];
        let gauges = div().grid().grid_cols(if wide { 4 } else { 2 }).gap_4().children(gauges.into_iter().enumerate().map(|(i, (label, value))| {
            v_flex()
                .gap_1()
                .p_4()
                .rounded(px(12.))
                .border_1()
                .border_color(theme.border)
                .bg(theme.colors.group_box)
                .child(div().text_sm().text_color(muted).child(label))
                .child(div().text_2xl().font_semibold().child(format!("{value:.0}%")))
                .child(div().mt_3().child(Progress::new(("gauge", i)).value(value as f32).color(tint)))
        }));
        let dash = |v: &Option<String>| v.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| "—".into());
        let facts = [
            ("Version", dash(&o.version)),
            ("Platform", dash(&o.platform)),
            ("Owner", dash(&o.owner)),
            ("Plex Pass", if o.plex_pass { "Active".into() } else { "No".into() }),
            ("Connected via", o.connected_via.clone()),
            ("Machine ID", dash(&o.machine_identifier)),
            ("Transcoder", if o.transcoder { "Available".into() } else { "—".into() }),
        ];
        let server = card(cx).child(card_title("Server")).child(v_flex().gap_2().text_sm().children(facts.into_iter().map(|(k, v)| {
            h_flex().items_start().gap_5().child(div().w(px(140.)).flex_none().text_color(muted).child(k)).child(div().flex_1().min_w_0().child(v))
        })));
        let release = o.release.clone();
        let can_install = o.can_install;
        let updates = v_flex()
            .rounded(px(12.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.colors.group_box)
            .child(
                v_flex().flex_1().p_6().gap_6().child(card_title("Updates")).child(match release {
                    Some(r) => v_flex()
                        .text_sm()
                        .child(format!("Version {} is available.", r.version.clone().unwrap_or_default()))
                        .when_some(r.added.clone().filter(|s| !s.is_empty()), |d, added| {
                            d.child(div().id("release-notes").mt_2p5().max_h(px(160.)).overflow_y_scroll().text_xs().text_color(muted).child(added))
                        })
                        .child(
                            h_flex()
                                .mt_4()
                                .gap_2()
                                .when(can_install, |d| {
                                    d.child(Button::new("install").primary().small().label("Install now").on_click(|_, _, cx| {
                                        let app = core(cx);
                                        Self::act(cx, async move { Ok(app.server()?.apply_update(false, false).await?) }, "Installing update…");
                                    }))
                                    .child(Button::new("tonight").outline().small().label("Tonight").on_click(|_, _, cx| {
                                        let app = core(cx);
                                        Self::act(cx, async move { Ok(app.server()?.apply_update(true, false).await?) }, "Update scheduled for tonight");
                                    }))
                                })
                                .when_some(r.download_url.clone().filter(|_| !can_install), |d, url| {
                                    d.child(Button::new("download-update").outline().small().icon(IconName::ExternalLink).label("Download").on_click(move |_, _, cx| cx.open_url(&url)))
                                }),
                        )
                        .into_any_element(),
                    None => div().text_sm().text_color(muted).child("The server is up to date.").into_any_element(),
                }),
            )
            .child(
                h_flex().px_6().py_4().border_t_1().border_color(theme.border).child(Button::new("check-updates").outline().small().label("Check now").on_click(cx.listener(|_, _, _, cx| {
                    let (app, weak) = (core(cx), cx.entity().downgrade());
                    store::act_then(cx, async move { Ok::<_, anyhow::Error>(app.server()?.check_for_updates(true).await?) }, move |_, cx| {
                        store::toast(cx, "Checked for updates");
                        weak.update(cx, |s, cx| s.load(cx)).ok();
                    });
                }))),
            );
        let period = self.period;
        let toggle = h_flex().child(
            h_flex()
                .child(Button::new("period-day").outline().small().label("Last 24 hours").selected(period == Period::Day).on_click(cx.listener(|this, _, _, cx| this.set_period(Period::Day, cx))))
                .child(Button::new("period-month").outline().small().label("Last 30 days").selected(period == Period::Month).on_click(cx.listener(|this, _, _, cx| this.set_period(Period::Month, cx)))),
        );
        let streaming_body = match &self.stats.data {
            None => self.status(&self.stats, cx),
            Some(sv) => {
                let s = &sv.stats;
                let body = if s.bandwidth.is_empty() && s.plays == 0 {
                    widgets::empty_box(
                        IconName::SearchX,
                        Some("No streaming".into()),
                        Some(format!("Nothing was streamed from this server {}.", if period == Period::Day { "in the last 24 hours" } else { "in the last 30 days" }).into()),
                        cx,
                    )
                    .into_any_element()
                } else {
                    let sent: i64 = s.bandwidth.iter().map(|b| b.lan + b.wan).sum();
                    let boxes = [(format::bytes(sent), "Data sent"), (format::count(s.plays), "Plays"), (hours(s.play_seconds), "Hours played")];
                    v_flex()
                        .gap_6()
                        .child(div().grid().grid_cols(3).gap_4().children(boxes.into_iter().map(|(value, label)| {
                            v_flex()
                                .px_4()
                                .py_3()
                                .rounded(px(8.))
                                .border_1()
                                .border_color(theme.border)
                                .child(div().text_xs().font_medium().text_color(muted).child(label))
                                .child(div().mt_1().text_2xl().font_semibold().child(value))
                        })))
                        .when(!s.bandwidth.is_empty(), |d| d.child(self.bandwidth(&s.bandwidth, sv.since, sv.step, cx)))
                        .into_any_element()
                };
                div().when(self.stats.loading, |d| d.opacity(0.5)).child(body).into_any_element()
            }
        };
        let streaming = card(cx).child(h_flex().justify_between().items_center().child(card_title("Streaming")).child(toggle)).child(streaming_body);
        let rankings = self.stats.data.as_ref().filter(|sv| !sv.stats.bandwidth.is_empty() || sv.stats.plays > 0).map(|sv| {
            div()
                .grid()
                .grid_cols(if wide { 2 } else { 1 })
                .gap_6()
                .when(self.stats.loading, |d| d.opacity(0.5))
                .child(card(cx).child(card_title("Top users")).child(ranking(&sv.stats.accounts, "No data sent to users.", tint, cx)))
                .child(card(cx).child(card_title("Top devices")).child(ranking(&sv.stats.devices, "No data sent to devices.", tint, cx)))
        });
        v_flex()
            .gap_6()
            .child(gauges)
            .child(div().grid().grid_cols(if wide { 2 } else { 1 }).gap_6().child(server).child(updates))
            .child(streaming)
            .children(rankings)
            .into_any_element()
    }

    fn set_period(&mut self, period: Period, cx: &mut Context<Self>) {
        if self.period != period {
            self.period = period;
            self.hover = None;
            self.load_stats(cx);
            cx.notify();
        }
    }

    /// Data sent per time bucket as stacked columns: remote (accent) on the baseline,
    /// local network (neutral) above it. Hover a column for its values; the same
    /// numbers are available as a table.
    fn bandwidth(&self, buckets: &[Bucket], since: i64, step: i64, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let tint = store(cx).read(cx).tint();
        let muted = theme.muted_foreground;
        let lan_color = theme::faint();
        let slots = slots(buckets, since, step, format::now());
        let hourly = step < 86_400;
        let (lan, wan) = slots.iter().fold((0, 0), |(l, w), s| (l + s.lan, w + s.wan));
        let key = |c: Hsla| div().size_2().flex_none().rounded(px(2.)).bg(c);
        let full = |ts: i64| local(ts, if hourly { "%a %-I:%M %p" } else { "%a, %b %-d" });
        let legend = h_flex()
            .mb_4()
            .items_center()
            .gap_5()
            .text_xs()
            .text_color(muted)
            .child(h_flex().gap_2().child(key(tint)).child("Remote").child(div().font_medium().text_color(theme.foreground).child(format::bytes(wan))))
            .child(h_flex().gap_2().child(key(lan_color)).child("Local network").child(div().font_medium().text_color(theme.foreground).child(format::bytes(lan))))
            .child(div().flex_1())
            .child(Button::new("chart-table").ghost().xsmall().text_color(muted).label(if self.table { "Show chart" } else { "Show as table" }).on_click(cx.listener(|this, _, _, cx| {
                this.table = !this.table;
                cx.notify();
            })));
        if self.table {
            let cell = |t: String, first: bool| div().flex_1().px_3().py_2().when(!first, |d| d.text_right()).child(t);
            let head = h_flex()
                .text_xs()
                .font_medium()
                .text_color(muted)
                .border_b_1()
                .border_color(theme.border)
                .bg(theme.colors.group_box)
                .child(cell(if hourly { "Hour" } else { "Day" }.into(), true))
                .child(cell("Remote".into(), false))
                .child(cell("Local network".into(), false))
                .child(cell("Total".into(), false));
            let rows = slots.iter().rev().filter(|s| s.lan + s.wan > 0).enumerate().map(|(i, s)| {
                h_flex()
                    .text_sm()
                    .text_color(muted)
                    .when(i > 0, |d| d.border_t_1().border_color(theme.border))
                    .child(cell(full(s.at), true))
                    .child(cell(format::bytes(s.wan), false))
                    .child(cell(format::bytes(s.lan), false))
                    .child(cell(format::bytes(s.lan + s.wan), false))
            });
            return v_flex()
                .child(legend)
                .child(v_flex().rounded(px(8.)).border_1().border_color(theme.border).overflow_hidden().child(head).child(v_flex().id("chart-rows").max_h(px(260.)).overflow_y_scroll().children(rows)))
                .into_any_element();
        }

        let (top, ticks) = scale(slots.iter().map(|s| s.lan + s.wan).max().unwrap_or(0));
        let n = slots.len();
        let every = if hourly { 6 } else { 7 };
        let y = |v: f64| TOP + H - (v / top) as f32 * H;
        let grid = ticks.iter().map(|(v, label)| {
            let top = y(*v);
            div()
                .absolute()
                .top(px(top))
                .left_0()
                .right(px(RIGHT))
                .h_px()
                .child(div().absolute().left(px(LEFT)).right_0().h_px().bg(theme.border))
                .child(div().absolute().left_0().w(px(LEFT - 10.)).top(px(-7.)).text_right().text_size(px(10.5)).text_color(muted).child(label.clone()))
        });
        let hover = self.hover;
        let columns = slots.iter().enumerate().map(|(i, s)| {
            let total = (s.lan + s.wan) as f64;
            let frac = |v: i64| (v as f64 / top) as f32;
            let tip = (hover == Some(i)).then(|| {
                let right = i > n / 2;
                deferred(
                    v_flex()
                        .absolute()
                        .top(px(-TOP))
                        .when(right, |d| d.right(relative(0.5)).mr(px(14.)))
                        .when(!right, |d| d.left(relative(0.5)).ml(px(14.)))
                        .min_w(px(150.))
                        .px_2p5()
                        .py_2()
                        .gap_0p5()
                        .rounded(px(8.))
                        .bg(theme.popover)
                        .border_1()
                        .border_color(hsla(0., 0., 1., 0.1))
                        .shadow_md()
                        .text_xs()
                        .whitespace_nowrap()
                        .child(div().mb_1().text_color(muted).child(full(s.at)))
                        .map(|d| {
                            if total == 0. {
                                d.child(div().text_color(muted).child("Nothing streamed"))
                            } else {
                                d.child(h_flex().gap_2().child(key(tint)).child(div().font_medium().child(format::bytes(s.wan))).child("remote"))
                                    .child(h_flex().gap_2().child(key(lan_color)).child(div().font_medium().child(format::bytes(s.lan))).child("local network"))
                            }
                        }),
                )
                .with_priority(1)
            });
            div()
                .id(("bw-col", i))
                .relative()
                .flex_1()
                .h_full()
                .flex()
                .items_end()
                .justify_center()
                .on_hover(cx.listener(move |this, on: &bool, _, cx| {
                    if *on {
                        this.hover = Some(i);
                    } else if this.hover == Some(i) {
                        this.hover = None;
                    }
                    cx.notify();
                }))
                .child(
                    v_flex()
                        .w(relative(0.62))
                        .max_w(px(24.))
                        .min_w(px(2.))
                        .h_full()
                        .justify_end()
                        .when(hover.is_some_and(|h| h != i), |d| d.opacity(0.45))
                        .when(s.lan > 0, |d| {
                            d.child(div().w_full().h(relative(frac(s.lan))).min_h(px(1.)).bg(lan_color).rounded_t(px(4.)).when(s.wan > 0, |d| d.mb(px(2.))))
                        })
                        .when(s.wan > 0, |d| d.child(div().w_full().h(relative(frac(s.wan))).min_h(px(1.)).bg(tint).when(s.lan == 0, |d| d.rounded_t(px(4.))))),
                )
                .children(tip)
        });
        let labels = slots.iter().enumerate().map(|(i, s)| {
            let show = (n - 1 - i).is_multiple_of(every);
            div()
                .flex_1()
                .flex()
                .justify_center()
                .whitespace_nowrap()
                .text_size(px(10.5))
                .text_color(muted)
                .when(show, |d| d.child(local(s.at, if hourly { "%-I %p" } else { "%b %-d" })))
        });
        v_flex()
            .child(legend)
            .child(
                div()
                    .relative()
                    .h(px(TOP + H + 24.))
                    .children(grid)
                    .child(h_flex().absolute().top(px(TOP)).left(px(LEFT)).right(px(RIGHT)).h(px(H)).children(columns))
                    .child(h_flex().absolute().top(px(TOP + H + 6.)).left(px(LEFT)).right(px(RIGHT)).children(labels)),
            )
            .id("bandwidth")
            .on_hover(cx.listener(|this, on: &bool, _, cx| {
                if !*on && this.hover.is_some() {
                    this.hover = None;
                    cx.notify();
                }
            }))
            .into_any_element()
    }

    // Libraries ------------------------------------------------------------------

    fn libraries_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let s = store(cx).read(cx);
        let (tint, sections, activities) = (s.tint(), s.sections.clone(), s.activities.clone());
        let muted = theme.muted_foreground;
        let head = h_flex()
            .mb_4()
            .items_center()
            .justify_between()
            .gap_4()
            .child(div().text_color(muted).child("Scan for new files, change folders and settings, or add a library."))
            .child(Button::new("add-library").outline().small().icon(IconName::Plus).label("Add library").on_click(|_, window, cx| server_library::open(None, window, cx)));
        let list = if sections.is_empty() {
            widgets::empty_box(IconName::FolderOpen, Some("No libraries".into()), Some("This server has no libraries yet.".into()), cx).into_any_element()
        } else {
            widgets::rows(cx)
                .flex_none()
                .children(sections.iter().enumerate().map(|(i, d)| self.library_row(d, i, activities.iter().find(|a| a.section_id.as_deref() == d.key.as_deref()).cloned(), tint, cx)))
                .into_any_element()
        };
        v_flex()
            .child(head)
            .child(list)
            .child(
                h_flex()
                    .mt_4()
                    .gap_2()
                    .child(Button::new("scan-all").primary().small().label("Scan all").on_click(|_, _, cx| {
                        let app = core(cx);
                        Self::act(cx, async move { Ok(app.server()?.refresh_all_sections(false).await?) }, "Scanning all libraries…");
                    }))
                    .child(Button::new("reload-list").ghost().small().label("Reload list").on_click(|_, _, cx| {
                        let app = core(cx);
                        store::act(cx, async move { session::reload_sections(&app).await }, None, false);
                    })),
            )
            .into_any_element()
    }

    fn library_row(&self, d: &Directory, i: usize, task: Option<patina_core::state::ServerActivity>, tint: Hsla, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let id = d.key.clone().unwrap_or_default();
        let title = d.title.clone().unwrap_or_default();
        let paths = d.location.iter().filter_map(|l| l.path.clone()).collect::<Vec<_>>().join(", ");
        let busy = task.is_some() || d.refreshing;
        let note = |t: String| div().mt_0p5().text_size(px(13.)).text_color(muted).child(t);
        let (id1, id2, d1, d2, d3) = (id.clone(), id.clone(), d.clone(), d.clone(), d.clone());
        h_flex()
            .px_4()
            .py_3p5()
            .gap(px(28.))
            .items_center()
            .justify_between()
            .when(i > 0, |r| r.border_t_1().border_color(theme.border))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().font_medium().child(title.clone()))
                    .child(note(paths))
                    .map(|c| match &task {
                        Some(t) => c
                            .child(note(format!("{} · {}%", t.subtitle.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| t.title.clone()), t.progress)))
                            .child(div().mt_2().max_w(px(360.)).child(Progress::new(("lib-scan", i)).value(t.progress as f32).color(tint))),
                        None if d.refreshing => c.child(note("Scanning…".into())),
                        None => c.when_some(d.scanned_at, |c, at| c.child(note(format!("Last scanned {}", format::ago(at))))),
                    }),
            )
            .child(
                h_flex()
                    .flex_none()
                    .items_center()
                    .gap_2()
                    .child(if busy {
                        Button::new(("lib-cancel", i)).outline().small().label("Cancel").on_click(move |_, _, cx| {
                            let (app, id) = (core(cx), id1.clone());
                            Self::act(cx, async move { Ok(app.server()?.cancel_refresh_section(&id).await?) }, "Scan cancelled");
                        })
                    } else {
                        Button::new(("lib-scan", i)).outline().small().label("Scan").on_click(move |_, _, cx| {
                            let (app, id) = (core(cx), id1.clone());
                            Self::act(cx, async move { Ok(app.server()?.refresh_section(&id, false, None).await?) }, "Scanning…");
                        })
                    })
                    .child(Button::new(("lib-edit", i)).outline().small().label("Edit").on_click(move |_, window, cx| server_library::open(Some(d1.clone()), window, cx)))
                    .child(Button::new(("lib-prefs", i)).outline().small().label("Settings").on_click(move |_, window, cx| server_library::open_prefs(d2.clone(), window, cx)))
                    .child(widgets::icon_button(("lib-more", i), IconName::Ellipsis, "More").dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
                        let run = |action: &'static str, ok: &'static str, id: String| {
                            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                                let (app, id) = (core(cx), id.clone());
                                Self::act(
                                    cx,
                                    async move {
                                        let s = app.server()?;
                                        match action {
                                            "refresh" => s.refresh_section(&id, true, None).await?,
                                            "analyze" => s.analyze_section(&id).await?,
                                            _ => s.empty_trash(&id).await?,
                                        }
                                        Ok(())
                                    },
                                    ok,
                                );
                            }
                        };
                        let d = d3.clone();
                        menu.min_w(px(220.))
                            .item(PopupMenuItem::new("Refresh all metadata").icon(IconName::RefreshCw).on_click(run("refresh", "Refreshing all metadata…", id2.clone())))
                            .item(PopupMenuItem::new("Analyze media").icon(IconName::ScanSearch).on_click(run("analyze", "Analyzing…", id2.clone())))
                            .item(PopupMenuItem::new("Empty trash").icon(IconName::Trash).on_click(run("emptyTrash", "Trash emptied", id2.clone())))
                            .separator()
                            .item(PopupMenuItem::new("Delete library…").icon(IconName::Trash).on_click(move |_, window, cx| Self::delete(&d, window, cx)))
                    })),
            )
            .into_any_element()
    }

    fn delete(d: &Directory, window: &mut Window, cx: &mut App) {
        let title = d.title.clone().unwrap_or_default();
        let id = d.key.clone().unwrap_or_default();
        widgets::confirm(
            format!("Delete “{title}”?"),
            Some("The library and its metadata are removed from the server. Media files on disk are not touched.".into()),
            "Delete library",
            true,
            window,
            cx,
            move |_, cx| {
                let (app, id) = (core(cx), id.clone());
                Self::act(cx, async move { ops::delete_library(&app, &id).await }, &format!("Deleted “{title}”"));
            },
        );
    }

    // Maintenance ----------------------------------------------------------------

    fn maintenance_view(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let action = |id: &'static str, title: &str, desc: &str, first: bool, ok: &'static str| {
            widgets::setting(
                first,
                title.to_string(),
                Some(desc.to_string().into()),
                Some(
                    Button::new(id)
                        .outline()
                        .small()
                        .label("Run")
                        .on_click(move |_, _, cx| {
                            let app = core(cx);
                            Self::act(
                                cx,
                                async move {
                                    let s = app.server()?;
                                    if id == "clean-bundles" { s.clean_bundles().await? } else { s.optimize_database().await? }
                                    Ok(())
                                },
                                ok,
                            );
                        })
                        .into_any_element(),
                ),
                cx,
            )
        };
        let database = titled_card("Database", cx)
            .child(action("clean-bundles", "Clean bundles", "Delete metadata bundles for media that no longer exists.", true, "Cleaning bundles…"))
            .child(action("optimize-db", "Optimize database", "Compact and re-index the library database.", false, "Optimizing database…"));

        let header = h_flex()
            .px_6()
            .py_4()
            .gap_4()
            .items_start()
            .justify_between()
            .border_b_1()
            .border_color(theme.border)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1p5()
                    .child(div().text_base().font_semibold().child("Scheduled tasks"))
                    .child(div().text_sm().text_color(muted).child("Plex's butler runs these during the maintenance window. Switch off any you don't want, or run one now.")),
            )
            .child(
                h_flex()
                    .flex_none()
                    .gap_1()
                    .child(Button::new("start-all").ghost().small().label("Start all").on_click(|_, _, cx| {
                        let app = core(cx);
                        Self::act(cx, async move { Ok(app.server()?.start_all_butler_tasks().await?) }, "Started all tasks");
                    }))
                    .child(Button::new("stop-all").ghost().small().label("Stop all").on_click(|_, _, cx| {
                        let app = core(cx);
                        Self::act(cx, async move { Ok(app.server()?.stop_all_butler_tasks().await?) }, "Stopped all tasks");
                    })),
            );
        let tasks_body = match self.maintenance.data.clone() {
            None => self.status(&self.maintenance, cx),
            Some(m) => {
                let mut list = v_flex();
                let mut first = true;
                for p in &m.schedule {
                    list = list.child(self.row(p, false, first, window, cx));
                    first = false;
                }
                for (i, t) in m.tasks.iter().enumerate() {
                    let name = t.task.name.clone().unwrap_or_default();
                    let title = t.task.title.clone().unwrap_or_else(|| name.clone());
                    let ok = format!("Started {title}");
                    let control = h_flex()
                        .gap_2()
                        .items_center()
                        .when(t.task.interval.is_some_and(|d| d > 0), |d| d.child(div().mr_2().text_xs().text_color(muted).child(every(t.task.interval))))
                        .child(Button::new(("run-task", i)).outline().small().label("Run now").on_click(move |_, _, cx| {
                            let (app, name) = (core(cx), name.clone());
                            Self::act(cx, async move { Ok(app.server()?.start_butler_task(&name).await?) }, &ok);
                        }))
                        .map(|d| match &t.pref {
                            Some(p) => {
                                let save = self.saver(&p.id, cx);
                                let on = is_on(&p.value);
                                d.child(
                                    Switch::new(("task-switch", i))
                                        .checked(on)
                                        .tooltip(if on { "Runs on schedule" } else { "Off" })
                                        .on_click(move |on: &bool, _, cx| save(if *on { "true".into() } else { "false".into() }, cx)),
                                )
                            }
                            None if !t.task.enabled => d.child(widgets::badge("Off", cx)),
                            None => d.child(div().w_8()),
                        });
                    list = list.child(widgets::setting(first, title, t.task.description.clone().filter(|s| !s.is_empty()).map(Into::into), Some(control.into_any_element()), cx));
                    first = false;
                }
                list.into_any_element()
            }
        };
        let tasks = v_flex().flex_none().rounded(px(12.)).border_1().border_color(theme.border).bg(theme.colors.group_box).overflow_hidden().child(header).child(tasks_body);

        let saving = self.saving;
        let diagnostics = titled_card("Diagnostics", cx).children(
            [
                ("logs", "Download logs", "Server logs as a zip, for troubleshooting or a support request."),
                ("database", "Download database backup", "A copy of the library database. Large libraries take a while."),
            ]
            .into_iter()
            .enumerate()
            .map(|(i, (what, label, desc))| {
                let control = h_flex()
                    .gap_2()
                    .when(saving == Some(what), |d| d.child(Spinner::new().color(muted)))
                    .child(
                        Button::new(("diagnostics", i))
                            .outline()
                            .small()
                            .label(if saving == Some(what) { "Saving…" } else { "Download" })
                            .disabled(saving.is_some())
                            .on_click(cx.listener(move |this, _, _, cx| this.diagnostics(what, cx))),
                    );
                widgets::setting(i == 0, label, Some(desc.into()), Some(control.into_any_element()), cx)
            }),
        );
        v_flex().gap_6().child(database).child(tasks).child(diagnostics).into_any_element()
    }

    /// Diagnostics can take a while on big servers.
    fn diagnostics(&mut self, what: &'static str, cx: &mut Context<Self>) {
        self.saving = Some(what);
        cx.notify();
        let app = core(cx);
        cx.spawn(async move |this, cx| {
            let result = bg(async move { ops::save_diagnostics(&app, what).await }).await;
            this.update(cx, |s, cx| {
                s.saving = None;
                match result {
                    Ok(path) => {
                        if let Some(dir) = path.parent() {
                            cx.open_with_system(dir);
                        }
                        store::toast(cx, format!("Saved to {}", path.display()));
                    }
                    Err(e) => store::error(cx, message(&e)),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // Settings -------------------------------------------------------------------

    fn settings_view(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(all) = self.prefs.data.clone() else { return self.status(&self.prefs, cx) };
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let filter = self.filter.read(cx).value().trim().to_lowercase();
        let searching = !filter.is_empty();
        let groups = filter_groups(&all, &filter, self.advanced);
        let bar = h_flex()
            .mb_6()
            .items_center()
            .gap_4()
            .child(div().w_80().child(Input::new(&self.filter).prefix(Icon::new(IconName::Search).size_4().text_color(muted)).cleanable(true)))
            .child(Switch::new("server-advanced").checked(self.advanced).label("Show advanced").on_click(cx.listener(|this, on: &bool, _, cx| {
                this.advanced = *on;
                cx.notify();
            })));
        if groups.is_empty() {
            let text = if searching { format!("No settings match “{}”.", self.filter.read(cx).value().trim()) } else { "No settings.".into() };
            return v_flex().child(bar).child(widgets::empty_box(IconName::SearchX, Some("No results".into()), Some(text.into()), cx)).into_any_element();
        }
        let current = groups.iter().find(|g| Some(&g.id) == self.group.as_ref()).unwrap_or(&groups[0]).id.clone();
        let nav = v_flex().w(px(180.)).flex_none().gap_1().children(groups.iter().enumerate().map(|(i, g)| {
            let active = !searching && g.id == current;
            let id = g.id.clone();
            Button::new(("pref-group", i))
                .when(active, |b| b.with_variant(gpui_kit::component::button::ButtonVariant::Secondary))
                .when(!active, |b| b.ghost())
                .w_full()
                .child(h_flex().w_full().justify_between().child(div().child(g.label.clone())).child(div().text_size(px(11.)).text_color(muted).child(g.prefs.len().to_string())))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.group = Some(id.clone());
                    this.filter.update(cx, |s, cx| s.set_value("", window, cx));
                    cx.notify();
                }))
        }));
        let shown: Vec<PrefGroup> = if searching { groups.clone() } else { groups.iter().filter(|g| g.id == current).cloned().collect() };
        let advanced = self.advanced;
        let mut cards = v_flex().flex_1().min_w_0().gap_6();
        for g in &shown {
            let mut c = titled_card(&g.label, cx);
            for (i, p) in g.prefs.iter().enumerate() {
                c = c.child(self.row(p, advanced, i == 0, window, cx));
            }
            cards = cards.child(c);
        }
        v_flex().child(bar).child(h_flex().items_start().gap_8().child(nav).child(cards)).into_any_element()
    }

    // Users & devices --------------------------------------------------------------

    fn users_view(&self, d: &UsersDevices, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let wide = window.viewport_size().width >= px(1024.);
        let line = |first: bool, title: String, sub: String| widgets::setting(first, title, Some(sub.into()), None, cx);
        let accounts = titled_card("Accounts", cx).children(d.accounts.iter().filter(|a| a.id.unwrap_or(0) > 0).enumerate().map(|(i, a)| {
            line(
                i == 0,
                a.name.clone().unwrap_or_default(),
                format!(
                    "Audio: {} · Subtitles: {}",
                    a.default_audio_language.clone().unwrap_or_else(|| "auto".into()),
                    a.default_subtitle_language.clone().unwrap_or_else(|| "auto".into())
                ),
            )
        }));
        let players = titled_card("Players on the network", cx)
            .when(d.clients.is_empty(), |c| c.child(widgets::empty_box(IconName::Users, None, Some("No players are advertising on the local network right now.".into()), cx).border_0()))
            .children(d.clients.iter().enumerate().map(|(i, c)| {
                line(i == 0, c.name.clone().unwrap_or_default(), format!("{} · {}", c.product.clone().unwrap_or_default(), c.address.clone().unwrap_or_default()))
            }));
        let friends = titled_card("Friends & shared users", cx)
            .when(d.friends.is_empty(), |c| c.child(widgets::empty_box(IconName::Users, None, Some("This server isn't shared with anyone.".into()), cx).border_0()))
            .children(d.friends.iter().enumerate().map(|(i, f)| {
                let name = [&f.title, &f.username, &f.email].into_iter().flatten().find(|s| !s.is_empty()).cloned().unwrap_or_else(|| "User".into());
                let detail = [f.username.clone().filter(|u| *u != name), f.email.clone()].into_iter().flatten().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
                h_flex()
                    .px_4()
                    .py_3p5()
                    .gap(px(28.))
                    .items_center()
                    .justify_between()
                    .when(i > 0, |r| r.border_t_1().border_color(theme.border))
                    .child(
                        h_flex()
                            .gap_3()
                            .min_w_0()
                            .child(
                                div()
                                    .relative()
                                    .size_8()
                                    .flex_none()
                                    .rounded_full()
                                    .overflow_hidden()
                                    .bg(theme.muted)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_xs()
                                    .font_medium()
                                    .text_color(muted)
                                    .child(format::initials(&name))
                                    .children(art_of(f.thumb.as_deref(), 28., 28., None).map(|i| i.absolute().inset_0().size_full())),
                            )
                            .child(v_flex().min_w_0().child(div().font_medium().child(name.clone())).when(!detail.is_empty(), |d| d.child(div().mt_0p5().text_size(px(13.)).text_color(muted).child(detail)))),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .gap_2()
                            .when(f.home, |d| d.child(widgets::badge("Home", cx)))
                            .when(f.restricted, |d| d.child(widgets::badge("Restricted", cx)))
                            .when_some(f.status.clone().filter(|s| !s.is_empty() && s != "accepted"), |d, s| {
                                let mut chars = s.chars();
                                let s = chars.next().map(|c| c.to_uppercase().collect::<String>() + chars.as_str()).unwrap_or_default();
                                d.child(div().px_2().py_0p5().rounded(px(6.)).bg(theme.secondary).text_xs().font_medium().child(s))
                            }),
                    )
            }));
        let devices = titled_card("Devices that have connected", cx).child(v_flex().id("devices").max_h(px(480.)).overflow_y_scroll().children(d.devices.iter().rev().enumerate().map(|(i, dev)| {
            widgets::setting(
                i == 0,
                dev.name.clone().unwrap_or_default(),
                Some(dev.platform.clone().unwrap_or_default().into()),
                Some(div().text_sm().text_color(muted).child(dev.created_at.map(|t| format!("First seen {}", format::date(t))).unwrap_or_default()).into_any_element()),
                cx,
            )
        })));
        v_flex()
            .gap_6()
            .child(div().grid().grid_cols(if wide { 2 } else { 1 }).items_start().gap_6().child(accounts).child(players))
            .child(friends)
            .child(devices)
            .into_any_element()
    }
}

/// Groups with the settings that pass the advanced switch and the filter.
fn filter_groups(all: &[PrefGroup], filter: &str, advanced: bool) -> Vec<PrefGroup> {
    all.iter()
        .map(|g| PrefGroup {
            prefs: g
                .prefs
                .iter()
                .filter(|p| (advanced || !p.advanced) && (filter.is_empty() || [&p.label, &p.id, p.summary.as_ref().unwrap_or(&String::new())].iter().any(|s| s.to_lowercase().contains(filter))))
                .cloned()
                .collect(),
            ..g.clone()
        })
        .filter(|g| !g.prefs.is_empty())
        .collect()
}

/// A ranked list with a thin bar per row, scaled to the largest entry.
fn ranking(rows: &[Usage], empty: &str, tint: Hsla, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let mut top: Vec<&Usage> = rows.iter().collect();
    top.sort_by_key(|r| std::cmp::Reverse(r.bytes));
    top.truncate(5);
    if top.is_empty() {
        return widgets::empty_box(IconName::ChartNoAxesColumn, None, Some(empty.to_string().into()), cx).border_0().p_4().into_any_element();
    }
    let max = top.iter().map(|r| r.bytes).max().unwrap_or(1).max(1) as f32;
    v_flex()
        .gap_3p5()
        .children(top.into_iter().map(|r| {
            v_flex()
                .child(
                    h_flex()
                        .mb_1p5()
                        .justify_between()
                        .gap_3()
                        .text_sm()
                        .child(div().min_w_0().truncate().font_medium().child(r.name.clone()))
                        .child(div().flex_none().text_xs().text_color(theme.muted_foreground).child(format::bytes(r.bytes))),
                )
                .child(div().h_1p5().rounded_full().overflow_hidden().bg(theme.muted).child(div().h_full().min_w(px(2.)).rounded_full().bg(tint).w(relative(r.bytes as f32 / max))))
        }))
        .into_any_element()
}

impl Render for Server {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let s = store(cx).read(cx);
        if !s.owned() {
            return widgets::page("server").child(widgets::empty("Server tools need owner access", Some("This server is shared with you.".into()), cx)).into_any_element();
        }
        let name = s.session.as_ref().map(|x| x.server_name.clone()).unwrap_or_default();
        let selected = PANES.iter().position(|(p, _)| *p == self.pane).unwrap_or(0);
        let tabs = TabBar::new("server-tabs")
            .segmented()
            .small()
            .selected_index(selected)
            .on_click(cx.listener(|this, ix: &usize, _, cx| this.set_pane(PANES[*ix].0, cx)))
            .children(PANES.iter().map(|(_, label)| Tab::new().label(*label)));
        let head = h_flex()
            .mb_6()
            .flex_wrap()
            .items_end()
            .justify_between()
            .gap_6()
            .child(
                v_flex()
                    .min_w_0()
                    .child(div().text_3xl().font_semibold().line_height(rems(2.25)).child(name))
                    .child(div().mt_1().text_color(theme.muted_foreground).child("Manage your Plex Media Server.")),
            )
            .child(tabs);
        let error = match self.pane {
            Pane::Overview => self.overview.error.clone().filter(|_| self.overview.data.is_some()).or_else(|| self.stats.error.clone().filter(|_| self.stats.data.is_some())),
            Pane::Maintenance => self.maintenance.error.clone().filter(|_| self.maintenance.data.is_some()),
            Pane::Settings => self.prefs.error.clone().filter(|_| self.prefs.data.is_some()),
            Pane::Users => self.people.error.clone().filter(|_| self.people.data.is_some()),
            Pane::Libraries => None,
        };
        let body = match self.pane {
            Pane::Overview => match self.overview.data.clone() {
                Some(o) => self.overview_view(&o, window, cx),
                None => self.status(&self.overview, cx),
            },
            Pane::Libraries => self.libraries_view(cx),
            Pane::Maintenance => self.maintenance_view(window, cx),
            Pane::Settings => self.settings_view(window, cx),
            Pane::Users => match self.people.data.clone() {
                Some(d) => self.users_view(&d, window, cx),
                None => self.status(&self.people, cx),
            },
        };
        widgets::page("server")
            .child(head)
            .when_some(error, |d, e| d.child(div().mb_3().text_sm().text_color(theme.danger).child(format!("Could not refresh: {e}"))))
            .child(body)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    #[test]
    fn slots_fill_gaps_on_the_server_grid() {
        let b = |at, lan, wan| Bucket { at, lan, wan };
        let s = slots(&[b(7200, 1, 2), b(14_400, 3, 0)], 100, 3600, 14_500);
        assert_eq!(s.first().map(|s| s.at), Some(3600));
        assert_eq!(s.len(), 4);
        assert_eq!(s[1], Slot { at: 7200, lan: 1, wan: 2 });
        assert_eq!(s[2], Slot { at: 10_800, lan: 0, wan: 0 });
        assert_eq!(s[3].lan, 3);
    }

    #[test]
    fn scale_picks_units_and_steps() {
        let (top, ticks) = scale(3 * 1024 * 1024);
        assert_eq!(top, 3. * 1024. * 1024.);
        let labels: Vec<&str> = ticks.iter().map(|(_, l)| l.as_str()).collect();
        assert_eq!(labels, ["0", "1 MB", "2 MB", "3 MB"]);
        let (_, ticks) = scale(700);
        assert_eq!(ticks.last().map(|(_, l)| l.as_str()), Some("750 B"));
    }

    #[test]
    fn hours_and_intervals() {
        assert_eq!(hours(45_000), "13");
        assert_eq!(hours(9000), "2.5");
        assert_eq!(every(Some(1)), "Daily");
        assert_eq!(every(Some(7)), "Every 7 days");
        assert_eq!(every(None), "");
    }

    #[test]
    fn settings_filter() {
        let pref = |id: &str, advanced: bool| Pref { id: id.into(), label: format!("{id} label"), summary: None, kind: "text".into(), value: String::new(), options: vec![], advanced, group: "g".into() };
        let groups = vec![PrefGroup { id: "g".into(), label: "G".into(), prefs: vec![pref("FriendlyName", false), pref("LogVerbose", true)] }];
        assert_eq!(filter_groups(&groups, "", false)[0].prefs.len(), 1);
        assert_eq!(filter_groups(&groups, "", true)[0].prefs.len(), 2);
        assert!(filter_groups(&groups, "verbose", false).is_empty());
        assert_eq!(filter_groups(&groups, "friendly", true)[0].prefs[0].id, "FriendlyName");
    }
}
