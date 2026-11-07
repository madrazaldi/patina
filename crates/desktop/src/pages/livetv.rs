//! Live TV from the server's DVR: what's on now and next per channel, and
//! scheduled recordings with their rules.

use crate::prelude::*;
use std::time::Duration;

use chrono::{Local, TimeZone};
use gpui_kit::component::progress::Progress;
use gpui_kit::component::tab::{Tab, TabBar};
use patina_core::livetv::{self, Channel, Program, Recordings};
use patina_core::ops;

use crate::art::art_of;
use crate::core::{Load, bg, core};
use crate::format;
use crate::store::{self, store};
use crate::widgets;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pane {
    Guide,
    Recordings,
}

/// Programs not yet over; the first is on air when it has begun.
fn upcoming(ch: &Channel, now: i64) -> Vec<&Program> {
    ch.programs.iter().filter(|p| p.ends_at > now).collect()
}

/// How far into a program `now` is, in percent.
fn elapsed(p: &Program, now: i64) -> f32 {
    let len = (p.ends_at - p.begins_at).max(1) as f32;
    ((now - p.begins_at) as f32 / len * 100.).clamp(0., 100.)
}

fn time(s: i64) -> String {
    Local.timestamp_opt(s, 0).single().map(|d| d.format("%-I:%M %p").to_string()).unwrap_or_default()
}

fn range(p: &Program) -> String {
    format!("{} – {}", time(p.begins_at), time(p.ends_at))
}

fn when(s: i64) -> String {
    Local.timestamp_opt(s, 0).single().map(|d| d.format("%a, %b %-d, %-I:%M %p").to_string()).unwrap_or_default()
}

pub struct LiveTv {
    tab: Pane,
    guide: Load<Vec<Channel>>,
    guide_stale: Option<Vec<Channel>>,
    recordings: Load<Recordings>,
    recordings_stale: Option<Recordings>,
    /// Unix seconds, advanced every minute so finished programs drop off.
    now: i64,
    /// Channel whose later listings are showing.
    open: Option<String>,
    revision: u64,
    _subs: Vec<Subscription>,
    _tick: Task<()>,
}

impl LiveTv {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subs = vec![cx.observe(&store(cx), |this: &mut Self, store, cx| {
            if store.read(cx).revision != this.revision && this.tab == Pane::Recordings {
                this.load_recordings(cx);
            }
        })];
        // Every minute the clock moves on; every 15 minutes the guide is fetched again.
        let tick = cx.spawn(async move |this, cx| {
            let mut minutes = 0u32;
            loop {
                cx.background_executor().timer(Duration::from_secs(60)).await;
                minutes += 1;
                let alive = this.update(cx, |l, cx| {
                    l.now = format::now();
                    if minutes.is_multiple_of(15) && l.tab == Pane::Guide {
                        l.load_guide(cx);
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        let mut this = Self {
            tab: Pane::Guide,
            guide: Load::Loading,
            guide_stale: None,
            recordings: Load::Loading,
            recordings_stale: None,
            now: format::now(),
            open: None,
            revision: store(cx).read(cx).revision,
            _subs: subs,
            _tick: tick,
        };
        this.load_guide(cx);
        this
    }

    fn load_guide(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.guide, Load::Ready(_)) {
            self.guide = Load::Loading;
        }
        let app = core(cx);
        cx.spawn(async move |this, cx| {
            let result = bg(async move { livetv::guide(&app.server()?, 6).await }).await;
            this.update(cx, |l, cx| {
                l.guide = Load::from_result(result);
                if let Some(g) = l.guide.ready() {
                    l.guide_stale = Some(g.clone());
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn load_recordings(&mut self, cx: &mut Context<Self>) {
        self.revision = store(cx).read(cx).revision;
        let app = core(cx);
        cx.spawn(async move |this, cx| {
            let result = bg(async move { livetv::recordings(&app.server()?).await }).await;
            this.update(cx, |l, cx| {
                l.recordings = Load::from_result(result);
                if let Some(r) = l.recordings.ready() {
                    l.recordings_stale = Some(r.clone());
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn set_tab(&mut self, tab: Pane, cx: &mut Context<Self>) {
        if self.tab == tab {
            return;
        }
        self.tab = tab;
        match tab {
            Pane::Guide => self.load_guide(cx),
            Pane::Recordings => self.load_recordings(cx),
        }
        cx.notify();
    }

    fn record(guid: String, cx: &mut App) {
        let app = core(cx);
        store::act(cx, async move { livetv::record(&app.server()?, &guid).await }, Some("Recording scheduled".into()), false);
    }

    fn record_button(id: ElementId, p: &Program) -> Option<AnyElement> {
        let guid = p.guid.clone()?;
        Some(
            div()
                .opacity(0.)
                .group_hover("channel", |s| s.opacity(1.))
                .child(widgets::icon_button(id, IconName::CircleDot, "Record").xsmall().on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    Self::record(guid.clone(), cx);
                }))
                .into_any_element(),
        )
    }

    fn guide_view(&self, channels: &[Channel], cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let tint = store(cx).read(cx).tint();
        let muted = theme.muted_foreground;
        if channels.is_empty() {
            return widgets::empty_box(IconName::Radio, Some("No channels".into()), Some("Set up a tuner and scan for channels in Plex's Live TV & DVR settings.".into()), cx).into_any_element();
        }
        let now = self.now;
        widgets::rows(cx)
            .flex_none()
            .children(channels.iter().enumerate().map(|(n, ch)| {
                let key = format!("{}{}", ch.dvr, ch.id);
                let list = upcoming(ch, now);
                let cur = list.first().copied().filter(|p| p.begins_at <= now);
                let next = if cur.is_some() { list.get(1).copied() } else { list.first().copied() };
                let later: Vec<&Program> = list.iter().skip(if cur.is_some() { 2 } else { 1 }).copied().collect();
                let expanded = self.open.as_deref() == Some(key.as_str());
                let watch = ch.clone();
                let k = key.clone();
                let line = h_flex()
                    .id(("channel", n))
                    .group("channel")
                    .min_h_16()
                    .px_4()
                    .py_2p5()
                    .gap_5()
                    .items_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.muted.opacity(0.4)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open = if this.open.as_deref() == Some(k.as_str()) { None } else { Some(k.clone()) };
                        cx.notify();
                    }))
                    .child(
                        h_flex()
                            .w(px(200.))
                            .flex_none()
                            .min_w_0()
                            .items_center()
                            .gap_2p5()
                            .font_medium()
                            .child(div().w(px(34.)).flex_none().text_right().text_xs().font_normal().text_color(muted).child(ch.number.clone().unwrap_or_default()))
                            .child(div().size_8().flex_none().rounded(px(6.)).overflow_hidden().bg(theme.muted).children(art_of(ch.thumb.as_deref(), 40., 40., None).map(|i| i.size_full().object_fit(ObjectFit::Contain))))
                            .child(div().min_w_0().truncate().child(ch.title.clone())),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .map(|mut d| {
                                d.style().flex_grow = Some(1.4);
                                d
                            })
                            .flex_basis(relative(0.))
                            .min_w_0()
                            .map(|d| match cur {
                                Some(p) => d
                                    .child(h_flex().min_h_6().gap_1p5().child(div().min_w_0().truncate().font_medium().child(p.title.clone())).children(Self::record_button(("rec-cur", n).into(), p)))
                                    .child(
                                        div()
                                            .truncate()
                                            .text_xs()
                                            .text_color(muted)
                                            .child(match &p.subtitle {
                                                Some(s) => format!("{} · {s}", range(p)),
                                                None => range(p),
                                            }),
                                    )
                                    .child(div().mt_2().max_w(px(280.)).child(Progress::new(("on-air", n)).value(elapsed(p, now)).color(tint))),
                                None => d.child(div().text_color(muted).child(if list.is_empty() { "No guide data" } else { "Off air" })),
                            }),
                    )
                    .child(
                        v_flex().flex_1().flex_basis(relative(0.)).min_w_0().text_color(muted).when_some(next, |d, p| {
                            d.child(
                                h_flex()
                                    .min_h_6()
                                    .gap_1p5()
                                    .child(div().flex_none().text_size(px(11.)).child(time(p.begins_at)))
                                    .child(div().min_w_0().truncate().child(p.title.clone()))
                                    .children(Self::record_button(("rec-next", n).into(), p)),
                            )
                            .when_some(p.subtitle.clone(), |d, s| d.child(div().truncate().text_xs().child(s)))
                        }),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .items_center()
                            .gap_2p5()
                            .child(Button::new(("watch", n)).outline().small().icon(IconName::Play).label("Watch").on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                store::toast(cx, "Tuning…");
                                let (app, ch) = (core(cx), watch.clone());
                                store::act(cx, async move { ops::playback::livetv_watch(&app, &ch.dvr, &ch.id, &ch.title).await }, None, false);
                            }))
                            .child(Icon::new(if expanded { IconName::ChevronUp } else { IconName::ChevronDown }).size(px(14.)).text_color(muted)),
                    );
                let listings = expanded.then(|| {
                    h_flex()
                        .id(("later", n))
                        .gap_px()
                        .overflow_x_scroll()
                        .pr_4()
                        .pb_3p5()
                        .pl(px(224.))
                        .when(later.is_empty(), |d| d.child(div().text_xs().text_color(muted).child("No later listings in the next six hours.")))
                        .children(later.iter().enumerate().map(|(i, p)| {
                            let guid = p.guid.clone();
                            v_flex()
                                .w_48()
                                .flex_none()
                                .gap_0p5()
                                .border_l_1()
                                .border_color(theme.border)
                                .px_3()
                                .py_2()
                                .child(div().text_size(px(11.)).text_color(muted).child(range(p)))
                                .child(div().truncate().child(p.title.clone()))
                                .child(div().truncate().text_xs().text_color(muted).child(p.subtitle.clone().unwrap_or_default()))
                                .when_some(guid, |d, guid| {
                                    d.child(div().mt_1().child(Button::new(("record-later", n * 1000 + i)).outline().xsmall().label("Record").on_click(move |_, _, cx| Self::record(guid.clone(), cx))))
                                })
                        }))
                });
                v_flex().when(n > 0, |d| d.border_t_1().border_color(theme.border)).child(line).children(listings)
            }))
            .into_any_element()
    }

    fn recordings_view(&self, r: &Recordings, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let upcoming = if r.upcoming.is_empty() {
            widgets::empty_box(IconName::CalendarClock, None, Some("Nothing scheduled. Record a program from the guide.".into()), cx).into_any_element()
        } else {
            widgets::rows(cx)
                .flex_none()
                .children(r.upcoming.iter().enumerate().map(|(i, u)| {
                    let sub = [u.subtitle.clone(), u.channel.clone()].into_iter().flatten().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
                    h_flex()
                        .min_h_12()
                        .px_3()
                        .py_1p5()
                        .gap_3p5()
                        .when(i > 0, |d| d.border_t_1().border_color(theme.border))
                        .child(div().w(px(170.)).flex_none().text_sm().text_color(muted).child(u.begins_at.map(when).unwrap_or_default()))
                        .child(v_flex().flex_1().min_w_0().child(div().truncate().child(u.title.clone())).child(div().truncate().text_xs().text_color(muted).child(sub)))
                        .when_some(u.status.clone(), |d, s| d.child(widgets::badge(s, cx)))
                })
                .collect::<Vec<_>>())
                .into_any_element()
        };
        let rules = if r.rules.is_empty() {
            widgets::empty_box(IconName::Repeat, None, Some("No recording rules.".into()), cx).into_any_element()
        } else {
            widgets::rows(cx)
                .flex_none()
                .children(r.rules.iter().enumerate().map(|(i, rule)| {
                    let (id, title) = (rule.id.clone(), rule.title.clone());
                    h_flex()
                        .min_h_12()
                        .px_3()
                        .py_1p5()
                        .gap_3p5()
                        .when(i > 0, |d| d.border_t_1().border_color(theme.border))
                        .child(v_flex().flex_1().min_w_0().child(div().truncate().child(rule.title.clone())).when_some(rule.kind.clone(), |d, k| d.child(div().truncate().text_xs().text_color(muted).child(k))))
                        .child(widgets::icon_button(("delete-rule", i), IconName::Trash, "Delete rule").on_click(cx.listener(move |_, _, window, cx| {
                            let (id, view) = (id.clone(), cx.entity().downgrade());
                            widgets::confirm(
                                "Delete recording rule?",
                                Some(format!("{title} will no longer be recorded. Existing recordings are kept.").into()),
                                "Delete",
                                true,
                                window,
                                cx,
                                move |_, cx| {
                                    let (app, id, view) = (core(cx), id.clone(), view.clone());
                                    store::act_then(cx, async move { livetv::delete_rule(&app.server()?, &id).await }, move |_, cx| {
                                        store::toast(cx, "Recording rule deleted");
                                        view.update(cx, |l, cx| l.load_recordings(cx)).ok();
                                    });
                                },
                            );
                        })))
                })
                .collect::<Vec<_>>())
                .into_any_element()
        };
        v_flex()
            .child(widgets::section_head("Upcoming", None).mt_0())
            .child(upcoming)
            .child(widgets::section_head("Rules", None))
            .child(rules)
            .into_any_element()
    }
}

impl Render for LiveTv {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let tabs = TabBar::new("livetv-tabs")
            .segmented()
            .small()
            .selected_index(if self.tab == Pane::Guide { 0 } else { 1 })
            .on_click(cx.listener(|this, ix: &usize, _, cx| this.set_tab(if *ix == 0 { Pane::Guide } else { Pane::Recordings }, cx)))
            .child(Tab::new().label("Guide"))
            .child(Tab::new().label("Recordings"));
        let mut page = widgets::page("livetv").child(widgets::page_head("Live TV", Some("Watch and record from your server's tuner.".into()), Some(tabs.into_any_element()), cx));
        let (error, body) = match self.tab {
            Pane::Guide => {
                let error = if let Load::Failed(e) = &self.guide { Some(e.clone()) } else { None };
                let data = self.guide.ready().cloned().or_else(|| self.guide_stale.clone());
                (error.clone(), match data {
                    Some(channels) => self.guide_view(&channels, cx),
                    None => self.status(error, cx),
                })
            }
            Pane::Recordings => {
                let error = if let Load::Failed(e) = &self.recordings { Some(e.clone()) } else { None };
                let data = self.recordings.ready().cloned().or_else(|| self.recordings_stale.clone());
                (error.clone(), match data {
                    Some(r) => self.recordings_view(&r, cx),
                    None => self.status(error, cx),
                })
            }
        };
        let stale = error.is_some() && match self.tab {
            Pane::Guide => self.guide_stale.is_some(),
            Pane::Recordings => self.recordings_stale.is_some(),
        };
        if let (true, Some(e)) = (stale, error) {
            page = page.child(div().mb_3().text_sm().text_color(theme.danger).child(format!("Could not refresh: {e}")));
        }
        page.child(body).child(div().mt_7().text_xs().text_color(theme.muted_foreground).child("Live TV needs a tuner connected to a Plex Pass server."))
    }
}

impl LiveTv {
    /// Loading, or the error with a way to try again.
    fn status(&self, error: Option<String>, cx: &mut Context<Self>) -> AnyElement {
        match error {
            Some(e) => widgets::failed(e, cx)
                .child(Button::new("retry").outline().small().mt_3().label("Try again").on_click(cx.listener(|this, _, _, cx| match this.tab {
                    Pane::Guide => this.load_guide(cx),
                    Pane::Recordings => this.load_recordings(cx),
                })))
                .into_any_element(),
            None => widgets::loading().into_any_element(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    fn program(b: i64, e: i64) -> Program {
        Program { title: "x".into(), subtitle: None, guid: None, summary: None, thumb: None, begins_at: b, ends_at: e }
    }

    #[test]
    fn guide_drops_finished_programs() {
        let ch = Channel { dvr: "d".into(), id: "1".into(), number: None, title: "One".into(), thumb: None, programs: vec![program(0, 100), program(100, 200), program(200, 300)] };
        let list = upcoming(&ch, 150);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].begins_at, 100);
        assert_eq!(elapsed(list[0], 150), 50.);
        assert_eq!(elapsed(list[1], 150), 0.);
    }
}
