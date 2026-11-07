//! Set audio and subtitle tracks for many episodes at once. The matching is
//! done in `patina_core::tracks`; this dialog only collects the choices.

use crate::prelude::*;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::progress::Progress;
use patina_core::tracks::{self, Query, View};
use patina_plex::Metadata;

use super::form::{self, Guard, Modal};
use crate::core::{bg, core, message};
use crate::store::store;

/// The show an item belongs to: its rating key and title.
pub fn show_of(m: &Metadata) -> (Option<String>, String) {
    match m.kind() {
        "show" => (m.rating_key.clone(), m.title().to_string()),
        "season" => (m.parent_rating_key.clone(), m.parent_title.clone().unwrap_or_default()),
        _ => (m.grandparent_rating_key.clone(), m.grandparent_title.clone().unwrap_or_default()),
    }
}

fn episodes(n: usize) -> String {
    format!("{n} episode{}", if n == 1 { "" } else { "s" })
}

/// "2 episodes have no audio matching “Japanese”, left unchanged".
fn miss_head(n: usize, what: &str) -> String {
    format!("{} {} {what}, left unchanged", episodes(n), if n == 1 { "has" } else { "have" })
}

pub struct TrackDialog {
    show: Option<String>,
    title: String,
    guard: Guard,
    loaded: bool,
    error: Option<String>,
    season: Option<i64>,
    reference: Option<String>,
    audio: Option<i64>,
    /// "keep", "off" or a stream id.
    subtitle: String,
    remember: bool,
    view: Option<View>,
    /// Applied so far, of how many, while applying.
    progress: Option<(usize, usize)>,
    open_audio: bool,
    open_subs: bool,
}

pub fn open(item: Metadata, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| TrackDialog::new(&item, cx));
    form::show(view, "Audio & subtitles", form::WIDE, window, cx);
}

impl Modal for TrackDialog {
    fn busy(&self) -> bool {
        self.progress.is_some()
    }
}

impl TrackDialog {
    fn new(item: &Metadata, cx: &mut Context<Self>) -> Self {
        let (show, title) = show_of(item);
        let kind = item.kind();
        let mut this = Self {
            show: show.clone(),
            title,
            guard: Guard::new(cx),
            loaded: false,
            error: None,
            season: match kind {
                "season" => item.index,
                "episode" => item.parent_index,
                _ => None,
            },
            reference: (kind == "episode").then(|| item.rating_key().to_string()),
            audio: None,
            subtitle: "keep".into(),
            remember: false,
            view: None,
            progress: None,
            open_audio: false,
            open_subs: false,
        };
        let Some(show) = show else {
            this.error = Some("This item doesn't belong to a show.".into());
            return this;
        };
        // Load the show's episodes.
        let app = core(cx);
        cx.spawn(async move |this, cx| {
            let result = bg(async move { tracks::load(&app, &show).await }).await;
            this.update(cx, |d, cx| {
                if !d.guard.valid(cx) {
                    return;
                }
                match result {
                    Ok(()) => {
                        d.loaded = true;
                        d.replan(cx);
                    }
                    Err(e) => d.error = Some(message(&e)),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        this
    }

    fn query(&self) -> Option<Query> {
        Some(Query { show: self.show.clone()?, season: self.season, reference: self.reference.clone(), audio: self.audio, subtitle: self.subtitle.clone() })
    }

    /// Plan again after a choice changed.
    fn replan(&mut self, cx: &mut Context<Self>) {
        if !self.loaded || !self.guard.valid(cx) {
            return;
        }
        let Some(q) = self.query() else { return };
        match tracks::view(&core(cx), &q) {
            Ok(v) => self.view = Some(v),
            Err(e) => self.error = Some(message(&e)),
        }
        cx.notify();
    }

    fn reset_tracks(&mut self) {
        self.audio = None;
        self.subtitle = "keep".into();
    }

    fn can_apply(&self) -> bool {
        self.view.as_ref().is_some_and(|v| !v.plan.changes.is_empty() || (self.remember && !v.nothing_chosen))
    }

    fn apply(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = &self.view else { return };
        if !self.guard.valid(cx) {
            window.close_dialog(cx);
            return;
        }
        let changes = view.plan.changes.clone();
        let remember = self.remember.then(|| self.query()).flatten();
        self.progress = Some((0, changes.len()));
        cx.notify();
        let app = core(cx);
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<(usize, usize)>();
        // Progress as the updates land.
        cx.spawn(async move |this, cx| {
            use futures::StreamExt as _;
            while let Some(p) = rx.next().await {
                if this.update(cx, |d, cx| {
                    d.progress = Some(p);
                    cx.notify();
                })
                .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let fut = async move {
            // Before applying: applying drops the loaded tracks the rules are read from.
            if let Some(q) = remember {
                patina_core::prefs::remember_tracks(&app, &q)?;
            }
            if !changes.is_empty() {
                tracks::apply(&app, changes, move |done, total| {
                    let _ = tx.unbounded_send((done, total));
                })
                .await?;
            }
            Ok::<_, anyhow::Error>(())
        };
        form::run(window, cx, fut, None, false, |d, _, window, cx| {
            d.progress = None;
            window.close_dialog(cx);
            cx.notify();
        });
    }

    fn missing(&self, id: &'static str, open: bool, head: String, list: &[String], cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        v_flex()
            .text_color(theme.muted_foreground)
            .child(
                h_flex()
                    .id(id)
                    .gap_2()
                    .items_center()
                    .cursor_pointer()
                    .child(Icon::new(IconName::TriangleAlert).size_3p5().text_color(hsla(0.11, 0.9, 0.5, 1.)))
                    .child(head)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if id == "no-audio" {
                            this.open_audio = !this.open_audio;
                        } else {
                            this.open_subs = !this.open_subs;
                        }
                        cx.notify();
                    })),
            )
            .when(open, |d| {
                d.child(
                    v_flex()
                        .id((id, 1usize))
                        .mt_2()
                        .pl_5()
                        .max_h(px(140.))
                        .overflow_y_scroll()
                        .text_xs()
                        .children(list.iter().map(|e| div().child(format!("• {e}")))),
                )
            })
            .into_any_element()
    }
}

impl Render for TrackDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let tint = store(cx).read(cx).tint();
        let busy = self.progress.is_some();
        let me = cx.entity().downgrade();
        let desc = format!("{}: pick tracks in one episode and Patina selects the closest match in every other.", self.title);
        let body = if let Some(e) = &self.error {
            form::error(e.clone(), cx).into_any_element()
        } else if let Some(view) = &self.view {
            if view.total == 0 {
                form::empty("No episodes", "This show has no episodes.", cx).into_any_element()
            } else {
                let mut seasons = vec![("all".to_string(), format!("All seasons ({})", episodes(view.total)))];
                seasons.extend(view.seasons.iter().map(|s| (s.index.to_string(), format!("{} ({})", s.title, episodes(s.count)))));
                let scope: Vec<(String, String)> = view.scope.iter().map(|e| (e.rating_key.clone(), e.label.clone())).collect();
                let mut audio = vec![("keep".to_string(), "Don't change".to_string())];
                audio.extend(view.audio.iter().map(|s| (s.id.to_string(), s.label.clone())));
                let mut subs = vec![("keep".to_string(), "Don't change".to_string()), ("off".to_string(), "None (turn off)".to_string())];
                subs.extend(view.subtitles.iter().map(|s| (s.id.to_string(), format!("{}{}", s.label, if s.external { " · external" } else { "" }))));
                let season_value = self.season.map_or("all".to_string(), |s| s.to_string());
                let audio_value = self.audio.map_or("keep".to_string(), |a| a.to_string());
                let pick = |f: fn(&mut Self, String)| {
                    let me = me.clone();
                    move |v: String, _: &mut Window, cx: &mut App| {
                        me.update(cx, |d, cx| {
                            f(d, v);
                            d.replan(cx);
                        })
                        .ok();
                    }
                };
                let status = if let Some((done, total)) = self.progress {
                    v_flex()
                        .gap_2p5()
                        .child(Progress::new("tracks-progress").color(tint).value(done as f32 / total.max(1) as f32 * 100.))
                        .child(format!("Updating {done} of {total}…"))
                        .into_any_element()
                } else if view.nothing_chosen {
                    div()
                        .text_color(theme.muted_foreground)
                        .child("Choose an audio or subtitle track. Matches are found by language first, then title, codec and channels.")
                        .into_any_element()
                } else if view.plan.episodes == 0 && view.plan.no_audio.is_empty() && view.plan.no_subtitle.is_empty() {
                    div().child("Every episode already uses these tracks.").into_any_element()
                } else {
                    div().child(format!("{} of {} will change.", view.plan.episodes, episodes(view.scope.len()))).into_any_element()
                };
                let no_audio = (!busy && !view.plan.no_audio.is_empty()).then(|| {
                    let head = miss_head(view.plan.no_audio.len(), &format!("no audio matching “{}”", view.audio_name.clone().unwrap_or_default()));
                    (head, view.plan.no_audio.clone())
                });
                let no_subs = (!busy && !view.plan.no_subtitle.is_empty()).then(|| {
                    let head = miss_head(view.plan.no_subtitle.len(), &format!("no subtitles matching “{}”", view.subtitle_name.clone().unwrap_or_default()));
                    (head, view.plan.no_subtitle.clone())
                });
                let nothing = view.nothing_chosen;
                let reference = view.reference.clone().unwrap_or_default();
                let no_audio_tracks = view.audio.is_empty();
                let (open_audio, open_subs, remember) = (self.open_audio, self.open_subs, self.remember);
                v_flex()
                    .child(
                        v_flex()
                            .gap_4()
                            .child(
                                h_flex()
                                    .gap_4()
                                    .items_start()
                                    .child(div().flex_1().min_w_0().child(form::field(
                                        "Apply to",
                                        form::select("tracks-season", seasons, &season_value, busy, pick(|d, v| {
                                            d.season = v.parse().ok();
                                            d.reference = None;
                                            d.reset_tracks();
                                        })),
                                        cx,
                                    )))
                                    .child(div().flex_1().min_w_0().child(form::field(
                                        "Take tracks from",
                                        form::select("tracks-reference", scope, &reference, busy, pick(|d, v| {
                                            d.reference = Some(v);
                                            d.reset_tracks();
                                        })),
                                        cx,
                                    ))),
                            )
                            .child(
                                h_flex()
                                    .gap_4()
                                    .items_start()
                                    .child(div().flex_1().min_w_0().child(form::field(
                                        "Audio",
                                        form::select("tracks-audio", audio, &audio_value, busy || no_audio_tracks, pick(|d, v| d.audio = v.parse().ok())),
                                        cx,
                                    )))
                                    .child(div().flex_1().min_w_0().child(form::field(
                                        "Subtitles",
                                        form::select("tracks-subtitle", subs, &self.subtitle, busy, pick(|d, v| d.subtitle = v)),
                                        cx,
                                    ))),
                            ),
                    )
                    .child(
                        h_flex()
                            .mt_4()
                            .gap_3()
                            .items_start()
                            .child(Checkbox::new("tracks-remember").checked(remember).disabled(busy || nothing).on_click(cx.listener(|this, on: &bool, _, cx| {
                                this.remember = *on;
                                cx.notify();
                            })))
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(div().text_sm().child("Remember for this show"))
                                    .child(div().text_sm().text_color(theme.muted_foreground).child("Use these languages for new episodes too, and whenever the show starts playing.")),
                            ),
                    )
                    .child(
                        v_flex()
                            .mt_5()
                            .pt_4()
                            .border_t_1()
                            .border_color(theme.border)
                            .gap_2p5()
                            .text_sm()
                            .child(status)
                            .when_some(no_audio, |d, (head, list)| d.child(self.missing("no-audio", open_audio, head, &list, cx)))
                            .when_some(no_subs, |d, (head, list)| d.child(self.missing("no-subs", open_subs, head, &list, cx))),
                    )
                    .into_any_element()
            }
        } else {
            v_flex()
                .py_10()
                .gap_3()
                .items_center()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(gpui_kit::component::spinner::Spinner::new())
                .child("Reading tracks from every episode…")
                .into_any_element()
        };
        let label = match &self.view {
            Some(v) if v.plan.episodes > 0 => format!("Apply to {}", episodes(v.plan.episodes)),
            _ if self.remember && self.can_apply() => "Remember".into(),
            _ => "Apply".into(),
        };
        let can_apply = self.can_apply();
        v_flex().child(form::desc(desc, cx)).child(body).child(
            form::foot(cx)
                .child(Button::new("cancel").outline().label("Cancel").disabled(busy).on_click(|_, window, cx| window.close_dialog(cx)))
                .child(Button::new("apply").primary().label(label).disabled(busy || !can_apply).on_click(cx.listener(|this, _, window, cx| this.apply(window, cx)))),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    #[test]
    fn show_of_each_kind() {
        let ep: Metadata = serde_json::from_str(r#"{"type":"episode","ratingKey":"1011","grandparentRatingKey":"500","grandparentTitle":"Static"}"#).unwrap();
        assert_eq!(show_of(&ep), (Some("500".into()), "Static".into()));
        let season: Metadata = serde_json::from_str(r#"{"type":"season","ratingKey":"601","parentRatingKey":"500","parentTitle":"Static"}"#).unwrap();
        assert_eq!(show_of(&season), (Some("500".into()), "Static".into()));
        let show: Metadata = serde_json::from_str(r#"{"type":"show","ratingKey":"500","title":"Static"}"#).unwrap();
        assert_eq!(show_of(&show), (Some("500".into()), "Static".into()));
    }

    #[test]
    fn missing_heads() {
        assert_eq!(miss_head(1, "no audio matching “x”"), "1 episode has no audio matching “x”, left unchanged");
        assert_eq!(miss_head(3, "no subtitles"), "3 episodes have no subtitles, left unchanged");
    }
}
