//! Activity: what's playing on the server, watch history, and background work.

use crate::prelude::*;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::component::progress::Progress;
use gpui_kit::component::tab::{Tab, TabBar};
use patina_core::ops::lists::{self, HistoryPage};
use patina_core::ops::server as server_ops;
use patina_plex::{Metadata, TranscodeSession};

use crate::art::art_of;
use crate::core::{Load, bg, core};
use crate::format;
use crate::nav::{self, Route};
use crate::store::{self, store};
use crate::widgets;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pane {
    Playing,
    History,
    Tasks,
}

/// How a session plays, and whether the server is transcoding it.
fn decision(t: Option<&TranscodeSession>) -> (String, bool) {
    let Some(t) = t else { return ("Direct play".into(), false) };
    if t.video_decision.as_deref() == Some("transcode") {
        let hw = if t.transcode_hw_requested { " (HW)" } else { "" };
        return (format!("Transcode {} → {}{hw}", up(&t.source_video_codec), up(&t.video_codec)), true);
    }
    if t.audio_decision.as_deref() == Some("transcode") {
        return ("Audio transcode".into(), true);
    }
    ("Direct stream".into(), false)
}

fn up(s: &Option<String>) -> String {
    s.as_deref().unwrap_or("").to_uppercase()
}

/// One of the summary figures above the sessions.
fn stat(value: String, label: &str, cx: &App) -> Div {
    let theme = cx.theme();
    v_flex()
        .gap_1()
        .px_4()
        .py_4()
        .rounded(px(12.))
        .border_1()
        .border_color(theme.border)
        .bg(theme.colors.group_box)
        .child(div().text_sm().text_color(theme.muted_foreground).child(label.to_string()))
        .child(div().text_2xl().font_semibold().child(value))
}

/// A card with a bordered title row, as the server-task lists use.
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

pub struct Activity {
    pane: Pane,
    page: u32,
    sessions: Load<Vec<Metadata>>,
    sessions_stale: Option<Vec<Metadata>>,
    history: Load<Rc<HistoryPage>>,
    history_stale: Option<Rc<HistoryPage>>,
    transcodes: Load<Vec<TranscodeSession>>,
    transcodes_stale: Option<Vec<TranscodeSession>>,
    revision: u64,
    sessions_tick: u64,
    _subs: Vec<Subscription>,
    _tick: Task<()>,
}

impl Activity {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subs = vec![cx.observe(&store(cx), |this: &mut Self, store, cx| {
            let s = store.read(cx);
            let (revision, tick) = (s.revision, s.sessions_tick);
            if (this.pane == Pane::Playing && tick != this.sessions_tick) || (this.pane == Pane::History && revision != this.revision) {
                this.load(cx);
            }
            cx.notify();
        })];
        // Sessions refetch on server events; the slow tick only keeps their progress
        // moving. Transcodes have no events, so they poll.
        let tick = cx.spawn(async move |this, cx| {
            let mut secs = 0u64;
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                secs += 1;
                let alive = this.update(cx, |a, cx| match a.pane {
                    Pane::Playing if secs.is_multiple_of(15) => a.load(cx),
                    Pane::Tasks if secs.is_multiple_of(5) => a.load(cx),
                    _ => {}
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        let s = store(cx).read(cx);
        let mut this = Self {
            pane: Pane::Playing,
            page: 0,
            sessions: Load::Loading,
            sessions_stale: None,
            history: Load::Loading,
            history_stale: None,
            transcodes: Load::Loading,
            transcodes_stale: None,
            revision: s.revision,
            sessions_tick: s.sessions_tick,
            _subs: subs,
            _tick: tick,
        };
        this.load(cx);
        this
    }

    /// Fetch what the open tab shows.
    fn load(&mut self, cx: &mut Context<Self>) {
        let s = store(cx).read(cx);
        (self.revision, self.sessions_tick) = (s.revision, s.sessions_tick);
        let app = core(cx);
        match self.pane {
            Pane::Playing => {
                cx.spawn(async move |this, cx| {
                    let result = bg(async move { Ok::<_, anyhow::Error>(app.server()?.sessions().await?) }).await;
                    this.update(cx, |a, cx| {
                        a.sessions = Load::from_result(result);
                        if let Some(v) = a.sessions.ready() {
                            a.sessions_stale = Some(v.clone());
                        }
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
            Pane::History => {
                let page = self.page;
                cx.spawn(async move |this, cx| {
                    let result = bg(async move { lists::history(&app, page).await }).await.map(Rc::new);
                    this.update(cx, |a, cx| {
                        if a.page != page {
                            return;
                        }
                        a.history = Load::from_result(result);
                        if let Some(v) = a.history.ready() {
                            a.history_stale = Some(v.clone());
                        }
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
            Pane::Tasks => {
                cx.spawn(async move |this, cx| {
                    let result = bg(async move { server_ops::tasks(&app).await.map(|t| t.transcodes) }).await;
                    this.update(cx, |a, cx| {
                        a.transcodes = Load::from_result(result);
                        if let Some(v) = a.transcodes.ready() {
                            a.transcodes_stale = Some(v.clone());
                        }
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
        }
    }

    fn set_pane(&mut self, pane: Pane, cx: &mut Context<Self>) {
        if self.pane != pane {
            self.pane = pane;
            self.load(cx);
            cx.notify();
        }
    }

    fn set_page(&mut self, page: u32, cx: &mut Context<Self>) {
        self.page = page;
        self.history = Load::Loading;
        self.history_stale = None;
        self.load(cx);
        cx.notify();
    }

    /// Loading, or the error with a way to try again.
    fn status(&self, error: Option<String>, cx: &mut Context<Self>) -> AnyElement {
        match error {
            Some(e) => widgets::failed(e, cx).child(Button::new("retry").outline().small().mt_3().label("Try again").on_click(cx.listener(|this, _, _, cx| this.load(cx)))).into_any_element(),
            None => widgets::loading().into_any_element(),
        }
    }

    fn playing(&self, list: &[Metadata], window: &Window, cx: &mut Context<Self>) -> AnyElement {
        if list.is_empty() {
            return widgets::empty_box(IconName::MonitorPlay, Some("Nothing is playing".into()), Some("Streams from every Plex app on this server show up here live.".into()), cx).into_any_element();
        }
        let theme = cx.theme().clone();
        let s = store(cx).read(cx);
        let (tint, owned) = (s.tint(), s.owned());
        let muted = theme.muted_foreground;
        let transcoding = list.iter().filter(|m| m.transcode_session.as_ref().and_then(|t| t.video_decision.as_deref()) == Some("transcode")).count();
        let bandwidth: i64 = list.iter().filter_map(|m| m.session.as_ref().and_then(|s| s.bandwidth)).sum();
        let stats = div()
            .grid()
            .grid_cols(3)
            .gap_4()
            .mb_6()
            .child(stat(list.len().to_string(), "Active streams", cx))
            .child(stat(transcoding.to_string(), "Transcoding", cx))
            .child(stat(format::bitrate(bandwidth), "Total bandwidth", cx));
        let cards = list.iter().enumerate().map(|(i, m)| {
            let (title, sub) = format::playing_lines(m);
            let (label, transcodes) = decision(m.transcode_session.as_ref());
            let user = m.user.as_ref().and_then(|u| u.title.clone()).unwrap_or_else(|| "Unknown".into());
            let player = m.player.clone().unwrap_or_default();
            let session = m.session.clone().unwrap_or_default();
            let open = m.clone();
            let sid = session.id.clone();
            v_flex()
                .p_6()
                .rounded(px(12.))
                .border_1()
                .border_color(theme.border)
                .bg(theme.colors.group_box)
                .child(
                    h_flex()
                        .items_start()
                        .gap_4()
                        .child(
                            div()
                                .id(("session-poster", i))
                                .w_20()
                                .h(px(120.))
                                .flex_none()
                                .rounded(px(8.))
                                .overflow_hidden()
                                .bg(theme.muted)
                                .cursor_pointer()
                                .on_click(move |_, _, cx| nav::push(Route::for_item(&open), cx))
                                .children(art_of(m.poster(), 100., 150., None).map(|i| i.size_full().object_fit(ObjectFit::Cover))),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    h_flex()
                                        .flex_wrap()
                                        .items_center()
                                        .gap_2()
                                        .text_sm()
                                        .child(div().text_size(px(10.5)).font_semibold().text_color(tint).child(player.state.clone().unwrap_or_default().to_uppercase()))
                                        .child(div().text_color(muted).child("·"))
                                        .child(
                                            div()
                                                .size_5()
                                                .flex_none()
                                                .rounded_full()
                                                .overflow_hidden()
                                                .bg(theme.muted)
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_size(px(8.))
                                                .text_color(muted)
                                                .child(format::initials(&user))
                                                .children(art_of(m.user.as_ref().and_then(|u| u.thumb.as_deref()), 24., 24., None).map(|i| i.absolute().inset_0().size_full())),
                                        )
                                        .child(div().child(user.clone()))
                                        .child(div().text_color(muted).child(format!("on {} ({})", player.title.clone().unwrap_or_default(), player.product.clone().unwrap_or_default()))),
                                )
                                .child(div().mt_2().text_lg().font_semibold().line_height(rems(1.4)).child(title))
                                .child(div().text_sm().text_color(muted).child(sub))
                                .child(
                                    h_flex()
                                        .my_3()
                                        .flex_wrap()
                                        .gap_1p5()
                                        .child(widgets::badge(label, cx).when(transcodes, |b| b.border_color(tint.opacity(0.4)).bg(tint.opacity(0.1)).text_color(tint)))
                                        .when_some(session.location.clone().filter(|l| !l.is_empty()), |d, l| d.child(widgets::badge(l.to_uppercase(), cx)))
                                        .when_some(session.bandwidth.filter(|b| *b > 0), |d, b| d.child(widgets::badge(format::bitrate(b), cx)))
                                        .when_some(player.address.clone().filter(|a| !a.is_empty()), |d, a| d.child(widgets::badge(a, cx))),
                                )
                                .child(Progress::new(("session-progress", i)).value((m.progress().unwrap_or(0.) * 100.) as f32).color(tint))
                                .child(
                                    h_flex()
                                        .mt_2()
                                        .items_center()
                                        .justify_between()
                                        .text_xs()
                                        .child(div().text_color(muted).child(format!("{} / {}", format::clock(m.view_offset.unwrap_or(0)), format::clock(m.duration.unwrap_or(0)))))
                                        .when_some(sid.filter(|_| owned), |d, sid| {
                                            d.child(Button::new(("stop-stream", i)).danger().xsmall().label("Stop stream").on_click(move |_, _, cx| {
                                                let (app, sid) = (core(cx), sid.clone());
                                                store::act(cx, async move { Ok::<_, anyhow::Error>(app.server()?.terminate_session(&sid, "Stopped by the server owner.").await?) }, Some("Stream stopped".into()), false);
                                            }))
                                        }),
                                ),
                        ),
                )
        });
        let cols = if window.viewport_size().width >= px(1280.) { 2 } else { 1 };
        v_flex().child(stats).child(div().grid().grid_cols(cols).gap_4().children(cards)).into_any_element()
    }

    fn history_view(&self, h: &HistoryPage, cx: &mut Context<Self>) -> AnyElement {
        if h.items.is_empty() && self.page == 0 {
            return widgets::empty_box(IconName::RotateCcwClock, Some("No history yet".into()), None, cx).into_any_element();
        }
        let theme = cx.theme().clone();
        let owned = store(cx).read(cx).owned();
        let muted = theme.muted_foreground;
        let rows = widgets::rows(cx).flex_none().children(h.items.iter().enumerate().map(|(i, m)| {
            let who = m.account_id.and_then(|id| h.accounts.get(&id).cloned());
            let sub = format!("{}{}", who.map(|w| format!("{w} · ")).unwrap_or_default(), m.viewed_at.map(format::ago).unwrap_or_default());
            let key = m.history_key.clone();
            h_flex()
                .min_h_12()
                .px_3()
                .py_1p5()
                .gap_3p5()
                .when(i > 0, |d| d.border_t_1().border_color(theme.border))
                .child(div().h_12().w_8().flex_none().rounded(px(4.)).overflow_hidden().bg(theme.muted).children(art_of(m.poster(), 48., 72., None).map(|i| i.size_full().object_fit(ObjectFit::Cover))))
                .child(v_flex().flex_1().min_w_0().child(div().truncate().child(m.full_title())).child(div().truncate().text_xs().text_color(muted).child(sub)))
                .when_some(key.filter(|_| owned), |d, key| {
                    d.child(widgets::icon_button(("delete-history", i), IconName::Trash, "Delete from history").on_click(move |_, _, cx| {
                        let (app, key) = (core(cx), key.clone());
                        store::act(cx, async move { lists::delete_history(&app, &key).await }, None, true);
                    }))
                })
        }));
        let page = self.page;
        let pager = h_flex()
            .mt_5()
            .items_center()
            .justify_center()
            .gap_3p5()
            .child(Button::new("newer").ghost().small().icon(IconName::ChevronLeft).label("Newer").disabled(page == 0).on_click(cx.listener(move |this, _, _, cx| this.set_page(page.saturating_sub(1), cx))))
            .child(div().text_sm().text_color(muted).child(format!("Page {}", page + 1)))
            .child(
                Button::new("older")
                    .ghost()
                    .small()
                    .label("Older")
                    .child(Icon::new(IconName::ChevronRight))
                    .disabled(!h.has_more)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_page(page + 1, cx))),
            );
        v_flex().child(rows).child(pager).into_any_element()
    }

    fn tasks_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let s = store(cx).read(cx);
        let (tint, activities) = (s.tint(), s.activities.clone());
        let muted = theme.muted_foreground;
        let row = |first: bool| {
            h_flex()
                .px_4()
                .py_3p5()
                .gap(px(28.))
                .items_center()
                .justify_between()
                .when(!first, |d| d.border_t_1().border_color(theme.border))
        };
        let background = titled_card("Background activity", cx)
            .when(activities.is_empty(), |d| d.child(widgets::empty_box(IconName::Activity, None, Some("The server is idle.".into()), cx).border_0()))
            .children(activities.iter().enumerate().map(|(i, a)| {
                let uuid = a.uuid.clone();
                row(i == 0)
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().font_medium().child(a.title.clone()))
                            .child(div().mt_0p5().text_size(px(13.)).text_color(muted).child(a.subtitle.clone().unwrap_or_default()))
                            .child(div().mt_2p5().max_w(px(480.)).child(Progress::new(("activity", i)).value(a.progress as f32).color(tint))),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .gap_2()
                            .child(div().text_sm().text_color(muted).child(format!("{}%", a.progress)))
                            .when(a.cancellable, |d| {
                                d.child(Button::new(("cancel-activity", i)).outline().small().label("Cancel").on_click(move |_, _, cx| {
                                    let (app, uuid) = (core(cx), uuid.clone());
                                    store::act(cx, async move { Ok::<_, anyhow::Error>(app.server()?.cancel_activity(&uuid).await?) }, None, false);
                                }))
                            }),
                    )
            }));
        let error = if let Load::Failed(e) = &self.transcodes { Some(e.clone()) } else { None };
        let transcoder = titled_card("Transcoder sessions", cx).child(match self.transcodes.ready().cloned().or_else(|| self.transcodes_stale.clone()) {
            None => self.status(error, cx),
            Some(list) if list.is_empty() => widgets::empty_box(IconName::MonitorPlay, None, Some("No transcodes running.".into()), cx).border_0().into_any_element(),
            Some(list) => v_flex()
                .children(list.iter().enumerate().map(|(i, t)| {
                    let key = t.key.clone();
                    row(i == 0)
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(div().font_medium().child(format!("{} → {} · {}", up(&t.source_video_codec), up(&t.video_codec), up(&t.audio_codec))))
                                .child(div().mt_0p5().text_size(px(13.)).text_color(muted).child(format!(
                                    "{} · {} · speed {:.1}×{}",
                                    t.context.clone().unwrap_or_default(),
                                    t.protocol.clone().unwrap_or_default(),
                                    t.speed.unwrap_or(0.),
                                    if t.throttled { " (throttled)" } else { "" }
                                )))
                                .child(div().mt_2p5().max_w(px(480.)).child(Progress::new(("transcode", i)).value(t.progress.unwrap_or(0.) as f32).color(tint))),
                        )
                        .when_some(key, |d, key| {
                            d.child(Button::new(("kill-transcode", i)).danger().small().label("Stop").on_click(move |_, _, cx| {
                                let (app, key) = (core(cx), key.clone());
                                store::act(cx, async move { server_ops::kill_transcode(&app, &key).await }, None, false);
                            }))
                        })
                }))
                .into_any_element(),
        });
        v_flex().gap_6().child(background).child(transcoder).into_any_element()
    }
}

impl Render for Activity {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let selected = match self.pane {
            Pane::Playing => 0,
            Pane::History => 1,
            Pane::Tasks => 2,
        };
        let tabs = TabBar::new("activity-tabs")
            .segmented()
            .small()
            .selected_index(selected)
            .on_click(cx.listener(|this, ix: &usize, _, cx| {
                let pane = match ix {
                    0 => Pane::Playing,
                    1 => Pane::History,
                    _ => Pane::Tasks,
                };
                this.set_pane(pane, cx);
            }))
            .child(Tab::new().label("Now playing"))
            .child(Tab::new().label("History"))
            .child(Tab::new().label("Server tasks"));
        let page = widgets::page("activity").child(widgets::page_head(
            "Activity",
            Some("What's playing on the server, watch history, and background work.".into()),
            Some(tabs.into_any_element()),
            cx,
        ));
        let (error, stale, body) = match self.pane {
            Pane::Playing => {
                let error = if let Load::Failed(e) = &self.sessions { Some(e.clone()) } else { None };
                match self.sessions.ready().cloned().or_else(|| self.sessions_stale.clone()) {
                    Some(list) => (error, true, self.playing(&list, window, cx)),
                    None => (None, false, self.status(error, cx)),
                }
            }
            Pane::History => {
                let error = if let Load::Failed(e) = &self.history { Some(e.clone()) } else { None };
                match self.history.ready().cloned().or_else(|| self.history_stale.clone()) {
                    Some(h) => (error, true, self.history_view(&h, cx)),
                    None => (None, false, self.status(error, cx)),
                }
            }
            Pane::Tasks => {
                let error = if let Load::Failed(e) = &self.transcodes { Some(e.clone()) } else { None };
                (error.filter(|_| self.transcodes_stale.is_some()), true, self.tasks_view(cx))
            }
        };
        page.when_some(error.filter(|_| stale), |d, e| d.child(div().mb_3().text_sm().text_color(theme.danger).child(format!("Could not refresh: {e}"))))
            .child(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    #[test]
    fn decisions() {
        assert_eq!(decision(None), ("Direct play".to_string(), false));
        let mut t = TranscodeSession { video_decision: Some("transcode".into()), source_video_codec: Some("hevc".into()), video_codec: Some("h264".into()), transcode_hw_requested: true, ..Default::default() };
        assert_eq!(decision(Some(&t)), ("Transcode HEVC → H264 (HW)".to_string(), true));
        t.video_decision = Some("copy".into());
        t.audio_decision = Some("transcode".into());
        assert_eq!(decision(Some(&t)).0, "Audio transcode");
        t.audio_decision = None;
        assert_eq!(decision(Some(&t)), ("Direct stream".to_string(), false));
    }
}
