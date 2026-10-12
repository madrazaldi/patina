//! Automatic playback preferences: which audio and subtitle tracks and which
//! version of a title to use, remembered per profile and per show.
//!
//! Rules are stored in `playback-prefs.json` next to the config file, scoped
//! by account and server (a profile on one server never affects another).
//! A show's rules override the profile's, field by field; a field a show
//! leaves unset inherits the profile's value.
//!
//! When playback starts the rules are matched against the streams of the file
//! ([`choose_streams`]) and the versions of the title ([`pick_version`]). Both
//! are pure functions, so they apply to newly added episodes too: nothing is
//! stored per episode.

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use patina_plex::{Media, Metadata, Part, PlexServer, Stream};
use serde::{Deserialize, Serialize};

use crate::state::AppState;
use crate::{config, storage};

// Rules ----------------------------------------------------------------------

/// When subtitles are turned on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SubtitleMode {
    /// Leave the subtitle track as Plex has it.
    #[default]
    Keep,
    Off,
    Always,
    /// Only for dialogue you can't follow: forced subtitles while the audio
    /// is in one of your languages, full subtitles when it is not.
    Foreign,
}

/// Which kind of subtitle track is wanted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ForcedMode {
    /// Full subtitles, forced ones when there are no others.
    #[default]
    Auto,
    /// Forced subtitles first, then full ones.
    Prefer,
    /// Never a forced track.
    Avoid,
    /// Only forced tracks.
    Only,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Resolution {
    #[default]
    #[serde(rename = "any")]
    Any,
    #[serde(rename = "4k")]
    Uhd,
    #[serde(rename = "1080")]
    Fhd,
    #[serde(rename = "720")]
    Hd,
    #[serde(rename = "sd")]
    Sd,
}

impl Resolution {
    /// 1 (SD) to 4 (4K); `None` for "any".
    fn rank(self) -> Option<i32> {
        match self {
            Self::Any => None,
            Self::Uhd => Some(4),
            Self::Fhd => Some(3),
            Self::Hd => Some(2),
            Self::Sd => Some(1),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Any => "Any",
            Self::Uhd => "4K",
            Self::Fhd => "1080p",
            Self::Hd => "720p",
            Self::Sd => "SD",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HdrMode {
    #[default]
    Any,
    Prefer,
    Avoid,
}

/// What a profile or show asks for. Every field is optional: `None` means
/// "not set" (a show then inherits the profile's value).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Rules {
    /// Languages in order of preference, as ISO codes or names ("eng", "ja", "French").
    pub audio_languages: Option<Vec<String>>,
    /// Subtitle languages; the audio languages when empty.
    pub subtitle_languages: Option<Vec<String>>,
    pub subtitles: Option<SubtitleMode>,
    pub forced: Option<ForcedMode>,
    /// Skip commentary and audio-description tracks.
    pub avoid_commentary: Option<bool>,
    pub resolution: Option<Resolution>,
    /// Preferred video codec ("hevc", "h264", "av1", ...); empty for any.
    pub codec: Option<String>,
    pub hdr: Option<HdrMode>,
}

impl Rules {
    pub fn is_empty(&self) -> bool {
        *self == Rules::default()
    }

    /// These rules, with `base` filling every field this leaves unset.
    pub fn over(&self, base: &Rules) -> Rules {
        Rules {
            audio_languages: self.audio_languages.clone().or_else(|| base.audio_languages.clone()),
            subtitle_languages: self.subtitle_languages.clone().or_else(|| base.subtitle_languages.clone()),
            subtitles: self.subtitles.or(base.subtitles),
            forced: self.forced.or(base.forced),
            avoid_commentary: self.avoid_commentary.or(base.avoid_commentary),
            resolution: self.resolution.or(base.resolution),
            codec: self.codec.clone().or_else(|| base.codec.clone()),
            hdr: self.hdr.or(base.hdr),
        }
    }

    pub fn resolve(&self) -> Resolved {
        let langs = |l: &Option<Vec<String>>| {
            let mut out: Vec<String> = Vec::new();
            for key in l.iter().flatten().map(|s| language_key(s)).filter(|k| !k.is_empty()) {
                if !out.contains(&key) {
                    out.push(key);
                }
            }
            out
        };
        Resolved {
            audio: langs(&self.audio_languages),
            subtitle_langs: langs(&self.subtitle_languages),
            subtitles: self.subtitles.unwrap_or_default(),
            forced: self.forced.unwrap_or_default(),
            avoid_commentary: self.avoid_commentary.unwrap_or(false),
            resolution: self.resolution.unwrap_or_default(),
            codec: self.codec.as_deref().map(codec_key).filter(|c| !c.is_empty()),
            hdr: self.hdr.unwrap_or_default(),
        }
    }
}

/// [`Rules`] with defaults filled in and languages normalised, ready to match.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    audio: Vec<String>,
    subtitle_langs: Vec<String>,
    subtitles: SubtitleMode,
    forced: ForcedMode,
    avoid_commentary: bool,
    resolution: Resolution,
    codec: Option<String>,
    hdr: HdrMode,
}

impl Resolved {
    fn has_track_rules(&self) -> bool {
        !self.audio.is_empty() || self.avoid_commentary || self.subtitles != SubtitleMode::Keep
    }

    fn has_version_rules(&self) -> bool {
        self.resolution != Resolution::Any || self.codec.is_some() || self.hdr != HdrMode::Any
    }

    /// Languages subtitles are wanted in.
    fn sub_langs(&self) -> &[String] {
        if self.subtitle_langs.is_empty() { &self.audio } else { &self.subtitle_langs }
    }

    /// Languages that need no subtitles: the audio ones, else the subtitle ones.
    fn native(&self) -> &[String] {
        if self.audio.is_empty() { &self.subtitle_langs } else { &self.audio }
    }
}

// Languages --------------------------------------------------------------------

/// Names a language goes by; the first is the canonical ISO 639-2 code.
const LANGUAGES: &[&[&str]] = &[
    &["eng", "en", "english"],
    &["jpn", "ja", "japanese"],
    &["fra", "fre", "fr", "french"],
    &["deu", "ger", "de", "german"],
    &["spa", "es", "spanish"],
    &["ita", "it", "italian"],
    &["por", "pt", "portuguese"],
    &["rus", "ru", "russian"],
    &["zho", "chi", "zh", "chinese", "mandarin", "cantonese"],
    &["kor", "ko", "korean"],
    &["nld", "dut", "nl", "dutch"],
    &["swe", "sv", "swedish"],
    &["nor", "nob", "nb", "no", "norwegian"],
    &["dan", "da", "danish"],
    &["fin", "fi", "finnish"],
    &["pol", "pl", "polish"],
    &["tur", "tr", "turkish"],
    &["ara", "ar", "arabic"],
    &["heb", "he", "hebrew"],
    &["hin", "hi", "hindi"],
    &["tha", "th", "thai"],
    &["ces", "cze", "cs", "czech"],
    &["hun", "hu", "hungarian"],
    &["ell", "gre", "el", "greek"],
    &["ukr", "uk", "ukrainian"],
    &["ind", "id", "indonesian"],
    &["vie", "vi", "vietnamese"],
    &["ron", "rum", "ro", "romanian"],
    &["bul", "bg", "bulgarian"],
    &["hrv", "hr", "croatian"],
    &["cat", "ca", "catalan"],
    &["fas", "per", "fa", "persian"],
    &["msa", "may", "ms", "malay"],
    &["tam", "ta", "tamil"],
    &["lat", "la", "latin"],
];

/// A language as a comparable key: "en", "eng", "English" and "en-US" are all "eng".
/// Unknown languages are compared by their lowercase name.
pub fn language_key(name: &str) -> String {
    let lower = name.trim().to_ascii_lowercase();
    let base = lower.split([' ', '-', '_', '(']).find(|s| !s.is_empty()).unwrap_or("");
    if matches!(base, "und" | "zxx" | "mis" | "mul" | "unknown") {
        return String::new();
    }
    LANGUAGES.iter().find(|names| names.contains(&base)).map_or_else(|| base.to_string(), |names| names[0].to_string())
}

fn stream_lang(s: &Stream) -> Option<String> {
    [&s.language_code, &s.language_tag, &s.language]
        .into_iter()
        .flatten()
        .map(|l| language_key(l))
        .find(|k| !k.is_empty())
}

fn titles(s: &Stream) -> String {
    [&s.title, &s.display_title, &s.extended_display_title].into_iter().flatten().map(|t| t.to_lowercase()).collect::<Vec<_>>().join(" | ")
}

/// Commentary and audio-description tracks, spotted by their titles.
pub fn is_commentary(s: &Stream) -> bool {
    let t = titles(s);
    ["commentary", "commentaire", "comentario", "kommentar", "audio description", "audio-description", "described video", "descriptive", "isolated score"]
        .iter()
        .any(|k| t.contains(k))
}

/// Forced-subtitle tracks, by flag or by title.
pub fn is_forced(s: &Stream) -> bool {
    s.forced || titles(s).contains("forced")
}

// Choosing tracks ------------------------------------------------------------

/// The subtitle outcome of a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubPick {
    Off,
    Track(i64),
}

/// The streams rules ask for; `None` leaves Plex's selection alone.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Choice {
    pub audio: Option<i64>,
    pub subtitle: Option<SubPick>,
}

/// The stream to keep: the selected one, else the default one, else the first.
fn best<'a>(list: &[&'a Stream]) -> Option<&'a Stream> {
    list.iter()
        .copied()
        .min_by_key(|s| (!s.selected, !s.default, s.index.or(s.id).unwrap_or(i64::MAX)))
}

fn pick_audio<'a>(r: &Resolved, part: &'a Part) -> Option<&'a Stream> {
    let all: Vec<&Stream> = part.audio_streams().collect();
    let plain: Vec<&Stream> = all.iter().copied().filter(|s| !(r.avoid_commentary && is_commentary(s))).collect();
    let pool = if plain.is_empty() { &all } else { &plain };
    for lang in &r.audio {
        let hits: Vec<&Stream> = pool.iter().copied().filter(|s| stream_lang(s).as_deref() == Some(lang.as_str())).collect();
        if let Some(s) = best(&hits) {
            return Some(s);
        }
    }
    // No preferred language is there; at least step off a commentary track.
    let current = part.audio_streams().find(|s| s.selected);
    if r.avoid_commentary && current.is_some_and(is_commentary) && !plain.is_empty() {
        return best(&plain);
    }
    None
}

/// Best subtitle track of `lang` of the wanted kind, preferring the selected
/// one, then non-SDH tracks.
fn pick_kind<'a>(subs: &[&'a Stream], lang: &str, forced: bool) -> Option<&'a Stream> {
    subs.iter()
        .copied()
        .filter(|s| stream_lang(s).as_deref() == Some(lang) && is_forced(s) == forced)
        .min_by_key(|s| (!s.selected, s.hearing_impaired, !s.default, s.index.or(s.id).unwrap_or(i64::MAX)))
}

fn pick_full_subtitle<'a>(r: &Resolved, subs: &[&'a Stream]) -> Option<&'a Stream> {
    let plain: Vec<&Stream> = subs.iter().copied().filter(|s| !(r.avoid_commentary && is_commentary(s))).collect();
    let pool = if plain.is_empty() { subs } else { &plain[..] };
    let order: &[bool] = match r.forced {
        ForcedMode::Auto => &[false, true],
        ForcedMode::Prefer => &[true, false],
        ForcedMode::Avoid => &[false],
        ForcedMode::Only => &[true],
    };
    for lang in r.sub_langs() {
        for forced in order {
            if let Some(s) = pick_kind(pool, lang, *forced) {
                return Some(s);
            }
        }
    }
    None
}

fn pick_subtitle(r: &Resolved, part: &Part, audio: Option<&Stream>) -> Option<SubPick> {
    let subs: Vec<&Stream> = part.subtitle_streams().collect();
    match r.subtitles {
        SubtitleMode::Keep => None,
        SubtitleMode::Off => Some(SubPick::Off),
        SubtitleMode::Always => pick_full_subtitle(r, &subs).map(|s| SubPick::Track(s.id.unwrap_or_default())),
        SubtitleMode::Foreign => {
            let audio_lang = audio.and_then(stream_lang);
            let native = audio_lang.is_some_and(|l| r.native().contains(&l));
            if native {
                // Only the foreign dialogue: a forced track in a subtitle language, else nothing.
                let forced = if r.forced == ForcedMode::Avoid {
                    None
                } else {
                    r.sub_langs().iter().find_map(|lang| pick_kind(&subs, lang, true))
                };
                Some(forced.map_or(SubPick::Off, |s| SubPick::Track(s.id.unwrap_or_default())))
            } else {
                pick_full_subtitle(r, &subs).map(|s| SubPick::Track(s.id.unwrap_or_default()))
            }
        }
    }
}

/// The audio and subtitle streams `rules` ask for in `part`. The choice may
/// name streams that are already selected.
pub fn choose_streams(r: &Resolved, part: &Part) -> Choice {
    if !r.has_track_rules() {
        return Choice::default();
    }
    let audio = pick_audio(r, part);
    // Subtitles follow the audio that will actually play.
    let playing = audio.or_else(|| part.selected_audio());
    Choice { audio: audio.and_then(|s| s.id), subtitle: pick_subtitle(r, part, playing) }
}

/// Select `choice` in `part`. Returns the stream ids that changed, as Plex
/// takes them (`Some(0)` for subtitles off).
pub fn apply_choice(part: &mut Part, choice: &Choice) -> (Option<i64>, Option<i64>) {
    let mut changed = (None, None);
    if let Some(id) = choice.audio
        && part.audio_streams().any(|s| s.id == Some(id) && !s.selected)
    {
        for s in part.streams.iter_mut().filter(|s| s.stream_type == Some(patina_plex::stream_type::AUDIO)) {
            s.selected = s.id == Some(id);
        }
        changed.0 = Some(id);
    }
    match choice.subtitle {
        Some(SubPick::Off) if part.selected_subtitle().is_some() => {
            for s in part.streams.iter_mut().filter(|s| s.stream_type == Some(patina_plex::stream_type::SUBTITLE)) {
                s.selected = false;
            }
            changed.1 = Some(0);
        }
        Some(SubPick::Track(id)) if part.subtitle_streams().any(|s| s.id == Some(id) && !s.selected) => {
            for s in part.streams.iter_mut().filter(|s| s.stream_type == Some(patina_plex::stream_type::SUBTITLE)) {
                s.selected = s.id == Some(id);
            }
            changed.1 = Some(id);
        }
        _ => {}
    }
    changed
}

// Choosing versions ----------------------------------------------------------

fn codec_key(codec: &str) -> String {
    match codec.trim().to_ascii_lowercase().as_str() {
        "hevc" | "h265" | "h.265" | "x265" => "hevc".into(),
        "h264" | "h.264" | "avc" | "x264" | "avc1" => "h264".into(),
        other => other.into(),
    }
}

fn media_resolution(m: &Media) -> Option<i32> {
    if let Some(r) = m.video_resolution.as_deref() {
        match r.to_ascii_lowercase().as_str() {
            "4k" | "2160" => return Some(4),
            "1080" => return Some(3),
            "720" => return Some(2),
            "576" | "480" | "sd" => return Some(1),
            _ => {}
        }
    }
    m.height.or_else(|| m.parts.first().and_then(|p| p.video_stream()).and_then(|v| v.height)).map(|h| match h {
        2000.. => 4,
        1000..=1999 => 3,
        700..=999 => 2,
        _ => 1,
    })
}

fn media_is_hdr(m: &Media) -> bool {
    if m.parts.first().and_then(|p| p.video_stream()).is_some_and(|v| v.hdr_label().is_some()) {
        return true;
    }
    let name = [&m.title, &m.edition_title].into_iter().flatten().map(|t| t.to_lowercase()).collect::<Vec<_>>().join(" ");
    ["hdr", "dolby vision", "hlg"].iter().any(|k| name.contains(k)) || name.split(|c: char| !c.is_alphanumeric()).any(|w| w == "dv")
}

/// The index of the version of a title that best fits the rules; 0 (Plex's
/// first) when the rules have no version preference or nothing fits better.
pub fn pick_version(r: &Resolved, versions: &[Media]) -> usize {
    if versions.len() < 2 || !r.has_version_rules() {
        return 0;
    }
    let want = r.resolution.rank();
    let score = |m: &Media| {
        let mut s = 0;
        if let Some(want) = want {
            s += match media_resolution(m) {
                // Closest wins; at equal distance the higher resolution does.
                Some(have) => (have - want).abs() * 100 + i32::from(have < want),
                None => 150,
            };
        }
        if let Some(codec) = &r.codec
            && m.video_codec.as_deref().map(codec_key).as_deref() != Some(codec.as_str())
        {
            s += 20;
        }
        match (r.hdr, media_is_hdr(m)) {
            (HdrMode::Prefer, false) | (HdrMode::Avoid, true) => s += 30,
            _ => {}
        }
        s
    };
    versions.iter().enumerate().min_by_key(|(i, m)| (score(m), *i)).map_or(0, |(i, _)| i)
}

// Storage ----------------------------------------------------------------------

/// The account and server a rule belongs to.
pub use crate::recovery::Scope;

/// The connected account and server.
pub fn current_scope(app: &AppState) -> Option<Scope> {
    app.session().as_ref().map(Scope::of)
}

/// The scope whose rules apply to `item`: its own server's when it was found on another one.
pub fn scope_for(app: &AppState, item: &Metadata) -> Option<Scope> {
    let mut scope = current_scope(app)?;
    if let Some(id) = item.server_id() {
        scope.server = id.to_string();
    }
    Some(scope)
}

/// One saved set of rules: a profile's (`show` is `None`) or a show's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub account: String,
    pub server: String,
    /// Rating key of the show; `None` for the profile's rules.
    pub show: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    pub rules: Rules,
}

fn path() -> PathBuf {
    config::config_dir().join("playback-prefs.json")
}

fn load() -> Vec<Entry> {
    storage::read_json(&path()).unwrap_or_else(|e| {
        tracing::error!("playback preferences: {e:#}");
        Vec::new()
    })
}

fn in_scope<'a>(entries: &'a [Entry], scope: &'a Scope) -> impl Iterator<Item = &'a Entry> {
    entries.iter().filter(move |e| e.account == scope.account && e.server == scope.server)
}

fn find<'a>(entries: &'a [Entry], scope: &Scope, show: Option<&str>) -> Option<&'a Entry> {
    entries.iter().find(|e| e.account == scope.account && e.server == scope.server && e.show.as_deref() == show)
}

/// What applies to `show` (or to the profile alone): the show's rules over the profile's.
fn effective(entries: &[Entry], scope: &Scope, show: Option<&str>) -> Rules {
    let profile = find(entries, scope, None).map(|e| e.rules.clone()).unwrap_or_default();
    match show.and_then(|s| find(entries, scope, Some(s))) {
        Some(e) => e.rules.over(&profile),
        None => profile,
    }
}

fn store(entries: &mut Vec<Entry>, scope: &Scope, show: Option<&str>, title: Option<&str>, rules: Rules) {
    entries.retain(|e| !(e.account == scope.account && e.server == scope.server && e.show.as_deref() == show));
    if !rules.is_empty() {
        entries.push(Entry {
            account: scope.account.clone(),
            server: scope.server.clone(),
            show: show.map(str::to_string),
            title: title.map(str::to_string),
            rules,
        });
    }
}

/// The show an item belongs to, for per-show rules.
fn show_of(item: &Metadata) -> Option<&str> {
    match item.kind() {
        "episode" => item.grandparent_rating_key.as_deref(),
        "season" => item.parent_rating_key.as_deref(),
        "show" => item.rating_key.as_deref(),
        _ => None,
    }
}

/// Everything a front end shows about one profile or show.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    /// The profile's own rules.
    pub profile: Rules,
    /// The show's own rules, when a show was asked for.
    pub show: Option<Rules>,
    /// What applies: the show's rules over the profile's.
    pub effective: Rules,
}

/// A show with rules of its own.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowRules {
    pub rating_key: String,
    pub title: String,
    pub rules: Rules,
}

fn scope(app: &AppState) -> Result<Scope> {
    current_scope(app).ok_or_else(|| anyhow!("Not connected to a server."))
}

pub fn view(app: &AppState, show: Option<&str>) -> Result<View> {
    let scope = scope(app)?;
    let entries = load();
    Ok(View {
        profile: find(&entries, &scope, None).map(|e| e.rules.clone()).unwrap_or_default(),
        show: show.map(|s| find(&entries, &scope, Some(s)).map(|e| e.rules.clone()).unwrap_or_default()),
        effective: effective(&entries, &scope, show),
    })
}

/// The shows of this profile that have rules of their own.
pub fn shows(app: &AppState) -> Result<Vec<ShowRules>> {
    let scope = scope(app)?;
    let entries = load();
    let mut out: Vec<ShowRules> = in_scope(&entries, &scope)
        .filter_map(|e| {
            Some(ShowRules { rating_key: e.show.clone()?, title: e.title.clone().unwrap_or_else(|| "Show".into()), rules: e.rules.clone() })
        })
        .collect();
    out.sort_by_key(|s| s.title.to_lowercase());
    Ok(out)
}

/// Replace the rules of the profile (`show` is `None`) or of a show. Empty
/// rules remove the entry, so the show follows the profile again.
pub fn save(app: &AppState, show: Option<&str>, title: Option<&str>, rules: Rules) -> Result<()> {
    let scope = scope(app)?;
    storage::update_json::<Vec<Entry>, _>(&path(), |entries| store(entries, &scope, show, title, rules))
}

pub fn clear(app: &AppState, show: Option<&str>) -> Result<()> {
    save(app, show, None, Rules::default())
}

/// What a person chose in the whole-show track dialog, as rules: the audio
/// language, and subtitles off, or in a language (forced or full as picked).
pub fn rules_from_tracks(audio: Option<&Stream>, subtitle: Option<Option<&Stream>>) -> Rules {
    let mut rules = Rules::default();
    if let Some(lang) = audio.and_then(stream_lang) {
        rules.audio_languages = Some(vec![lang]);
        rules.avoid_commentary = Some(!audio.is_some_and(is_commentary));
    }
    match subtitle {
        None => {}
        Some(None) => rules.subtitles = Some(SubtitleMode::Off),
        Some(Some(s)) => {
            if let Some(lang) = stream_lang(s) {
                rules.subtitle_languages = Some(vec![lang]);
            }
            let forced = is_forced(s);
            rules.subtitles = Some(SubtitleMode::Always);
            rules.forced = Some(if forced { ForcedMode::Only } else { ForcedMode::Avoid });
        }
    }
    rules
}

/// Remember the choices of the track dialog as this show's rules.
pub fn remember_tracks(app: &AppState, q: &crate::tracks::Query) -> Result<()> {
    let (audio, subtitle, title) = crate::tracks::chosen(app, q)?;
    let rules = rules_from_tracks(audio.as_ref(), subtitle.as_ref().map(Option::as_ref));
    anyhow::ensure!(!rules.is_empty(), "Choose an audio or subtitle track to remember.");
    // Keep the show's other rules (version preferences) while replacing its tracks.
    let current = view(app, Some(&q.show))?.show.unwrap_or_default();
    let merged = Rules {
        audio_languages: rules.audio_languages.clone().or(current.audio_languages),
        subtitle_languages: if rules.subtitles.is_some() { rules.subtitle_languages.clone() } else { current.subtitle_languages },
        subtitles: rules.subtitles.or(current.subtitles),
        forced: if rules.subtitles.is_some() { rules.forced } else { current.forced },
        avoid_commentary: rules.avoid_commentary.or(current.avoid_commentary),
        resolution: current.resolution,
        codec: current.codec,
        hdr: current.hdr,
    };
    save(app, Some(&q.show), Some(&title), merged)?;
    app.toast(format!("Remembered these tracks for {title}"));
    Ok(())
}

// Hooks used by playback -------------------------------------------------------

/// The version of `item` to play: an explicit choice (`requested`, including
/// the first version) always stands; otherwise the pick of the rules of `scope`
/// (the account and the server the item plays from).
pub fn media_index(scope: Option<&Scope>, item: &Metadata, requested: Option<usize>) -> usize {
    if let Some(index) = requested {
        return index;
    }
    if item.media.len() < 2 || !matches!(item.kind(), "movie" | "episode") {
        return 0;
    }
    let Some(scope) = scope else { return 0 };
    pick_version(&effective(&load(), scope, show_of(item)).resolve(), &item.media)
}

/// Which version of `item` the rules prefer (for pre-selecting it in a picker).
pub fn preferred_version(app: &AppState, item: &Metadata) -> usize {
    media_index(scope_for(app, item).as_ref(), item, None)
}

/// Apply the rules to the streams of an item that is about to play: select the
/// tracks in `item` (so mpv is given them) and save them to Plex in the
/// background so other apps agree. A resumed item keeps the tracks it had.
/// `scope` is the account and the server the item plays from.
pub fn apply_auto(app: &AppState, scope: Option<&Scope>, server: &PlexServer, item: &mut Metadata, media_index: usize) {
    if !matches!(item.kind(), "movie" | "episode") || item.view_offset.unwrap_or(0) > 0 {
        return;
    }
    let Some(scope) = scope else { return };
    let rules = effective(&load(), scope, show_of(item)).resolve();
    if !rules.has_track_rules() {
        return;
    }
    let index = if media_index < item.media.len() { media_index } else { 0 };
    let Some(part) = item.media.get_mut(index).and_then(|m| m.parts.first_mut()) else { return };
    let Some(part_id) = part.id else { return };
    let choice = choose_streams(&rules, part);
    let (audio, subtitle) = apply_choice(part, &choice);
    if audio.is_some() || subtitle.is_some() {
        let server = server.clone();
        app.spawn(async move {
            if let Err(e) = server.select_streams(part_id, audio, subtitle, false).await {
                tracing::debug!("could not save automatic track choice: {e}");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use patina_plex::stream_type::{AUDIO, SUBTITLE, VIDEO};

    fn stream(id: i64, kind: i64, lang: &str, title: &str) -> Stream {
        Stream {
            id: Some(id),
            stream_type: Some(kind),
            language_code: Some(lang.into()),
            title: Some(title.to_string()).filter(|t| !t.is_empty()),
            index: Some(id),
            ..Default::default()
        }
    }
    fn selected(mut s: Stream) -> Stream {
        s.selected = true;
        s
    }
    fn forced(mut s: Stream) -> Stream {
        s.forced = true;
        s
    }
    fn part(streams: Vec<Stream>) -> Part {
        Part { id: Some(1), streams, ..Default::default() }
    }
    fn rules(f: impl FnOnce(&mut Rules)) -> Resolved {
        let mut r = Rules::default();
        f(&mut r);
        r.resolve()
    }
    fn langs(l: &[&str]) -> Option<Vec<String>> {
        Some(l.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn languages_match_by_code_tag_or_name() {
        assert_eq!(language_key("en"), "eng");
        assert_eq!(language_key("eng"), "eng");
        assert_eq!(language_key("English"), "eng");
        assert_eq!(language_key("en-US"), "eng");
        assert_eq!(language_key("pt-BR"), "por");
        assert_eq!(language_key("Portuguese (Brazil)"), "por");
        assert_eq!(language_key("fre"), language_key("fra"));
        assert_eq!(language_key("und"), "");
        assert_eq!(language_key("Klingon"), "klingon");
        let mut tagged = stream(1, AUDIO, "", "");
        tagged.language_code = None;
        tagged.language_tag = Some("ja".into());
        assert_eq!(stream_lang(&tagged).as_deref(), Some("jpn"));
    }

    #[test]
    fn commentary_is_found_by_title() {
        assert!(is_commentary(&stream(1, AUDIO, "eng", "Director's Commentary")));
        assert!(is_commentary(&stream(1, AUDIO, "eng", "English Audio Description")));
        assert!(is_commentary(&stream(1, SUBTITLE, "eng", "Commentary by the cast")));
        assert!(!is_commentary(&stream(1, AUDIO, "eng", "English (AC3 5.1)")));
        let mut by_display = stream(1, AUDIO, "eng", "");
        by_display.extended_display_title = Some("English (Commentary) (AC3 2.0)".into());
        assert!(is_commentary(&by_display));
    }

    #[test]
    fn picks_audio_by_language_order() {
        let p = part(vec![selected(stream(1, AUDIO, "eng", "")), stream(2, AUDIO, "jpn", ""), stream(3, AUDIO, "fra", "")]);
        let r = rules(|r| r.audio_languages = langs(&["ja", "en"]));
        assert_eq!(choose_streams(&r, &p).audio, Some(2));
        let r = rules(|r| r.audio_languages = langs(&["de", "French"]));
        assert_eq!(choose_streams(&r, &p).audio, Some(3));
        // None of the languages is there: Plex's choice stands.
        let r = rules(|r| r.audio_languages = langs(&["ko"]));
        assert_eq!(choose_streams(&r, &p), Choice::default());
    }

    #[test]
    fn avoids_commentary_but_not_when_it_is_all_there_is() {
        let p = part(vec![selected(stream(1, AUDIO, "eng", "Commentary")), stream(2, AUDIO, "eng", "")]);
        let r = rules(|r| {
            r.audio_languages = langs(&["eng"]);
            r.avoid_commentary = Some(true);
        });
        assert_eq!(choose_streams(&r, &p).audio, Some(2));
        // Without the rule the selected track stays.
        let r = rules(|r| r.audio_languages = langs(&["eng"]));
        assert_eq!(choose_streams(&r, &p).audio, Some(1));
        // Only commentary exists in the language: use it.
        let only = part(vec![stream(1, AUDIO, "eng", "Commentary")]);
        let r = rules(|r| {
            r.audio_languages = langs(&["eng"]);
            r.avoid_commentary = Some(true);
        });
        assert_eq!(choose_streams(&r, &only).audio, Some(1));
        // No language rule: still step off commentary.
        let r = rules(|r| r.avoid_commentary = Some(true));
        assert_eq!(choose_streams(&r, &p).audio, Some(2));
    }

    #[test]
    fn keeps_the_selected_track_among_equals() {
        let p = part(vec![stream(1, AUDIO, "eng", "AAC"), selected(stream(2, AUDIO, "eng", "AC3"))]);
        let r = rules(|r| r.audio_languages = langs(&["eng"]));
        assert_eq!(choose_streams(&r, &p).audio, Some(2));
    }

    #[test]
    fn subtitle_modes() {
        let p = part(vec![
            selected(stream(1, AUDIO, "jpn", "")),
            stream(2, SUBTITLE, "eng", ""),
            forced(stream(3, SUBTITLE, "eng", "Forced")),
            selected(stream(4, SUBTITLE, "fra", "")),
        ]);
        let set = |mode, forced| {
            rules(|r| {
                r.subtitle_languages = langs(&["eng"]);
                r.subtitles = Some(mode);
                r.forced = forced;
            })
        };
        assert_eq!(choose_streams(&set(SubtitleMode::Keep, None), &p).subtitle, None);
        assert_eq!(choose_streams(&set(SubtitleMode::Off, None), &p).subtitle, Some(SubPick::Off));
        assert_eq!(choose_streams(&set(SubtitleMode::Always, None), &p).subtitle, Some(SubPick::Track(2)));
        assert_eq!(choose_streams(&set(SubtitleMode::Always, Some(ForcedMode::Prefer)), &p).subtitle, Some(SubPick::Track(3)));
        assert_eq!(choose_streams(&set(SubtitleMode::Always, Some(ForcedMode::Only)), &p).subtitle, Some(SubPick::Track(3)));
        assert_eq!(choose_streams(&set(SubtitleMode::Always, Some(ForcedMode::Avoid)), &p).subtitle, Some(SubPick::Track(2)));
        // A language with nothing in it leaves the selection alone.
        let none = rules(|r| {
            r.subtitle_languages = langs(&["ko"]);
            r.subtitles = Some(SubtitleMode::Always);
        });
        assert_eq!(choose_streams(&none, &p).subtitle, None);
    }

    #[test]
    fn forced_subtitles_fall_back_to_full_ones_only_in_auto() {
        let p = part(vec![stream(1, AUDIO, "eng", ""), forced(stream(2, SUBTITLE, "eng", ""))]);
        let make = |mode| {
            rules(|r| {
                r.subtitle_languages = langs(&["eng"]);
                r.subtitles = Some(SubtitleMode::Always);
                r.forced = Some(mode);
            })
        };
        assert_eq!(choose_streams(&make(ForcedMode::Auto), &p).subtitle, Some(SubPick::Track(2)));
        assert_eq!(choose_streams(&make(ForcedMode::Avoid), &p).subtitle, None);
        // A track titled "Forced" counts even without the flag.
        let titled = part(vec![stream(1, SUBTITLE, "eng", "English Forced"), stream(2, SUBTITLE, "eng", "English")]);
        assert_eq!(choose_streams(&make(ForcedMode::Only), &titled).subtitle, Some(SubPick::Track(1)));
        assert_eq!(choose_streams(&make(ForcedMode::Avoid), &titled).subtitle, Some(SubPick::Track(2)));
    }

    #[test]
    fn foreign_mode_follows_the_audio_language() {
        let streams = |audio: &str| {
            vec![
                selected(stream(1, AUDIO, audio, "")),
                stream(2, SUBTITLE, "eng", ""),
                forced(stream(3, SUBTITLE, "eng", "")),
            ]
        };
        let r = rules(|r| {
            r.audio_languages = langs(&["eng"]);
            r.subtitles = Some(SubtitleMode::Foreign);
        });
        // English audio: only the forced track (foreign dialogue).
        assert_eq!(choose_streams(&r, &part(streams("eng"))).subtitle, Some(SubPick::Track(3)));
        // Japanese audio: full English subtitles.
        assert_eq!(choose_streams(&r, &part(streams("jpn"))).subtitle, Some(SubPick::Track(2)));
        // English audio and no forced track: subtitles off.
        let bare = part(vec![selected(stream(1, AUDIO, "eng", "")), selected(stream(2, SUBTITLE, "eng", ""))]);
        assert_eq!(choose_streams(&r, &bare).subtitle, Some(SubPick::Off));
        // The audio the rules pick is the one that counts: Japanese is chosen and is "native" for
        // this profile, so only its forced track is shown (English audio would have meant full subtitles).
        let both = part(vec![selected(stream(1, AUDIO, "eng", "")), stream(4, AUDIO, "jpn", ""), stream(2, SUBTITLE, "eng", ""), forced(stream(3, SUBTITLE, "eng", ""))]);
        let jp = rules(|r| {
            r.audio_languages = langs(&["jpn"]);
            r.subtitle_languages = langs(&["eng"]);
            r.subtitles = Some(SubtitleMode::Foreign);
        });
        let c = choose_streams(&jp, &both);
        assert_eq!((c.audio, c.subtitle), (Some(4), Some(SubPick::Track(3))));
    }

    #[test]
    fn applying_a_choice_changes_only_what_differs() {
        let mut p = part(vec![
            selected(stream(1, AUDIO, "eng", "")),
            stream(2, AUDIO, "jpn", ""),
            selected(stream(3, SUBTITLE, "eng", "")),
            stream(4, SUBTITLE, "fra", ""),
        ]);
        let changed = apply_choice(&mut p, &Choice { audio: Some(2), subtitle: Some(SubPick::Off) });
        assert_eq!(changed, (Some(2), Some(0)));
        assert_eq!(p.selected_audio().and_then(|s| s.id), Some(2));
        assert!(p.selected_subtitle().is_none());
        // Already as asked: nothing to save.
        assert_eq!(apply_choice(&mut p, &Choice { audio: Some(2), subtitle: Some(SubPick::Off) }), (None, None));
        assert_eq!(apply_choice(&mut p, &Choice { audio: None, subtitle: Some(SubPick::Track(4)) }), (None, Some(4)));
        assert_eq!(p.selected_subtitle().and_then(|s| s.id), Some(4));
    }

    fn version(res: &str, codec: &str, hdr: bool) -> Media {
        let mut video = Stream { stream_type: Some(VIDEO), ..Default::default() };
        if hdr {
            video.color_trc = Some("smpte2084".into());
        }
        Media {
            video_resolution: Some(res.into()),
            video_codec: Some(codec.into()),
            parts: vec![Part { streams: vec![video], ..Default::default() }],
            ..Default::default()
        }
    }

    #[test]
    fn ranks_versions() {
        let versions = [version("1080", "h264", false), version("4k", "hevc", true), version("720", "h264", false)];
        let pick = |f: fn(&mut Rules)| {
            let mut r = Rules::default();
            f(&mut r);
            pick_version(&r.resolve(), &versions)
        };
        assert_eq!(pick(|_| {}), 0, "no preference keeps Plex's order");
        assert_eq!(pick(|r| r.resolution = Some(Resolution::Uhd)), 1);
        assert_eq!(pick(|r| r.resolution = Some(Resolution::Fhd)), 0);
        assert_eq!(pick(|r| r.resolution = Some(Resolution::Hd)), 2);
        assert_eq!(pick(|r| r.codec = Some("H.265".into())), 1);
        assert_eq!(pick(|r| r.hdr = Some(HdrMode::Prefer)), 1);
        assert_eq!(pick(|r| r.hdr = Some(HdrMode::Avoid)), 0);
        // 4K wanted, but HDR avoided: resolution weighs more.
        assert_eq!(
            pick(|r| {
                r.resolution = Some(Resolution::Uhd);
                r.hdr = Some(HdrMode::Avoid);
            }),
            1
        );
        // 1080p equidistant from 4K and 720p: the higher one wins.
        let two = [version("720", "h264", false), version("4k", "hevc", false)];
        let fhd = Rules { resolution: Some(Resolution::Fhd), ..Default::default() }.resolve();
        assert_eq!(pick_version(&fhd, &two), 1);
        // A single version is never "changed".
        assert_eq!(pick_version(&fhd, &two[..1]), 0);
    }

    #[test]
    fn hdr_is_also_recognised_by_version_name() {
        let mut m = version("4k", "hevc", false);
        m.title = Some("2160p DV".into());
        assert!(media_is_hdr(&m));
        m.title = Some("Director's Cut".into());
        assert!(!media_is_hdr(&m));
    }

    #[test]
    fn show_rules_override_the_profile_field_by_field() {
        let scope = Scope { account: "alice".into(), server: "home".into() };
        let mut entries = Vec::new();
        store(
            &mut entries,
            &scope,
            None,
            None,
            Rules { audio_languages: langs(&["eng"]), subtitles: Some(SubtitleMode::Off), avoid_commentary: Some(true), ..Default::default() },
        );
        store(&mut entries, &scope, Some("42"), Some("Anime"), Rules { audio_languages: langs(&["jpn"]), subtitles: Some(SubtitleMode::Always), ..Default::default() });
        let anime = effective(&entries, &scope, Some("42"));
        assert_eq!(anime.audio_languages, langs(&["jpn"]));
        assert_eq!(anime.subtitles, Some(SubtitleMode::Always));
        assert_eq!(anime.avoid_commentary, Some(true), "inherited from the profile");
        let other = effective(&entries, &scope, Some("7"));
        assert_eq!(other.audio_languages, langs(&["eng"]));
        assert_eq!(effective(&entries, &scope, None).subtitles, Some(SubtitleMode::Off));
        // Empty rules clear the show again.
        store(&mut entries, &scope, Some("42"), None, Rules::default());
        assert_eq!(effective(&entries, &scope, Some("42")).audio_languages, langs(&["eng"]));
    }

    #[test]
    fn rules_never_cross_accounts_or_servers() {
        let alice = Scope { account: "alice".into(), server: "home".into() };
        let bob = Scope { account: "bob".into(), server: "home".into() };
        let away = Scope { account: "alice".into(), server: "away".into() };
        let mut entries = Vec::new();
        store(&mut entries, &alice, None, None, Rules { audio_languages: langs(&["jpn"]), ..Default::default() });
        store(&mut entries, &alice, Some("1"), Some("Show"), Rules { subtitles: Some(SubtitleMode::Off), ..Default::default() });
        for other in [&bob, &away] {
            assert!(effective(&entries, other, Some("1")).is_empty());
            assert_eq!(in_scope(&entries, other).count(), 0);
        }
        // Saving for another scope leaves the first one untouched.
        store(&mut entries, &bob, None, None, Rules { audio_languages: langs(&["eng"]), ..Default::default() });
        assert_eq!(effective(&entries, &alice, None).audio_languages, langs(&["jpn"]));
        assert_eq!(effective(&entries, &bob, None).audio_languages, langs(&["eng"]));
    }

    #[test]
    fn remembers_the_tracks_chosen_in_the_dialog() {
        let audio = stream(1, AUDIO, "jpn", "");
        let sub = stream(2, SUBTITLE, "eng", "");
        let r = rules_from_tracks(Some(&audio), Some(Some(&sub)));
        assert_eq!(r.audio_languages, langs(&["jpn"]));
        assert_eq!(r.subtitle_languages, langs(&["eng"]));
        assert_eq!((r.subtitles, r.forced), (Some(SubtitleMode::Always), Some(ForcedMode::Avoid)));
        assert_eq!(rules_from_tracks(None, Some(None)).subtitles, Some(SubtitleMode::Off));
        assert!(rules_from_tracks(None, None).is_empty());
        let forced_sub = forced(stream(3, SUBTITLE, "eng", ""));
        assert_eq!(rules_from_tracks(None, Some(Some(&forced_sub))).forced, Some(ForcedMode::Only));
    }

    #[test]
    fn rules_round_trip_through_json() {
        let r = Rules { resolution: Some(Resolution::Uhd), hdr: Some(HdrMode::Avoid), subtitles: Some(SubtitleMode::Foreign), ..Default::default() };
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["resolution"], "4k");
        assert_eq!(json["hdr"], "avoid");
        assert_eq!(json["subtitles"], "foreign");
        assert_eq!(serde_json::from_value::<Rules>(json).unwrap(), r);
        assert_eq!(serde_json::from_str::<Rules>("{}").unwrap(), Rules::default());
    }
}
