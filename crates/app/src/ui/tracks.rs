//! Set audio and subtitle tracks for a whole show or season at once (like
//! PASTA): pick tracks in one episode and the closest match is selected in
//! every other episode.

use dioxus::prelude::*;
use futures::StreamExt;
use patina_plex::{Metadata, Part, Stream};

use crate::state::{AppState, use_app};
use crate::ui::components::Spinner;
use crate::ui::icons::{Ic, Icon};

/// Whether an item can open the dialog.
pub fn supports(item: &Metadata) -> bool {
    matches!(item.kind(), "show" | "season" | "episode")
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum SubChoice {
    Keep,
    Off,
    Track(i64),
}

impl SubChoice {
    fn parse(v: &str) -> Self {
        match v {
            "off" => Self::Off,
            v => v.parse().map(Self::Track).unwrap_or(Self::Keep),
        }
    }
}

#[derive(Debug, PartialEq)]
struct PartChange {
    part_id: i64,
    audio: Option<i64>,
    subtitle: Option<i64>,
}

/// What applying a choice would do across the episodes in scope.
#[derive(Debug, Default, PartialEq)]
struct Plan {
    changes: Vec<PartChange>,
    /// Episodes (with changes) that would be touched.
    episodes: usize,
    no_audio: Vec<String>,
    no_subtitle: Vec<String>,
}

fn episode_label(ep: &Metadata) -> String {
    format!("{} · {}", ep.episode_code(), ep.title())
}

fn plan_part(part: &Part, audio: Option<&Stream>, subtitle: Option<Option<&Stream>>) -> (Option<i64>, Option<i64>, bool, bool) {
    let (mut a, mut s, mut miss_a, mut miss_s) = (None, None, false, false);
    if let Some(tpl) = audio {
        match part.matching_stream(tpl) {
            Some(m) if m.selected => {}
            Some(m) => a = m.id,
            None => miss_a = true,
        }
    }
    match subtitle {
        None => {}
        // Turn subtitles off.
        Some(None) => {
            if part.selected_subtitle().is_some() {
                s = Some(0);
            }
        }
        Some(Some(tpl)) => match part.matching_stream(tpl) {
            Some(m) if m.selected => {}
            Some(m) => s = m.id,
            None => miss_s = true,
        },
    }
    (a, s, miss_a, miss_s)
}

fn plan(episodes: &[&Metadata], audio: Option<&Stream>, subtitle: Option<Option<&Stream>>) -> Plan {
    let mut out = Plan::default();
    for ep in episodes {
        let (mut touched, mut miss_a, mut miss_s) = (false, false, false);
        for part in ep.media.iter().flat_map(|m| &m.parts) {
            let Some(part_id) = part.id else { continue };
            let (a, s, ma, ms) = plan_part(part, audio, subtitle);
            miss_a |= ma;
            miss_s |= ms;
            if a.is_some() || s.is_some() {
                touched = true;
                out.changes.push(PartChange { part_id, audio: a, subtitle: s });
            }
        }
        out.episodes += touched as usize;
        if miss_a {
            out.no_audio.push(episode_label(ep));
        }
        if miss_s {
            out.no_subtitle.push(episode_label(ep));
        }
    }
    out
}

/// Modal shown while `AppState::tracks_target` holds a show, season or episode.
#[component]
pub fn TrackDialog() -> Element {
    let app = use_app();
    let Some(item) = (app.tracks_target)() else { return rsx! {} };
    rsx! { TrackForm { key: "{item.rating_key()}", item } }
}

#[component]
fn TrackForm(item: Metadata) -> Element {
    let app = use_app();
    let mut target = app.tracks_target;
    let show_rk = match item.kind() {
        "show" => Some(item.rating_key().to_string()),
        "season" => item.parent_rating_key.clone(),
        _ => item.grandparent_rating_key.clone(),
    };
    let show_title = match item.kind() {
        "show" => item.title().to_string(),
        "season" => item.parent_title.clone().unwrap_or_default(),
        _ => item.grandparent_title.clone().unwrap_or_default(),
    };
    let mut season = use_signal(|| match item.kind() {
        "season" => item.index,
        "episode" => item.parent_index,
        _ => None,
    });
    let mut reference = use_signal(|| (item.kind() == "episode").then(|| item.rating_key().to_string()));
    let mut audio_choice = use_signal(|| None::<i64>);
    let mut sub_choice = use_signal(|| SubChoice::Keep);
    let progress = use_signal(|| None::<(usize, usize)>);

    let episodes = use_resource(move || {
        let rk = show_rk.clone();
        async move {
            match rk {
                Some(rk) => app.server().all_leaves_with_streams(&rk).await,
                None => Ok(Vec::new()),
            }
        }
    });

    let busy = progress().is_some();
    let close = move |_| {
        if !busy {
            target.set(None);
        }
    };

    let read = episodes.read();
    let body = match &*read {
        None => rsx! {
            div { class: "center", style: "min-height: 160px; gap: 12px",
                Spinner {}
                span { class: "muted", "Reading tracks from every episode…" }
            }
        },
        Some(Err(e)) => rsx! { div { class: "error-box", "{e}" } },
        Some(Ok(all)) if all.is_empty() => rsx! { p { class: "muted", "This show has no episodes." } },
        Some(Ok(all)) => {
            // Seasons in library order, with episode counts.
            let mut seasons: Vec<(i64, String, usize)> = Vec::new();
            for ep in all {
                let idx = ep.parent_index.unwrap_or_default();
                match seasons.iter_mut().find(|s| s.0 == idx) {
                    Some(s) => s.2 += 1,
                    None => seasons.push((idx, ep.parent_title.clone().unwrap_or_else(|| format!("Season {idx}")), 1)),
                }
            }
            let scope: Vec<&Metadata> = all.iter().filter(|ep| season().is_none_or(|s| ep.parent_index == Some(s))).collect();
            let reference_ep = reference()
                .and_then(|rk| scope.iter().find(|ep| ep.rating_key() == rk).copied())
                .or(scope.first().copied());
            let ref_part = reference_ep.and_then(|ep| ep.media.first()).and_then(|m| m.parts.first());
            let audio: Vec<Stream> = ref_part.map(|p| p.audio_streams().cloned().collect()).unwrap_or_default();
            let subs: Vec<Stream> = ref_part.map(|p| p.subtitle_streams().cloned().collect()).unwrap_or_default();
            let audio_tpl = audio_choice().and_then(|id| audio.iter().find(|s| s.id == Some(id)));
            let sub_tpl = match sub_choice() {
                SubChoice::Keep => None,
                SubChoice::Off => Some(None),
                SubChoice::Track(id) => subs.iter().find(|s| s.id == Some(id)).map(Some),
            };
            let p = plan(&scope, audio_tpl, sub_tpl);
            let nothing_chosen = audio_tpl.is_none() && sub_tpl.is_none();
            let audio_name = audio_tpl.map(|s| s.short_label()).unwrap_or_default();
            let sub_name = match sub_tpl {
                Some(Some(s)) => s.short_label(),
                _ => String::new(),
            };
            let reference_rk = reference_ep.map(|ep| ep.rating_key().to_string()).unwrap_or_default();
            let scope_eps: Vec<(String, String)> = scope.iter().map(|ep| (ep.rating_key().to_string(), episode_label(ep))).collect();
            let changes: Vec<(i64, Option<i64>, Option<i64>)> = p.changes.iter().map(|c| (c.part_id, c.audio, c.subtitle)).collect();
            let apply_label = if p.episodes == 0 { "Apply".to_string() } else { format!("Apply to {} episode{}", p.episodes, if p.episodes == 1 { "" } else { "s" }) };

            rsx! {
                div { class: "track-grid",
                    div { class: "picker",
                        label { Icon { name: Ic::Layers, class: "sm" } "Apply to" }
                        select { class: "select", disabled: busy,
                            onchange: move |e| {
                                season.set(e.value().parse().ok());
                                reference.set(None);
                                audio_choice.set(None);
                                sub_choice.set(SubChoice::Keep);
                            },
                            option { value: "all", selected: season().is_none(), "All seasons ({all.len()} episodes)" }
                            for (idx, title, n) in seasons {
                                option { key: "{idx}", value: "{idx}", selected: season() == Some(idx), "{title} ({n} episodes)" }
                            }
                        }
                    }
                    div { class: "picker",
                        label { Icon { name: Ic::Tv, class: "sm" } "Take tracks from" }
                        select { class: "select", disabled: busy,
                            onchange: move |e| {
                                reference.set(Some(e.value()));
                                audio_choice.set(None);
                                sub_choice.set(SubChoice::Keep);
                            },
                            for (rk, label) in scope_eps {
                                option { key: "{rk}", value: "{rk}", selected: rk == reference_rk, "{label}" }
                            }
                        }
                    }
                    div { class: "picker",
                        label { Icon { name: Ic::Volume, class: "sm" } "Audio" }
                        select { class: "select", disabled: busy || audio.is_empty(),
                            onchange: move |e| audio_choice.set(e.value().parse().ok()),
                            option { value: "keep", selected: audio_choice().is_none(), "Don't change" }
                            for s in audio.iter() {
                                option { key: "{s.id:?}", value: "{s.id.unwrap_or_default()}", selected: audio_choice() == s.id, "{s.label()}" }
                            }
                        }
                    }
                    div { class: "picker",
                        label { Icon { name: Ic::Subtitles, class: "sm" } "Subtitles" }
                        select { class: "select", disabled: busy,
                            onchange: move |e| sub_choice.set(SubChoice::parse(&e.value())),
                            option { value: "keep", selected: sub_choice() == SubChoice::Keep, "Don't change" }
                            option { value: "off", selected: sub_choice() == SubChoice::Off, "None (turn off)" }
                            for s in subs.iter() {
                                option { key: "{s.id:?}", value: "{s.id.unwrap_or_default()}", selected: sub_choice() == SubChoice::Track(s.id.unwrap_or_default()),
                                    {format!("{}{}", s.label(), if s.is_external() { " · external" } else { "" })}
                                }
                            }
                        }
                    }
                }

                div { class: "track-summary",
                    if let Some((done, total)) = progress() {
                        div { class: "track-progress", i { style: "width: {percent(done, total):.1}%" } }
                        span { "Updating {done} of {total}…" }
                    } else if nothing_chosen {
                        span { class: "muted", "Choose an audio or subtitle track. Patina finds the closest match (same language, then title, codec and channels) in each episode." }
                    } else if p.episodes == 0 && p.no_audio.is_empty() && p.no_subtitle.is_empty() {
                        span { Icon { name: Ic::CheckCircle, class: "sm" } " Every episode already uses these tracks." }
                    } else {
                        span { "{p.episodes} of {scope.len()} episodes will change." }
                    }
                    if !busy && !p.no_audio.is_empty() {
                        Misses { what: format!("no audio matching “{audio_name}”"), episodes: p.no_audio.clone() }
                    }
                    if !busy && !p.no_subtitle.is_empty() {
                        Misses { what: format!("no subtitles matching “{sub_name}”"), episodes: p.no_subtitle.clone() }
                    }
                }

                div { class: "foot",
                    button { class: "btn ghost", disabled: busy, onclick: close, "Cancel" }
                    button { class: "btn primary", disabled: busy || changes.is_empty(),
                        onclick: move |_| apply(app, changes.clone(), progress),
                        Icon { name: Ic::Check, class: "sm" }
                        "{apply_label}"
                    }
                }
            }
        }
    };

    rsx! {
        div { class: "modal-backdrop", onclick: close,
            div { class: "modal wide", onclick: move |e| e.stop_propagation(),
                h2 { "Audio & subtitles" }
                p { class: "desc", "{show_title} — set tracks for many episodes at once." }
                {body}
            }
        }
    }
}

fn percent(done: usize, total: usize) -> f64 {
    done as f64 / total.max(1) as f64 * 100.0
}

#[component]
fn Misses(what: String, episodes: Vec<String>) -> Element {
    let head = match episodes.len() {
        1 => format!("1 episode has {what} — left unchanged"),
        n => format!("{n} episodes have {what} — left unchanged"),
    };
    rsx! {
        details { class: "track-misses",
            summary { Icon { name: Ic::Alert, class: "sm" } "{head}" }
            ul { for e in episodes { li { key: "{e}", "{e}" } } }
        }
    }
}

/// Save the changes to Plex a few at a time, reporting progress as they land.
fn apply(app: AppState, changes: Vec<(i64, Option<i64>, Option<i64>)>, mut progress: Signal<Option<(usize, usize)>>) {
    let total = changes.len();
    progress.set(Some((0, total)));
    spawn(async move {
        let server = app.server();
        let mut results = futures::stream::iter(changes)
            .map(|(part, audio, sub)| {
                let server = server.clone();
                async move { server.select_streams(part, audio, sub, false).await }
            })
            .buffer_unordered(6);
        let (mut done, mut failed, mut first_error) = (0, 0, None);
        while let Some(r) = results.next().await {
            done += 1;
            if let Err(e) = r {
                failed += 1;
                first_error.get_or_insert(e.to_string());
            }
            progress.set(Some((done, total)));
        }
        match first_error {
            None => app.toast(format!("Updated tracks for {total} file{}", if total == 1 { "" } else { "s" })),
            Some(e) => app.error(format!("{failed} of {total} updates failed: {e}")),
        }
        app.bump();
        let mut target = app.tracks_target;
        target.set(None);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use patina_plex::{Media, stream_type};

    fn stream(id: i64, kind: i64, lang: &str, selected: bool) -> Stream {
        Stream { id: Some(id), stream_type: Some(kind), language_code: Some(lang.into()), selected, index: Some(id), ..Default::default() }
    }

    fn episode(n: i64, streams: Vec<Stream>) -> Metadata {
        Metadata {
            index: Some(n),
            parent_index: Some(1),
            media: vec![Media { parts: vec![Part { id: Some(n), streams, ..Default::default() }], ..Default::default() }],
            ..Default::default()
        }
    }

    #[test]
    fn plans_only_what_changes() {
        let (a, s) = (stream_type::AUDIO, stream_type::SUBTITLE);
        let eps = [
            // Already on Japanese audio.
            episode(1, vec![stream(11, a, "eng", false), stream(12, a, "jpn", true), stream(13, s, "eng", true)]),
            // English selected; Japanese available.
            episode(2, vec![stream(21, a, "eng", true), stream(22, a, "jpn", false)]),
            // No Japanese at all.
            episode(3, vec![stream(31, a, "eng", true), stream(32, s, "eng", false)]),
        ];
        let refs: Vec<&Metadata> = eps.iter().collect();
        let tpl = stream(0, a, "jpn", false);

        let p = plan(&refs, Some(&tpl), Some(None));
        assert_eq!(p.changes, vec![
            PartChange { part_id: 1, audio: None, subtitle: Some(0) },
            PartChange { part_id: 2, audio: Some(22), subtitle: None },
        ]);
        assert_eq!(p.episodes, 2);
        assert_eq!(p.no_audio, vec![episode_label(&eps[2])]);

        let sub_tpl = stream(0, s, "eng", false);
        let p = plan(&refs, None, Some(Some(&sub_tpl)));
        assert_eq!(p.changes, vec![PartChange { part_id: 3, audio: None, subtitle: Some(32) }]);
        assert_eq!(p.no_subtitle, vec![episode_label(&eps[1])]);
    }
}
