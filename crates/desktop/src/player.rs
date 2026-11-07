//! Playback chrome: the now-playing bar with its queue, lyrics and player
//! panels, and the playback-failure and resume banners.

use crate::prelude::*;
use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use patina_core::controls::{PlaybackState, Repeat, Tune};
use patina_core::failure::{self, Recover};
use patina_core::lyrics::Lyrics;
use patina_core::ops;
use patina_core::player::{self, Control, QueueCommand};
use patina_core::state::{NowPlaying, QueueView};
use patina_plex::{Media, Metadata};

use crate::art::{ArtKey, art, art_of};
use crate::core::{Load, bg, blocking, core, message};
use crate::format;
use crate::nav::{self, Route};
use crate::store::{Panel, store};
use crate::theme;

/// Send a remote-control command to the running player.
fn send(c: Control) {
    let _guard = patina_core::runtime().enter();
    player::control(c);
}

/// Edit the running queue.
fn edit(c: QueueCommand, cx: &App) {
    let app = core(cx);
    let _guard = patina_core::runtime().enter();
    player::queue(app, c);
}

/// Change volume, speed, repeat, chapter or the sleep timer.
fn tune(c: Tune, cx: &App) {
    let app = core(cx);
    let _guard = patina_core::runtime().enter();
    player::tune(&app, c);
}

/// Open the panel `p`, or close it when it is open.
fn toggle(p: Panel, cx: &mut App) {
    store(cx).update(cx, |s, cx| {
        s.panel = if s.panel == Some(p) { None } else { Some(p) };
        cx.notify();
    });
}

fn close_panel(cx: &mut App) {
    store(cx).update(cx, |s, cx| {
        if s.panel.is_some() {
            s.panel = None;
            cx.notify();
        }
    });
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// Volume, speed, repeat, sleep timer and buffering, when they differ from the defaults.
fn chips(ps: &PlaybackState, now_ms: i64) -> Vec<String> {
    let mut out = Vec::new();
    let d = &ps.diagnostics;
    if d.buffering {
        out.push(d.buffer_percent.map_or("Buffering".to_string(), |p| format!("Buffering {p}%")));
    }
    if ps.muted {
        out.push("Muted".into());
    } else if (ps.volume - 100.).abs() > 0.5 {
        out.push(format!("Vol {}%", ps.volume.round()));
    }
    if (ps.speed - 1.).abs() > 0.001 {
        out.push(format!("{}x", ps.speed));
    }
    match ps.repeat {
        Repeat::Off => {}
        Repeat::One => out.push("Repeat one".into()),
        Repeat::All => out.push("Repeat all".into()),
    }
    if let Some(end) = ps.sleep.as_ref().and_then(|s| s.ends_at_ms) {
        let mins = ((end - now_ms) as f64 / 60_000.).ceil().max(1.);
        out.push(format!("Sleep {mins}m"));
    }
    out
}

/// "1080p H264" for a version of an item.
fn media_label(m: &Media, i: usize) -> String {
    let res = match (&m.video_resolution, m.height) {
        (Some(r), _) if !r.is_empty() && r.chars().all(|c| c.is_ascii_digit()) => format!("{r}p"),
        (Some(r), _) if !r.is_empty() => r.to_uppercase(),
        (_, Some(h)) => format!("{h}p"),
        _ => format!("version {}", i + 1),
    };
    match &m.video_codec {
        Some(c) if !c.is_empty() => format!("{res} {}", c.to_uppercase()),
        _ => res,
    }
}

/// An outlined badge.
fn badge(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .flex_none()
        .px_2()
        .py_0p5()
        .rounded(px(6.))
        .border_1()
        .border_color(cx.theme().border)
        .text_xs()
        .font_medium()
        .text_color(cx.theme().foreground)
        .child(text.into())
}

/// Small uppercase heading used in the panels.
fn heading(text: &str, cx: &App) -> Div {
    div().text_size(px(10.5)).font_semibold().text_color(cx.theme().muted_foreground).child(text.to_uppercase())
}

/// A floating surface anchored to the bar's top-right, styled like a popover.
fn surface(cx: &App) -> Div {
    v_flex()
        .absolute()
        .right_3()
        .bottom(px(68.))
        .bg(cx.theme().popover)
        .text_color(cx.theme().popover_foreground)
        .rounded(px(12.))
        .border_1()
        .border_color(hsla(0., 0., 1., 0.1))
        .shadow_lg()
        .overflow_hidden()
}

// The bar ------------------------------------------------------------------------

struct Views {
    bar: Entity<Bar>,
}

impl Global for Views {}

/// The bar at the bottom of the window while something plays.
pub fn now_playing_bar(window: &mut Window, cx: &mut App) -> AnyElement {
    if store(cx).read(cx).now_playing.is_none() {
        return div().into_any_element();
    }
    let bar = match cx.try_global::<Views>() {
        Some(v) => v.bar.clone(),
        None => {
            let bar = cx.new(|cx| Bar::new(window, cx));
            cx.set_global(Views { bar: bar.clone() });
            bar
        }
    };
    bar.into_any_element()
}

struct Bar {
    /// The fraction under the pointer while scrubbing, held after release until playback catches up.
    scrub: Option<f32>,
    scrubbing: bool,
    /// Bumped with each seek so an older timer doesn't drop a newer target.
    seek_gen: u64,
    seek_bounds: Rc<Cell<Bounds<Pixels>>>,
    bar_bounds: Rc<Cell<Bounds<Pixels>>>,
    focus: FocusHandle,
    queue: Entity<QueuePanel>,
    lyrics: Entity<LyricsPanel>,
    player: Entity<PlayerPanel>,
    _subs: Vec<Subscription>,
}

impl Bar {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subs = vec![cx.observe(&store(cx), |this: &mut Self, store, cx| {
            // Drop the held scrub target once playback has caught up with it.
            if let (Some(f), false) = (this.scrub, this.scrubbing)
                && let Some(np) = store.read(cx).now_playing.as_ref()
                && (np.position_ms - (f as f64 * np.duration_ms as f64) as i64).abs() < 2500
            {
                this.scrub = None;
            }
            cx.notify();
        })];
        Self {
            scrub: None,
            scrubbing: false,
            seek_gen: 0,
            seek_bounds: Rc::new(Cell::new(Bounds::default())),
            bar_bounds: Rc::new(Cell::new(Bounds::default())),
            focus: cx.focus_handle(),
            queue: cx.new(|_| QueuePanel::new()),
            lyrics: cx.new(LyricsPanel::new),
            player: cx.new(|cx| PlayerPanel::new(window, cx)),
            _subs: subs,
        }
    }

    fn fraction(&self, x: Pixels) -> f32 {
        let b = self.seek_bounds.get();
        if b.size.width <= px(0.) {
            return 0.;
        }
        ((x - b.left()) / b.size.width).clamp(0., 1.)
    }

    /// Seek to `ms`, holding the target on the line for a moment so it doesn't jump back.
    fn seek_to(&mut self, ms: i64, duration: i64, cx: &mut Context<Self>) {
        if duration > 0 {
            self.scrub = Some(ms as f32 / duration as f32);
        }
        send(Control::SeekTo(ms as f64 / 1000.));
        self.seek_gen += 1;
        let generation = self.seek_gen;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            this.update(cx, |b, cx| {
                if b.seek_gen == generation && !b.scrubbing {
                    b.scrub = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn release(&mut self, cx: &mut Context<Self>) {
        if !self.scrubbing {
            return;
        }
        self.scrubbing = false;
        let Some(np) = store(cx).read(cx).now_playing.clone() else { return };
        if let Some(f) = self.scrub {
            self.seek_to((f as f64 * np.duration_ms as f64) as i64, np.duration_ms, cx);
        }
    }

    /// Arrows seek 5 s, PageUp/PageDown 60 s, Home/End jump to the ends; repeated presses build on the pending target.
    fn seek_key(&mut self, e: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(np) = store(cx).read(cx).now_playing.clone() else { return };
        let m = &e.keystroke.modifiers;
        if np.duration_ms <= 0 || m.alt || m.control || m.platform {
            return;
        }
        let now = self.scrub.map_or(np.position_ms, |f| (f as f64 * np.duration_ms as f64) as i64);
        let to = match e.keystroke.key.as_str() {
            "left" | "down" => now - 5000,
            "right" | "up" => now + 5000,
            "pagedown" => now - 60_000,
            "pageup" => now + 60_000,
            "home" => 0,
            // Stop just short of the end so End never skips to the next item by itself.
            "end" => np.duration_ms - 1000,
            _ => return,
        };
        cx.stop_propagation();
        self.seek_to(to.clamp(0, (np.duration_ms - 1000).max(0)), np.duration_ms, cx);
    }

    fn seek_line(&self, np: &NowPlaying, position: i64, tint: Hsla, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let pct = if np.duration_ms > 0 { (position as f32 / np.duration_ms as f32).clamp(0., 1.) } else { 0. };
        let scrubbing = self.scrub.is_some();
        let focused = self.focus.is_focused(window);
        let line2 = cx.theme().input;
        let (seek_bounds, weak) = (self.seek_bounds.clone(), cx.entity().downgrade());
        div()
            .id("seek")
            .group("seek")
            .absolute()
            .top(px(-5.))
            .left_0()
            .right_0()
            .h(px(9.))
            .cursor_pointer()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::seek_key))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus, cx);
                    this.scrubbing = true;
                    this.scrub = Some(this.fraction(e.position.x));
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(if focused { 3. } else { 4. }))
                    .h(px(if focused { 3. } else { 1. }))
                    .when(scrubbing || focused, |d| d.bg(line2))
                    .group_hover("seek", |s| s.bg(line2))
                    .child(div().h_full().w(relative(pct)).bg(tint)),
            )
            .child(
                canvas(
                    move |bounds, _, _| seek_bounds.set(bounds),
                    move |_, _, window, _| {
                        // While scrubbing, follow the pointer anywhere in the window.
                        let w = weak.clone();
                        window.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
                            if phase == DispatchPhase::Bubble {
                                w.update(cx, |b, cx| {
                                    if b.scrubbing {
                                        b.scrub = Some(b.fraction(e.position.x));
                                        cx.notify();
                                    }
                                })
                                .ok();
                            }
                        });
                        let w = weak.clone();
                        window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                            if phase == DispatchPhase::Bubble {
                                w.update(cx, |b, cx| b.release(cx)).ok();
                            }
                        });
                    },
                )
                .absolute()
                .inset_0(),
            )
            .into_any_element()
    }

    fn up_next(&self, next: &Metadata, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let (title, sub) = format::playing_lines(next);
        let bar = self.bar_bounds.clone();
        surface(cx)
            .id("up-next")
            .occlude()
            .w(px(440.))
            .flex_row()
            .items_center()
            .gap_3()
            .py_2p5()
            .pr_2p5()
            .pl_3()
            .on_mouse_down_out(move |e, _, cx| {
                if !bar.get().contains(&e.position) {
                    close_panel(cx);
                }
            })
            .child(
                div()
                    .size_8()
                    .flex_none()
                    .rounded(px(4.))
                    .overflow_hidden()
                    .bg(theme.muted)
                    .children(art_of(next.poster().or(next.thumb.as_deref()), 32., 32., None).map(|i| i.size_full().object_fit(ObjectFit::Cover))),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(heading("Up next", cx))
                    .child(
                        div()
                            .text_sm()
                            .truncate()
                            .child(title)
                            .when(!sub.is_empty(), |d| d.child(div().text_color(theme.muted_foreground).child(format!(" · {sub}")))),
                    ),
            )
            .child(Button::new("play-next").outline().small().label("Play now").on_click(|_, _, _| send(Control::Next)))
            .child(Button::new("stop-after").ghost().small().label("Stop after this").on_click(|_, _, cx| edit(QueueCommand::StopAfter { on: true }, cx)))
            .into_any_element()
    }
}

impl Render for Bar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let s = store(cx).read(cx);
        let Some(np) = s.now_playing.clone() else { return div().into_any_element() };
        let queue = s.queue.clone();
        let playback = s.playback.clone();
        let tint = s.tint();
        let signed_in = s.session.is_some();
        let is_track = np.item.kind() == "track";
        let panel = match s.panel {
            Some(Panel::Queue) if queue.is_some() => Some(Panel::Queue),
            Some(Panel::Lyrics) if is_track => Some(Panel::Lyrics),
            Some(Panel::Player) if playback.is_some() => Some(Panel::Player),
            _ => None,
        };
        // Offline there is no server to ask for artwork; use what the download saved.
        let cover = if np.local {
            s.downloads.iter().find(|d| d.item.rating_key == np.item.rating_key && d.poster.is_some()).map(|d| art(ArtKey::download(d.id.clone())))
        } else {
            let src = if np.item.kind() == "episode" { np.item.thumb.as_deref() } else { np.item.poster() };
            art_of(src, 60., 60., None)
        };
        let next = queue.as_ref().and_then(|q| {
            let i = q.items.iter().position(|e| e.uid == np.uid)?;
            q.items.get(i + 1).map(|e| e.item.clone())
        });
        if panel != Some(Panel::Queue) {
            self.queue.update(cx, |q, _| q.placed = false);
        }
        if panel != Some(Panel::Player) {
            self.player.update(cx, |p, _| p.placed = false);
        }

        let (title, sub) = format::playing_lines(&np.item);
        let position = self.scrub.map_or(np.position_ms, |f| (f as f64 * np.duration_ms as f64) as i64);
        let status = if np.live {
            "Live"
        } else if np.local {
            "Offline"
        } else if np.transcoding {
            "Transcode"
        } else {
            "Direct play"
        };
        let item = np.item.clone();
        let muted = theme.muted_foreground;
        let ghost = |id: &'static str, icon: IconName, tip: &'static str| Button::new(id).ghost().icon(icon).tooltip(tip).text_color(muted);
        let toggle_button = |id: &'static str, icon: IconName, tip: &'static str, p: Panel| {
            Button::new(id).ghost().icon(icon).tooltip(tip).text_color(if panel == Some(p) { tint } else { muted }).on_click(move |_, _, cx| toggle(p, cx))
        };

        let left = h_flex()
            .id("np-item")
            .flex_1()
            .flex_basis(relative(0.))
            .min_w_0()
            .gap_3()
            .cursor_pointer()
            .on_click(move |_, _, cx| {
                if signed_in {
                    nav::push(Route::for_item(&item), cx);
                }
            })
            .child(
                div()
                    .size_10()
                    .flex_none()
                    .rounded(px(6.))
                    .overflow_hidden()
                    .bg(theme.muted)
                    .border_1()
                    .border_color(hsla(0., 0., 1., 0.1))
                    .children(cover.map(|i| i.size_full().object_fit(ObjectFit::Cover))),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_sm().font_medium().truncate().child(title))
                    .child(div().text_xs().text_color(muted).truncate().child(sub)),
            );

        let center = h_flex()
            .flex_none()
            .items_center()
            .gap_1()
            .child(ghost("np-prev", IconName::SkipBack, "Previous").disabled(np.queue_pos == 0).on_click(|_, _, _| send(Control::Previous)))
            .child(ghost("np-back", IconName::RotateCcw, "Back 10 seconds").on_click(|_, _, _| send(Control::SeekRelative(-10.))))
            .child(
                Button::new("np-play")
                    .primary()
                    .large()
                    .mx_1()
                    .rounded(px(999.))
                    .icon(if np.paused { IconName::Play } else { IconName::Pause })
                    .tooltip(if np.paused { "Play" } else { "Pause" })
                    .on_click(|_, _, _| send(Control::TogglePause)),
            )
            .child(ghost("np-stop", IconName::Square, "Stop").on_click(|_, _, _| send(Control::Stop)))
            .child(ghost("np-next", IconName::SkipForward, "Next").disabled(np.queue_pos + 1 >= np.queue_len).on_click(|_, _, _| send(Control::Next)));

        let chips = playback.as_ref().map(|ps| chips(ps, now_ms())).unwrap_or_default();
        let live_badge = badge(status, cx).when(np.live, |d| d.border_color(tint.opacity(0.4)).bg(tint.opacity(0.1)).text_color(tint));
        let right = h_flex()
            .flex_1()
            .flex_basis(relative(0.))
            .map(|mut d| {
                d.style().flex_grow = Some(1.5);
                d
            })
            .min_w_0()
            .overflow_hidden()
            .justify_end()
            .items_center()
            .gap_2p5()
            .text_xs()
            .text_color(muted)
            .when(np.stop_after, |d| {
                d.child(
                    h_flex()
                        .flex_none()
                        .gap_2()
                        .child(badge("Stops after this", cx))
                        .child(div().id("undo-stop").cursor_pointer().underline().text_color(theme.foreground).child("Undo").on_click(|_, _, cx| edit(QueueCommand::StopAfter { on: false }, cx))),
                )
            })
            .children(chips.into_iter().map(|c| badge(c, cx)))
            .child(live_badge)
            .when(np.queue_len > 1, |d| d.child(div().flex_none().child(format!("{} of {}", np.queue_pos + 1, np.queue_len))))
            .child(div().flex_none().child(if np.live { format::clock(position) } else { format!("{} / {}", format::clock(position), format::clock(np.duration_ms)) }))
            .when(queue.is_some() || is_track || playback.is_some(), |d| {
                d.child(
                    h_flex()
                        .flex_none()
                        .ml_1()
                        .mr(px(-6.))
                        .gap_0p5()
                        .when(playback.is_some(), |d| d.child(toggle_button("np-player", IconName::SlidersHorizontal, "Player controls and diagnostics", Panel::Player)))
                        .when(is_track, |d| d.child(toggle_button("np-lyrics", IconName::MicVocal, "Lyrics", Panel::Lyrics)))
                        .when(queue.is_some(), |d| d.child(toggle_button("np-queue", IconName::ListMusic, "Queue", Panel::Queue))),
                )
            });

        let bar_bounds = self.bar_bounds.clone();
        let max_h = window.viewport_size().height * 0.6;
        let floating = match panel {
            Some(p) => {
                let bar = self.bar_bounds.clone();
                let content: AnyElement = match p {
                    Panel::Queue => self.queue.clone().into_any_element(),
                    Panel::Lyrics => self.lyrics.clone().into_any_element(),
                    Panel::Player => self.player.clone().into_any_element(),
                };
                Some(
                    surface(cx)
                        .id("player-panel")
                        .occlude()
                        .w(px(380.))
                        .max_h(max_h)
                        .when(p == Panel::Lyrics, |d| d.h(max_h))
                        .on_mouse_down_out(move |e, _, cx| {
                            if !bar.get().contains(&e.position) {
                                close_panel(cx);
                            }
                        })
                        .child(content)
                        .into_any_element(),
                )
            }
            None => match &next {
                Some(next) if np.ending && !np.stop_after => Some(self.up_next(next, cx)),
                _ => None,
            },
        };

        div()
            .id("now-playing")
            .relative()
            .flex_none()
            .h(px(60.))
            .px_4()
            .border_t_1()
            .border_color(theme.border)
            .bg(theme.background)
            .child(canvas(move |bounds, _, _| bar_bounds.set(bounds), |_, _, _, _| {}).absolute().inset_0())
            .child(h_flex().size_full().items_center().gap_6().child(left).child(center).child(right))
            .when(!np.live && np.duration_ms > 0, |d| d.child(self.seek_line(&np, position, tint, window, cx)))
            .children(floating)
            .into_any_element()
    }
}

// Banners ------------------------------------------------------------------------

/// Why playback failed, with ways to recover.
pub fn failure_banner(_window: &mut Window, cx: &mut App) -> AnyElement {
    let Some(f) = store(cx).read(cx).failure.clone() else { return div().into_any_element() };
    let theme = cx.theme().clone();
    let (title, _) = format::playing_lines(&f.item);
    let recover = |action: Recover| {
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
            let app = core(cx);
            let _guard = patina_core::runtime().enter();
            if let Err(e) = failure::recover(&app, action) {
                crate::store::error(cx, message(&e));
            }
        }
    };
    let outline = |id: ElementId, label: String| Button::new(id).outline().small().label(label).text_color(theme.foreground);
    let mut actions = h_flex().mt_2p5().flex_wrap().items_center().gap_1p5();
    if f.can_retry {
        let label = if f.position_ms > 0 { format!("Try again from {}", format::clock(f.position_ms)) } else { "Try again".into() };
        actions = actions.child(outline("fail-retry".into(), label).on_click(recover(Recover::Retry)));
    }
    for (i, m) in f.item.media.iter().enumerate() {
        if i != f.media_index {
            actions = actions.child(outline(("fail-version", i).into(), format!("Try {}", media_label(m, i))).on_click(recover(Recover::Version { index: i })));
        }
    }
    if let Some(q) = &f.lower_quality {
        actions = actions.child(outline("fail-lower".into(), format!("Try {}", q.label)).on_click(recover(Recover::LowerQuality)));
    }
    div()
        .flex_none()
        .px_4()
        .pt_3()
        .child(
            h_flex()
                .items_start()
                .gap_3()
                .py_3()
                .pl_4()
                .pr_2p5()
                .rounded(px(10.))
                .border_1()
                .border_color(theme.border)
                .bg(theme.colors.group_box)
                .text_sm()
                .text_color(theme.danger)
                .child(Icon::new(IconName::TriangleAlert).size_4().mt_0p5().flex_none())
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(div().truncate().font_medium().child(format!("Couldn't play {title}.")))
                        .child(div().truncate().text_color(theme.danger.opacity(0.9)).child(f.reason.clone()))
                        .when_some(f.detail.clone(), |d, detail| d.child(div().mt_0p5().truncate().text_xs().text_color(theme.muted_foreground).child(detail)))
                        .child(actions),
                )
                .child(
                    Button::new("fail-dismiss").ghost().small().icon(IconName::X).tooltip("Dismiss").flex_none().on_click(|_, _, cx| {
                        let app = core(cx);
                        failure::clear(&app);
                    }),
                ),
        )
        .into_any_element()
}

/// The offer to resume the queue that was playing when Patina last stopped.
pub fn resume_banner(_window: &mut Window, cx: &mut App) -> AnyElement {
    let s = store(cx).read(cx);
    let saved = match (&s.saved_queue, &s.now_playing) {
        (Some(saved), None) => saved.clone(),
        _ => return div().into_any_element(),
    };
    let tint = s.tint();
    let theme = cx.theme().clone();
    let (title, sub) = format::playing_lines(&saved.current);
    let muted = theme.muted_foreground;
    let clear = |cx: &mut App| {
        store(cx).update(cx, |s, cx| {
            s.saved_queue = None;
            cx.notify();
        })
    };
    div()
        .flex_none()
        .px_4()
        .pt_3()
        .child(
            h_flex()
                .items_center()
                .gap_3()
                .py_2p5()
                .pl_4()
                .pr_2p5()
                .rounded(px(10.))
                .border_1()
                .border_color(theme.border)
                .bg(theme.colors.group_box)
                .text_sm()
                .child(Icon::new(IconName::Play).size_4().flex_none().text_color(tint))
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child("Resume ")
                        .child(div().font_medium().child(title))
                        .when(!sub.is_empty(), |d| d.child(div().text_color(muted).child(format!(" · {sub}"))))
                        .when(saved.position_ms > 0, |d| d.child(div().text_color(muted).child(format!(" at {}", format::clock(saved.position_ms)))))
                        .when(saved.count > 1, |d| d.child(div().text_color(muted).truncate().child(format!(" · {} of {} in the queue", saved.index + 1, saved.count)))),
                )
                .child(
                    h_flex()
                        .flex_none()
                        .items_center()
                        .gap_1p5()
                        .child(Button::new("resume-saved").primary().small().label("Resume").on_click(move |_, _, cx| {
                            let app = core(cx);
                            let _guard = patina_core::runtime().enter();
                            match ops::playback::resume_saved(&app) {
                                Ok(()) => clear(cx),
                                Err(e) => crate::store::error(cx, message(&e)),
                            }
                        }))
                        .child(Button::new("resume-later").ghost().small().label("Not now").on_click(move |_, _, cx| clear(cx)))
                        .child(Button::new("resume-forget").ghost().small().label("Forget").on_click(move |_, _, cx| {
                            clear(cx);
                            let app = core(cx);
                            cx.background_spawn(blocking(move || ops::playback::forget_saved(&app))).detach();
                        })),
                ),
        )
        .into_any_element()
}

// The queue panel ----------------------------------------------------------------

/// A queue entry being dragged to a new place.
#[derive(Clone)]
struct DragEntry {
    uid: u64,
    title: SharedString,
}

impl Render for DragEntry {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_1p5()
            .rounded(px(6.))
            .bg(cx.theme().popover)
            .border_1()
            .border_color(cx.theme().border)
            .shadow_md()
            .text_sm()
            .child(self.title.clone())
    }
}

/// Whether moving `uid` before `before` (`None` = the end) changes the order.
fn moves(uids: &[u64], uid: u64, before: Option<u64>) -> bool {
    let Some(from) = uids.iter().position(|u| *u == uid) else { return false };
    let to = match before {
        None => uids.len(),
        Some(b) => match uids.iter().position(|u| *u == b) {
            Some(i) => i,
            None => return false,
        },
    };
    to != from && to != from + 1
}

/// The running play queue. Rows reorder by dragging.
struct QueuePanel {
    scroll: ScrollHandle,
    /// Whether the playing entry has been scrolled into view since the panel opened.
    placed: bool,
}

impl QueuePanel {
    fn new() -> Self {
        Self { scroll: ScrollHandle::new(), placed: false }
    }

    fn drop_on(uids: Vec<u64>, before: Option<u64>) -> impl Fn(&DragEntry, &mut Window, &mut App) + 'static {
        move |d, _, cx| {
            if moves(&uids, d.uid, before) {
                edit(QueueCommand::Move { uid: d.uid, before }, cx);
            }
        }
    }
}

impl Render for QueuePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let s = store(cx).read(cx);
        let tint = s.tint();
        let Some(queue): Option<QueueView> = s.queue.clone() else { return div().into_any_element() };
        let current = queue.items.iter().position(|q| Some(q.uid) == queue.current);
        let upcoming = current.map_or(queue.items.len(), |c| queue.items.len() - c - 1);
        if !self.placed && !queue.items.is_empty() {
            // Open with the playing entry near the top, one earlier entry above it.
            self.scroll.scroll_to_top_of_item(current.unwrap_or(0).saturating_sub(1));
            self.placed = true;
        }
        let uids: Vec<u64> = queue.items.iter().map(|q| q.uid).collect();
        let muted = theme.muted_foreground;
        let (shuffled, stop_after) = (queue.shuffled, queue.stop_after);

        let header = h_flex()
            .flex_none()
            .items_center()
            .gap_0p5()
            .py_1p5()
            .pr_1p5()
            .pl_4()
            .border_b_1()
            .border_color(theme.border)
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .text_xs()
                    .child(div().font_semibold().child("UP NEXT"))
                    .child(div().text_color(muted).child(format!("· {}", if upcoming > 0 { format::plural(upcoming as i64, "item") } else { "Nothing".into() }))),
            )
            .child(
                Button::new("q-shuffle")
                    .ghost()
                    .small()
                    .icon(IconName::Shuffle)
                    .tooltip(if shuffled { "Unshuffle" } else { "Shuffle" })
                    .text_color(if shuffled { tint } else { muted })
                    .on_click(move |_, _, cx| edit(QueueCommand::Shuffle { on: !shuffled }, cx)),
            )
            .child(
                Button::new("q-stop-after")
                    .ghost()
                    .small()
                    .icon(IconName::CircleStop)
                    .tooltip(if stop_after { "Keep playing after this" } else { "Stop after this" })
                    .text_color(if stop_after { tint } else { muted })
                    .on_click(move |_, _, cx| edit(QueueCommand::StopAfter { on: !stop_after }, cx)),
            );

        let rows = queue.items.iter().enumerate().map(|(i, entry)| {
            let (t, sub) = format::playing_lines(&entry.item);
            let playing = Some(i) == current;
            let past = current.is_some_and(|c| i < c);
            let uid = entry.uid;
            let square = format::shape_for(&entry.item) == format::Shape::Square;
            let drag = DragEntry { uid, title: t.clone().into() };
            h_flex()
                .id(("q-row", uid as usize))
                .group("q-row")
                .h(px(48.))
                .flex_none()
                .gap_3()
                .pl_2()
                .pr(px(6.))
                .rounded(px(6.))
                .when(past, |d| d.opacity(0.45))
                .when(playing, |d| d.bg(theme.muted))
                .when(!playing, |d| d.cursor_pointer().hover(|s| s.bg(theme.muted.opacity(0.6))))
                .on_click(move |_, _, cx| {
                    if !playing {
                        edit(QueueCommand::Jump { uid }, cx);
                    }
                })
                .on_drag(drag, |d, _, _, cx| cx.new(|_| d.clone()))
                .drag_over::<DragEntry>(move |s, _, _, _| s.border_t_1().border_color(tint))
                .on_drop(Self::drop_on(uids.clone(), Some(uid)))
                .child(
                    div()
                        .h_9()
                        .w(px(if square { 36. } else { 24. }))
                        .flex_none()
                        .rounded(px(4.))
                        .overflow_hidden()
                        .bg(theme.muted)
                        .children(art_of(entry.item.poster().or(entry.item.thumb.as_deref()), 48., 48., None).map(|i| i.size_full().object_fit(ObjectFit::Cover))),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(div().text_size(px(12.5)).truncate().when(playing, |d| d.font_medium()).child(t))
                        .child(div().text_xs().text_color(muted).truncate().child(sub)),
                )
                .child(
                    h_flex()
                        .flex_none()
                        .items_center()
                        .gap_0p5()
                        .text_xs()
                        .text_color(muted)
                        .when(playing, |d| d.child(div().pr_1p5().child(Icon::new(IconName::AudioLines).size_4().text_color(tint))))
                        .when(!playing, |d| {
                            d.when_some(entry.item.duration, |d, ms| d.child(if entry.item.kind() == "track" { format::clock(ms) } else { format::duration(ms) }))
                                .child(
                                    div().opacity(0.).group_hover("q-row", |s| s.opacity(1.)).child(
                                        Button::new(("q-remove", uid as usize))
                                            .ghost()
                                            .xsmall()
                                            .icon(IconName::X)
                                            .tooltip("Remove from queue")
                                            .text_color(muted)
                                            .on_click(move |_, _, cx| edit(QueueCommand::Remove { uid }, cx)),
                                    ),
                                )
                        }),
                )
        });

        v_flex()
            .size_full()
            .min_h_0()
            .child(header)
            .child(
                v_flex()
                    .id("queue-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .p_1()
                    .children(rows)
                    .when(queue.items.is_empty(), |d| d.child(div().p_6().text_sm().text_center().text_color(muted).child("The queue is empty.")))
                    .child(
                        div()
                            .id("q-end")
                            .flex_none()
                            .h_4()
                            .drag_over::<DragEntry>(move |s, _, _, _| s.border_t_1().border_color(tint))
                            .on_drop(Self::drop_on(uids, None)),
                    ),
            )
            .into_any_element()
    }
}

// The lyrics panel ---------------------------------------------------------------

/// The line playing at `pos`: the last timed line that has started.
fn active_line(lyrics: &Lyrics, pos: i64) -> Option<usize> {
    if !lyrics.synced {
        return None;
    }
    lyrics.lines.iter().enumerate().filter(|(_, l)| l.time_ms.is_some_and(|t| t <= pos)).map(|(i, _)| i).next_back()
}

/// Lyrics for the playing track. Synced lines follow playback.
struct LyricsPanel {
    key: Option<String>,
    lyrics: Load<Option<Lyrics>>,
    /// The last position tick and when it came, to interpolate between ticks.
    anchor: (i64, Instant, bool),
    /// The reader's own scrolling holds auto-scroll off until then.
    held_until: Option<Instant>,
    /// The line last scrolled to, once the lyrics loaded.
    placed: Option<usize>,
    rendered_at: Instant,
    scroll: ScrollHandle,
    _tick: Task<()>,
}

impl LyricsPanel {
    fn new(cx: &mut Context<Self>) -> Self {
        let tick = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(200)).await;
                let alive = this.update(cx, |l, cx| {
                    let synced = l.lyrics.ready().and_then(|x| x.as_ref()).is_some_and(|x| x.synced);
                    if synced && l.rendered_at.elapsed() < Duration::from_secs(1) {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        Self {
            key: None,
            lyrics: Load::Loading,
            anchor: (0, Instant::now(), true),
            held_until: None,
            placed: None,
            rendered_at: Instant::now(),
            scroll: ScrollHandle::new(),
            _tick: tick,
        }
    }

    fn load(&mut self, key: String, cx: &mut Context<Self>) {
        self.key = Some(key.clone());
        self.lyrics = Load::Loading;
        self.placed = None;
        let app = core(cx);
        cx.spawn(async move |this, cx| {
            let k = key.clone();
            let result = bg(async move { patina_core::lyrics::load(&app.server()?, &k).await }).await;
            this.update(cx, |l, cx| {
                if l.key.as_deref() == Some(key.as_str()) {
                    l.lyrics = Load::from_result(result);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Centre line `i` in the list, using the last layout.
    fn centre(&mut self, i: usize) -> bool {
        let (Some(line), view) = (self.scroll.bounds_for_item(i), self.scroll.bounds()) else { return false };
        if view.size.height <= px(0.) {
            return false;
        }
        let max = self.scroll.max_offset().y;
        let y = (view.top() + view.size.height / 2. - (line.top() + line.size.height / 2.)).clamp(-max, px(0.));
        self.scroll.set_offset(point(px(0.), y));
        true
    }
}

impl Render for LyricsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        self.rendered_at = Instant::now();
        let Some(np) = store(cx).read(cx).now_playing.clone() else { return div().into_any_element() };
        let key = np.item.rating_key().to_string();
        if self.key.as_deref() != Some(key.as_str()) {
            self.load(key, cx);
        }
        if self.anchor.0 != np.position_ms || self.anchor.2 != np.paused {
            self.anchor = (np.position_ms, Instant::now(), np.paused);
        }
        let pos = if self.anchor.2 { self.anchor.0 } else { self.anchor.0 + self.anchor.1.elapsed().as_millis().min(1500) as i64 };

        let header = h_flex()
            .flex_none()
            .h(px(44.))
            .min_w_0()
            .items_center()
            .gap_1p5()
            .px_4()
            .border_b_1()
            .border_color(theme.border)
            .text_xs()
            .child(div().flex_none().font_semibold().child("LYRICS"))
            .child(div().min_w_0().truncate().text_color(theme.muted_foreground).child(format!("· {}", np.item.title())));

        let body = match &self.lyrics {
            Load::Loading => crate::widgets::loading().py_7().into_any_element(),
            Load::Failed(e) => div()
                .p_4()
                .child(div().p_3().rounded(px(8.)).border_1().border_color(theme.border).text_sm().text_color(theme.danger).child(e.clone()))
                .into_any_element(),
            Load::Ready(None) => div().p_8().text_sm().text_center().text_color(theme.muted_foreground).child("No lyrics for this track").into_any_element(),
            Load::Ready(Some(l)) if l.lines.is_empty() => div().p_8().text_sm().text_center().text_color(theme.muted_foreground).child("No lyrics for this track").into_any_element(),
            Load::Ready(Some(l)) => {
                let l = l.clone();
                let active = active_line(&l, pos);
                let held = self.held_until.is_some_and(|t| Instant::now() < t);
                if let Some(i) = active
                    && !held
                    && self.placed != Some(i)
                    && self.centre(i)
                {
                    self.placed = Some(i);
                }
                let pad = window.viewport_size().height * 0.22;
                let (text2, text3) = (theme.muted_foreground, theme::faint());
                v_flex()
                    .id("lyrics")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .px(px(22.))
                    .when(l.synced, |d| d.py(pad))
                    .when(!l.synced, |d| d.pt(px(18.)).pb_6())
                    .text_base()
                    .line_height(relative(1.55))
                    .on_scroll(cx.listener(|this, _: &ScrollEvent, _, _| this.held_until = Some(Instant::now() + Duration::from_secs(4))))
                    .children(l.lines.iter().enumerate().map(|(i, line)| {
                        let text = if line.text.is_empty() { " ".to_string() } else { line.text.clone() };
                        let row = div().id(("lyric", i)).w_full().py(px(3.)).child(text);
                        if !l.synced {
                            return row.text_color(text2);
                        }
                        let on = Some(i) == active;
                        match line.time_ms {
                            Some(ms) => row
                                .text_color(if on { theme.foreground } else { text3 })
                                .cursor_pointer()
                                .hover(|s| if on { s } else { s.text_color(text2) })
                                .on_click(cx.listener(move |this, _, _, _| {
                                    this.held_until = None;
                                    send(Control::SeekTo(ms as f64 / 1000.));
                                })),
                            None => row.text_color(text3),
                        }
                    }))
                    .into_any_element()
            }
        };
        v_flex().size_full().min_h_0().child(header).child(body).into_any_element()
    }
}

// The player panel ---------------------------------------------------------------

const SPEEDS: [f64; 6] = [0.5, 0.75, 1., 1.25, 1.5, 2.];
const SLEEPS: [u32; 5] = [15, 30, 45, 60, 90];

/// "12 min" or "40s" until the sleep timer ends.
fn left_label(ms: i64) -> String {
    let s = (ms as f64 / 1000.).ceil() as i64;
    if s >= 60 { format!("{} min", (s as f64 / 60.).ceil() as i64) } else { format!("{s}s") }
}

/// Volume, speed, repeat, sleep timer, chapters and playback diagnostics.
struct PlayerPanel {
    volume: Entity<SliderState>,
    /// When the slider last changed the volume; the player's value wins again a moment later.
    dragged_at: Option<Instant>,
    chapters: ScrollHandle,
    placed: bool,
    _subs: Vec<Subscription>,
    _tick: Task<()>,
}

impl PlayerPanel {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let volume = cx.new(|_| SliderState::new().min(0.).max(130.).step(1.).default_value(100.));
        let subs = vec![cx.subscribe_in(&volume, window, |this, _, e: &SliderEvent, _, cx| {
            if let SliderEvent::Change(v) = e {
                this.dragged_at = Some(Instant::now());
                tune(Tune::Volume { value: v.start() as f64 }, cx);
            }
        })];
        // The sleep timer counts down on its own; tick while it runs.
        let tick = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let alive = this.update(cx, |_, cx| {
                    if store(cx).read(cx).playback.as_ref().is_some_and(|p| p.sleep.as_ref().is_some_and(|s| s.ends_at_ms.is_some())) {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        Self { volume, dragged_at: None, chapters: ScrollHandle::new(), placed: false, _subs: subs, _tick: tick }
    }
}

impl Render for PlayerPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let s = store(cx).read(cx);
        let tint = s.tint();
        let track = s.now_playing.as_ref().is_some_and(|n| n.item.kind() == "track");
        let Some(ps) = s.playback.clone() else { return div().into_any_element() };
        if self.dragged_at.is_none_or(|t| t.elapsed() > Duration::from_secs(1)) && (self.volume.read(cx).value().start() as f64 - ps.volume).abs() > 0.5 {
            let v = ps.volume as f32;
            self.volume.update(cx, |st, cx| st.set_value(v, window, cx));
        }
        if !self.placed && !ps.chapters.is_empty() {
            self.chapters.scroll_to_top_of_item(ps.chapter.unwrap_or(0).saturating_sub(1));
            self.placed = true;
        }
        let muted = theme.muted_foreground;
        let label = |text: &str| div().w_12().flex_none().text_size(px(10.5)).font_semibold().text_color(muted).child(text.to_uppercase());
        let chip = |id: ElementId, on: bool| {
            Button::new(id)
                .outline()
                .xsmall()
                .rounded(px(999.))
                .px_2p5()
                .text_color(if on { tint } else { muted })
                .when(on, |b| b.border_color(tint.opacity(0.55)))
        };

        let speed = h_flex().items_center().gap_2p5().child(label("Speed")).child(h_flex().flex_wrap().gap_1().children(SPEEDS.iter().enumerate().map(|(i, &v)| {
            chip(("speed", i).into(), (ps.speed - v).abs() < 0.001).label(format!("{v}x")).on_click(move |_, _, cx| tune(Tune::Speed { value: v }, cx))
        })));
        let repeats = [(Repeat::Off, "Off", None), (Repeat::All, "All", Some(IconName::Repeat)), (Repeat::One, "One", Some(IconName::Repeat1))];
        let repeat = h_flex().items_center().gap_2p5().child(label("Repeat")).child(h_flex().flex_wrap().gap_1().children(repeats.into_iter().enumerate().map(|(i, (mode, text, icon))| {
            chip(("repeat", i).into(), ps.repeat == mode).when_some(icon, |b, icon| b.icon(icon)).label(text).on_click(move |_, _, cx| tune(Tune::Repeat { mode }, cx))
        })));
        let end_of_item = ps.sleep.as_ref().is_some_and(|s| s.mode == "endOfItem");
        let sleep = h_flex().items_start().gap_2p5().child(label("Sleep").pt_1()).child(
            h_flex()
                .flex_wrap()
                .gap_1()
                .child(chip("sleep-off".into(), ps.sleep.is_none()).label("Off").on_click(|_, _, cx| tune(Tune::SleepOff, cx)))
                .children(SLEEPS.iter().map(|&m| chip(("sleep", m as usize).into(), false).label(format!("{m}m")).on_click(move |_, _, cx| tune(Tune::SleepMinutes { minutes: m }, cx))))
                .child(chip("sleep-end".into(), end_of_item).label("End of item").on_click(|_, _, cx| tune(Tune::SleepEndOfItem, cx))),
        );
        let left = ps.sleep.as_ref().and_then(|s| s.ends_at_ms).map(|end| (end - now_ms()).max(0));
        let sleep_note = match (left, end_of_item) {
            (Some(ms), _) => Some(format!("Playback stops in {}.", left_label(ms))),
            (None, true) => Some("Playback stops when this item ends.".to_string()),
            _ => None,
        };

        let controls = v_flex()
            .gap_3()
            .child(
                h_flex()
                    .items_center()
                    .gap_2p5()
                    .child(
                        Button::new("mute")
                            .ghost()
                            .small()
                            .icon(if ps.muted { IconName::VolumeX } else { IconName::Volume2 })
                            .tooltip(if ps.muted { "Unmute" } else { "Mute" })
                            .text_color(if ps.muted { tint } else { muted })
                            .on_click(|_, _, cx| tune(Tune::Mute { on: None }, cx)),
                    )
                    .child(div().flex_1().child(Slider::new(&self.volume).accessibility_label("Volume")))
                    .child(div().w_10().text_right().text_sm().text_color(muted).child(format!("{}%", ps.volume.round()))),
            )
            .child(speed)
            .child(repeat)
            .child(sleep)
            .when_some(sleep_note, |d, n| d.child(div().text_xs().text_color(muted).child(n)));

        let separator = || div().flex_none().h_px().bg(theme.border);
        let chapters = (!ps.chapters.is_empty()).then(|| {
            v_flex().gap_2().child(heading("Chapters", cx)).child(
                v_flex()
                    .id("chapters")
                    .mx(px(-4.))
                    .max_h(px(144.))
                    .overflow_y_scroll()
                    .track_scroll(&self.chapters)
                    .children(ps.chapters.iter().enumerate().map(|(i, c)| {
                        let current = ps.chapter == Some(i);
                        h_flex()
                            .id(("chapter", i))
                            .flex_none()
                            .gap_2p5()
                            .px_2()
                            .py_1p5()
                            .rounded(px(6.))
                            .text_sm()
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.secondary))
                            .when(current, |d| d.text_color(tint))
                            .on_click(move |_, _, cx| tune(Tune::Chapter { index: i }, cx))
                            .child(div().flex_1().min_w_0().truncate().child(if c.title.is_empty() { format!("Chapter {}", i + 1) } else { c.title.clone() }))
                            .child(div().text_color(muted).child(format::clock(c.time_ms)))
                    })),
            )
        });

        let d = &ps.diagnostics;
        let connection = match d.connection.as_ref() {
            Some(c) => {
                let kind = match c.kind.as_str() {
                    "local" => "Local network",
                    "remote" => "Remote",
                    "relay" => "Plex relay",
                    "offline" => "Downloaded file",
                    other => other,
                }
                .to_string();
                h_flex().child(kind).when_some(c.address.clone(), |d, a| d.child(div().text_color(muted).child(format!(" · {a}"))))
            }
            None => h_flex().child("Unknown"),
        };
        let buffer = if d.buffering {
            div().child(d.buffer_percent.map_or("Buffering".to_string(), |p| format!("Buffering, {p}% full")))
        } else if let Some(secs) = d.buffered_secs {
            div().child(format!("{secs:.1} s ahead"))
        } else {
            div().text_color(muted).child("n/a")
        };
        let mut facts: Vec<(&str, AnyElement)> = vec![("Connection", connection.into_any_element()), ("Buffer", buffer.into_any_element())];
        if !track {
            let decoder = match d.hwdec.as_deref() {
                None => "Unknown".to_string(),
                Some("no") => "Software".to_string(),
                Some(hw) => format!("Hardware ({hw})"),
            };
            facts.push(("Decoder", div().child(decoder).into_any_element()));
            facts.push(("Dropped frames", div().child(format!("{} output · {} decoder", d.dropped_frames, d.decoder_dropped_frames)).into_any_element()));
        }
        if let Some(ms) = d.recovered_from_ms {
            facts.push(("Recovered", div().child(format!("Resumed from {} after the connection failed", format::clock(ms))).into_any_element()));
        }
        let diagnostics = v_flex().gap_2().child(heading("Diagnostics", cx)).child(v_flex().gap_1p5().text_xs().children(facts.into_iter().map(|(k, v)| {
            h_flex().items_start().gap_4().child(div().w(px(92.)).flex_none().text_color(muted).child(k)).child(div().flex_1().min_w_0().child(v))
        })));

        v_flex()
            .id("player-controls")
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .p_4()
            .gap_4()
            .child(controls)
            .when_some(chapters, |d, c| d.child(separator()).child(c))
            .child(separator())
            .child(diagnostics)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    use patina_core::controls::{Diagnostics, SleepState};
    use patina_core::lyrics::Line;

    fn state() -> PlaybackState {
        PlaybackState { volume: 100., muted: false, speed: 1., repeat: Repeat::Off, sleep: None, chapters: vec![], chapter: None, diagnostics: Diagnostics::default() }
    }

    #[test]
    fn chips_only_show_changes() {
        assert!(chips(&state(), 0).is_empty());
        let mut ps = state();
        ps.volume = 80.;
        ps.speed = 1.5;
        ps.repeat = Repeat::One;
        ps.sleep = Some(SleepState { mode: "minutes".into(), ends_at_ms: Some(90_000) });
        assert_eq!(chips(&ps, 0), ["Vol 80%", "1.5x", "Repeat one", "Sleep 2m"]);
        ps.muted = true;
        ps.diagnostics.buffering = true;
        ps.diagnostics.buffer_percent = Some(40);
        assert_eq!(chips(&ps, 89_000)[..2], ["Buffering 40%", "Muted"]);
    }

    #[test]
    fn media_labels() {
        let mut m = Media { video_resolution: Some("1080".into()), video_codec: Some("h264".into()), ..Default::default() };
        assert_eq!(media_label(&m, 0), "1080p H264");
        m.video_resolution = Some("4k".into());
        m.video_codec = None;
        assert_eq!(media_label(&m, 0), "4K");
        let m = Media { height: Some(720), ..Default::default() };
        assert_eq!(media_label(&m, 1), "720p");
        assert_eq!(media_label(&Media::default(), 1), "version 2");
    }

    #[test]
    fn queue_moves() {
        let uids = [1, 2, 3];
        assert!(!moves(&uids, 1, Some(1)));
        assert!(!moves(&uids, 1, Some(2)));
        assert!(moves(&uids, 1, Some(3)));
        assert!(moves(&uids, 1, None));
        assert!(!moves(&uids, 3, None));
        assert!(moves(&uids, 3, Some(1)));
        assert!(!moves(&uids, 9, None));
    }

    #[test]
    fn lyrics_follow_position() {
        let line = |t: Option<i64>| Line { time_ms: t, text: "x".into() };
        let l = Lyrics { synced: true, lines: vec![line(Some(0)), line(None), line(Some(5000)), line(Some(9000))] };
        assert_eq!(active_line(&l, 0), Some(0));
        assert_eq!(active_line(&l, 6000), Some(2));
        assert_eq!(active_line(&l, 20_000), Some(3));
        assert_eq!(active_line(&Lyrics { synced: false, ..l }, 6000), None);
    }

    #[test]
    fn sleep_labels() {
        assert_eq!(left_label(40_000), "40s");
        assert_eq!(left_label(61_000), "2 min");
    }
}
