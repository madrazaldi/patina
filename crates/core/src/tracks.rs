//! Set audio and subtitle tracks for a whole show or season at once (like
//! PASTA): pick tracks in one episode and the closest match is selected in
//! every other episode.
//!
//! The front end loads a show once ([`load`]), then asks for a fresh [`View`]
//! whenever a choice changes, and finally [`apply`]s the planned changes.

use std::sync::Arc;

use anyhow::{Result, anyhow};
use futures::StreamExt;
use patina_plex::{Metadata, Part, Stream};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

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

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartChange {
    part_id: i64,
    audio: Option<i64>,
    subtitle: Option<i64>,
}

/// What applying a choice would do across the episodes in scope.
#[derive(Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub changes: Vec<PartChange>,
    /// Episodes (with changes) that would be touched.
    pub episodes: usize,
    pub no_audio: Vec<String>,
    pub no_subtitle: Vec<String>,
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

/// Load every episode of a show (with streams) for the dialog.
pub async fn load(app: &AppState, show: &str) -> Result<()> {
    let episodes = app.server()?.all_leaves_with_streams(show).await?;
    *app.tracks.lock().unwrap() = Some((show.to_string(), Arc::new(episodes)));
    Ok(())
}

/// The dialog's current choices.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    pub show: String,
    /// Season index; `None` means every season.
    pub season: Option<i64>,
    /// Episode whose tracks are offered; defaults to the first in scope.
    pub reference: Option<String>,
    pub audio: Option<i64>,
    /// "keep", "off" or a stream id.
    pub subtitle: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Season {
    pub index: i64,
    pub title: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Episode {
    pub rating_key: String,
    pub label: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: i64,
    pub label: String,
    pub external: bool,
}

impl From<&Stream> for Track {
    fn from(s: &Stream) -> Self {
        Self { id: s.id.unwrap_or_default(), label: s.label(), external: s.is_external() }
    }
}

/// Everything the dialog renders for a query.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub total: usize,
    pub seasons: Vec<Season>,
    pub scope: Vec<Episode>,
    pub reference: Option<String>,
    pub audio: Vec<Track>,
    pub subtitles: Vec<Track>,
    pub audio_name: Option<String>,
    pub subtitle_name: Option<String>,
    pub nothing_chosen: bool,
    pub plan: Plan,
}

/// The episodes in scope for a query, the reference episode, its tracks and
/// the templates the choices name.
struct Chosen<'a> {
    scope: Vec<&'a Metadata>,
    reference: Option<&'a Metadata>,
    audio: Vec<&'a Stream>,
    subs: Vec<&'a Stream>,
    audio_tpl: Option<&'a Stream>,
    sub_tpl: Option<Option<&'a Stream>>,
}

fn choose<'a>(all: &'a [Metadata], q: &Query) -> Chosen<'a> {
    let scope: Vec<&Metadata> = all.iter().filter(|ep| q.season.is_none_or(|s| ep.parent_index == Some(s))).collect();
    let reference = q
        .reference
        .as_deref()
        .and_then(|rk| scope.iter().find(|ep| ep.rating_key() == rk).copied())
        .or(scope.first().copied());
    let ref_part = reference.and_then(|ep| ep.media.first()).and_then(|m| m.parts.first());
    let audio: Vec<&Stream> = ref_part.map(|p| p.audio_streams().collect()).unwrap_or_default();
    let subs: Vec<&Stream> = ref_part.map(|p| p.subtitle_streams().collect()).unwrap_or_default();
    let audio_tpl = q.audio.and_then(|id| audio.iter().find(|s| s.id == Some(id)).copied());
    let sub_tpl = match SubChoice::parse(&q.subtitle) {
        SubChoice::Keep => None,
        SubChoice::Off => Some(None),
        SubChoice::Track(id) => subs.iter().find(|s| s.id == Some(id)).map(|s| Some(*s)),
    };
    Chosen { scope, reference, audio, subs, audio_tpl, sub_tpl }
}

fn loaded(app: &AppState, show: &str) -> Result<Arc<Vec<Metadata>>> {
    match &*app.tracks.lock().unwrap() {
        Some((loaded, eps)) if loaded == show => Ok(eps.clone()),
        _ => Err(anyhow!("The episode list is not loaded.")),
    }
}

/// The streams a query names (copies), and the show's title; what the dialog's
/// "remember for this show" turns into playback rules.
#[allow(clippy::type_complexity)]
pub fn chosen(app: &AppState, q: &Query) -> Result<(Option<Stream>, Option<Option<Stream>>, String)> {
    let all = loaded(app, &q.show)?;
    let c = choose(&all, q);
    let title = all.first().and_then(|ep| ep.grandparent_title.clone()).unwrap_or_default();
    Ok((c.audio_tpl.cloned(), c.sub_tpl.map(|s| s.cloned()), title))
}

pub fn view(app: &AppState, q: &Query) -> Result<View> {
    let all = loaded(app, &q.show)?;

    // Seasons in library order, with episode counts.
    let mut seasons: Vec<Season> = Vec::new();
    for ep in all.iter() {
        let idx = ep.parent_index.unwrap_or_default();
        match seasons.iter_mut().find(|s| s.index == idx) {
            Some(s) => s.count += 1,
            None => seasons.push(Season {
                index: idx,
                title: ep.parent_title.clone().unwrap_or_else(|| format!("Season {idx}")),
                count: 1,
            }),
        }
    }
    let Chosen { scope, reference, audio, subs, audio_tpl, sub_tpl } = choose(&all, q);
    let plan = plan(&scope, audio_tpl, sub_tpl);

    Ok(View {
        total: all.len(),
        seasons,
        scope: scope.iter().map(|ep| Episode { rating_key: ep.rating_key().to_string(), label: episode_label(ep) }).collect(),
        reference: reference.map(|ep| ep.rating_key().to_string()),
        audio: audio.iter().map(|s| Track::from(*s)).collect(),
        subtitles: subs.iter().map(|s| Track::from(*s)).collect(),
        audio_name: audio_tpl.map(Stream::short_label),
        subtitle_name: sub_tpl.flatten().map(Stream::short_label),
        nothing_chosen: audio_tpl.is_none() && sub_tpl.is_none(),
        plan,
    })
}

/// Save the changes to Plex a few at a time, reporting `(done, total)` as they land.
pub async fn apply(app: &AppState, changes: Vec<PartChange>, progress: impl Fn(usize, usize)) -> Result<()> {
    let server = app.server()?;
    let total = changes.len();
    progress(0, total);
    let mut results = futures::stream::iter(changes)
        .map(|c| {
            let server = server.clone();
            async move { server.select_streams(c.part_id, c.audio, c.subtitle, false).await }
        })
        .buffer_unordered(6);
    let (mut done, mut failed, mut first_error) = (0, 0, None);
    while let Some(r) = results.next().await {
        done += 1;
        if let Err(e) = r {
            failed += 1;
            first_error.get_or_insert(e.to_string());
        }
        progress(done, total);
    }
    *app.tracks.lock().unwrap() = None;
    app.bump();
    match first_error {
        None => {
            app.toast(format!("Updated tracks for {total} file{}", if total == 1 { "" } else { "s" }));
            Ok(())
        }
        Some(e) => Err(anyhow!("{failed} of {total} updates failed: {e}")),
    }
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

    #[test]
    fn parses_subtitle_choice() {
        assert_eq!(SubChoice::parse("keep"), SubChoice::Keep);
        assert_eq!(SubChoice::parse("off"), SubChoice::Off);
        assert_eq!(SubChoice::parse("42"), SubChoice::Track(42));
    }
}
