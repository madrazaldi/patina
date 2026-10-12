//! Automatic playback preferences (`patina_core::prefs`) as the settings
//! rows and per-show picker present them: one [`AutoField`] per rule.

use patina_core::prefs::{Rules, ShowRules};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// The profile's rules and the shows with rules of their own.
#[derive(Debug, Clone, Default)]
pub struct AutoPrefs {
    pub profile: Rules,
    pub shows: Vec<ShowRules>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoField {
    Audio,
    Subtitles,
    SubtitleLanguages,
    Forced,
    Commentary,
    Resolution,
    Codec,
    Hdr,
}

pub const FIELDS: [AutoField; 8] = [
    AutoField::Audio,
    AutoField::Subtitles,
    AutoField::SubtitleLanguages,
    AutoField::Forced,
    AutoField::Commentary,
    AutoField::Resolution,
    AutoField::Codec,
    AutoField::Hdr,
];

const SUBTITLES: [(&str, &str); 4] = [("keep", "Leave as Plex has them"), ("off", "Off"), ("always", "Always on"), ("foreign", "Only for foreign dialogue")];
const FORCED: [(&str, &str); 4] =
    [("auto", "Full subtitles (forced if nothing else)"), ("prefer", "Forced first, then full"), ("avoid", "Never forced"), ("only", "Only forced")];
const COMMENTARY: [(&str, &str); 2] = [("true", "Skip commentary"), ("false", "Allow commentary")];
const RESOLUTION: [(&str, &str); 5] = [("any", "Any"), ("4k", "4K"), ("1080", "1080p"), ("720", "720p"), ("sd", "SD")];
const CODEC: [(&str, &str); 4] = [("h264", "H.264"), ("hevc", "HEVC (H.265)"), ("av1", "AV1"), ("vp9", "VP9")];
const HDR: [(&str, &str); 3] = [("any", "Any"), ("prefer", "Prefer HDR"), ("avoid", "Avoid HDR (SDR)")];

fn text_of<T: Serialize>(v: &Option<T>) -> String {
    v.as_ref().and_then(|v| serde_json::to_value(v).ok()).and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

fn parse<T: DeserializeOwned>(v: &str) -> Option<T> {
    serde_json::from_value(Value::String(v.into())).ok()
}

fn list(v: &str) -> Option<Vec<String>> {
    let out: Vec<String> = v.split([',', ';']).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    (!out.is_empty()).then_some(out)
}

impl AutoField {
    pub fn id(self) -> &'static str {
        match self {
            Self::Audio => "audio",
            Self::Subtitles => "subtitles",
            Self::SubtitleLanguages => "subtitle-languages",
            Self::Forced => "forced",
            Self::Commentary => "commentary",
            Self::Resolution => "resolution",
            Self::Codec => "codec",
            Self::Hdr => "hdr",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        FIELDS.into_iter().find(|f| f.id() == id)
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Audio => "Audio languages",
            Self::Subtitles => "Subtitles",
            Self::SubtitleLanguages => "Subtitle languages",
            Self::Forced => "Forced subtitles",
            Self::Commentary => "Skip commentary",
            Self::Resolution => "Version: resolution",
            Self::Codec => "Version: video codec",
            Self::Hdr => "Version: HDR",
        }
    }

    pub fn help(self) -> &'static str {
        match self {
            Self::Audio => "In order of preference, e.g. eng, ja. Language codes or names.",
            Self::Subtitles => "When subtitles are turned on.",
            Self::SubtitleLanguages => "Used for subtitles; the audio languages when empty.",
            Self::Forced => "Which kind of subtitle track to pick.",
            Self::Commentary => "Never pick commentary or audio-description tracks.",
            Self::Resolution => "Which version to play when a title has several files.",
            Self::Codec => "Preferred video codec when a title has several versions.",
            Self::Hdr => "Whether to prefer or avoid HDR versions.",
        }
    }

    /// Typed in, rather than chosen from a list.
    pub fn is_text(self) -> bool {
        matches!(self, Self::Audio | Self::SubtitleLanguages)
    }

    fn table(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::Subtitles => &SUBTITLES,
            Self::Forced => &FORCED,
            Self::Commentary => &COMMENTARY,
            Self::Resolution => &RESOLUTION,
            Self::Codec => &CODEC,
            Self::Hdr => &HDR,
            Self::Audio | Self::SubtitleLanguages => &[],
        }
    }

    /// The rule's value as edited: the typed list, or the choice's value ("" when not set).
    pub fn value(self, r: &Rules) -> String {
        match self {
            Self::Audio => r.audio_languages.as_deref().unwrap_or_default().join(", "),
            Self::SubtitleLanguages => r.subtitle_languages.as_deref().unwrap_or_default().join(", "),
            Self::Subtitles => text_of(&r.subtitles),
            Self::Forced => text_of(&r.forced),
            Self::Commentary => r.avoid_commentary.map(|b| b.to_string()).unwrap_or_default(),
            Self::Resolution => text_of(&r.resolution),
            Self::Codec => r.codec.clone().unwrap_or_default(),
            Self::Hdr => text_of(&r.hdr),
        }
    }

    /// Choices as (value, label), the first being "not set".
    pub fn choices(self, r: &Rules, unset: &str) -> Vec<(String, String)> {
        let mut out = vec![(String::new(), unset.to_string())];
        out.extend(self.table().iter().map(|(v, l)| (v.to_string(), l.to_string())));
        let current = self.value(r);
        if !current.is_empty() && !out.iter().any(|(v, _)| *v == current) {
            out.push((current.clone(), current));
        }
        out
    }

    /// What the rule currently says, for a row's note.
    pub fn label(self, r: &Rules, unset: &str) -> String {
        let value = self.value(r);
        if value.is_empty() {
            return unset.to_string();
        }
        self.table().iter().find(|(v, _)| *v == value).map_or(value, |(_, l)| l.to_string())
    }

    /// Store a typed or chosen value; empty clears the rule.
    pub fn set(self, r: &mut Rules, v: &str) {
        let v = v.trim();
        match self {
            Self::Audio => r.audio_languages = list(v),
            Self::SubtitleLanguages => r.subtitle_languages = list(v),
            Self::Subtitles => r.subtitles = parse(v),
            Self::Forced => r.forced = parse(v),
            Self::Commentary => r.avoid_commentary = v.parse().ok(),
            Self::Resolution => r.resolution = parse(v),
            Self::Codec => r.codec = Some(v.to_lowercase()).filter(|c| !c.is_empty()),
            Self::Hdr => r.hdr = parse(v),
        }
    }
}

/// "ja audio · subtitles for foreign dialogue · 1080p", or "no rules".
pub fn describe(r: &Rules) -> String {
    let bits: Vec<String> = FIELDS
        .into_iter()
        .filter(|f| !f.value(r).is_empty())
        .map(|f| match f {
            AutoField::Audio => format!("{} audio", f.value(r)),
            AutoField::SubtitleLanguages => format!("subtitles in {}", f.value(r)),
            AutoField::Subtitles => format!("subtitles {}", f.label(r, "").to_lowercase()),
            AutoField::Forced => format!("forced: {}", f.label(r, "").to_lowercase()),
            _ => f.label(r, "").to_lowercase(),
        })
        .collect();
    if bits.is_empty() { "no rules".into() } else { bits.join(" · ") }
}

#[cfg(test)]
mod tests {
    use super::*;
    use patina_core::prefs::{ForcedMode, Resolution, SubtitleMode};

    #[test]
    fn values_round_trip_through_the_rules() {
        let mut r = Rules::default();
        AutoField::Audio.set(&mut r, "eng; ja ,");
        AutoField::Subtitles.set(&mut r, "foreign");
        AutoField::Forced.set(&mut r, "only");
        AutoField::Commentary.set(&mut r, "true");
        AutoField::Resolution.set(&mut r, "4k");
        AutoField::Codec.set(&mut r, "HEVC");
        assert_eq!(r.audio_languages, Some(vec!["eng".into(), "ja".into()]));
        assert_eq!(r.subtitles, Some(SubtitleMode::Foreign));
        assert_eq!(r.forced, Some(ForcedMode::Only));
        assert_eq!(r.avoid_commentary, Some(true));
        assert_eq!(r.resolution, Some(Resolution::Uhd));
        assert_eq!(r.codec.as_deref(), Some("hevc"));
        for f in FIELDS {
            let v = f.value(&r);
            if f != AutoField::SubtitleLanguages && f != AutoField::Hdr {
                assert!(!v.is_empty(), "{f:?}");
            }
            let mut again = Rules::default();
            f.set(&mut again, &v);
            assert_eq!(f.value(&again), v, "{f:?}");
        }
    }

    #[test]
    fn empty_values_clear_a_rule() {
        let mut r = Rules { audio_languages: Some(vec!["en".into()]), subtitles: Some(SubtitleMode::Off), ..Default::default() };
        AutoField::Audio.set(&mut r, " ");
        AutoField::Subtitles.set(&mut r, "");
        assert!(r.is_empty());
    }

    #[test]
    fn choices_start_with_not_set_and_keep_an_unusual_value() {
        let r = Rules { codec: Some("mpeg4".into()), ..Default::default() };
        let c = AutoField::Codec.choices(&r, "Same as profile");
        assert_eq!(c[0], (String::new(), "Same as profile".to_string()));
        assert!(c.iter().any(|(v, _)| v == "mpeg4"));
        assert_eq!(AutoField::Codec.label(&r, "-"), "mpeg4");
        assert_eq!(AutoField::Resolution.label(&r, "-"), "-");
    }

    #[test]
    fn describes_the_rules_in_words() {
        let mut r = Rules::default();
        assert_eq!(describe(&r), "no rules");
        AutoField::Audio.set(&mut r, "ja");
        AutoField::Subtitles.set(&mut r, "foreign");
        AutoField::Resolution.set(&mut r, "1080");
        assert_eq!(describe(&r), "ja audio · subtitles only for foreign dialogue · 1080p");
    }

    #[test]
    fn ids_find_their_field() {
        for f in FIELDS {
            assert_eq!(AutoField::from_id(f.id()), Some(f));
        }
        assert_eq!(AutoField::from_id("nope"), None);
    }
}
