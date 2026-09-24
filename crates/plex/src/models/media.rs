use serde::{Deserialize, Serialize};

use crate::stream_type;

/// One version of an item (a file or set of files).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Media {
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub id: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub duration: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub bitrate: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub width: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub height: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_f64")]
    pub aspect_ratio: Option<f64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub audio_channels: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub audio_codec: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub video_codec: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub video_resolution: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub container: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub video_frame_rate: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub video_profile: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub audio_profile: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub edition_title: Option<String>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub optimized_for_streaming: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub has64bit_offsets: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub selected: bool,
    #[serde(rename = "Part", default)]
    pub parts: Vec<Part>,
}

impl Media {
    /// Human label such as "4K HEVC · DV · TrueHD 7.1".
    pub fn label(&self) -> String {
        let mut bits = Vec::new();
        if let Some(t) = self.edition_title.as_deref().or(self.title.as_deref()) {
            bits.push(t.to_string());
        }
        if let Some(r) = self.video_resolution.as_deref() {
            bits.push(match r {
                "4k" | "4K" => "4K".to_string(),
                "sd" | "SD" => "SD".to_string(),
                r if r.chars().all(|c| c.is_ascii_digit()) => format!("{r}p"),
                r => r.to_uppercase(),
            });
        }
        if let Some(v) = &self.video_codec {
            bits.push(v.to_uppercase());
        }
        if let Some(stream) = self.parts.first().and_then(|p| p.video_stream())
            && let Some(hdr) = stream.hdr_label() {
                bits.push(hdr.to_string());
            }
        if let Some(a) = &self.audio_codec {
            let ch = self.audio_channels.map(channel_label).unwrap_or_default();
            bits.push(format!("{} {}", a.to_uppercase(), ch).trim().to_string());
        }
        bits.join(" · ")
    }
}

pub fn channel_label(channels: i64) -> String {
    match channels {
        1 => "Mono".into(),
        2 => "Stereo".into(),
        3 => "2.1".into(),
        6 => "5.1".into(),
        7 => "6.1".into(),
        8 => "7.1".into(),
        n => format!("{n}ch"),
    }
}

/// A single file of a [`Media`].
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub id: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub key: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub duration: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub file: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub size: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub container: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub video_profile: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub audio_profile: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub indexes: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub decision: Option<String>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub has_thumbnail: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub selected: bool,
    #[serde(rename = "Stream", default)]
    pub streams: Vec<Stream>,
}

impl Part {
    pub fn streams_of(&self, kind: i64) -> impl Iterator<Item = &Stream> {
        self.streams.iter().filter(move |s| s.stream_type == Some(kind))
    }

    pub fn video_stream(&self) -> Option<&Stream> {
        self.streams_of(stream_type::VIDEO).next()
    }

    pub fn audio_streams(&self) -> impl Iterator<Item = &Stream> {
        self.streams_of(stream_type::AUDIO)
    }

    pub fn subtitle_streams(&self) -> impl Iterator<Item = &Stream> {
        self.streams_of(stream_type::SUBTITLE)
    }

    pub fn selected_audio(&self) -> Option<&Stream> {
        self.audio_streams().find(|s| s.selected).or_else(|| self.audio_streams().next())
    }

    pub fn selected_subtitle(&self) -> Option<&Stream> {
        self.subtitle_streams().find(|s| s.selected)
    }

    /// The stream in this part that best corresponds to `template`, which
    /// usually comes from another episode. Language must match (and for
    /// subtitles, forced-ness too). Ties go to the stream that comes first.
    pub fn matching_stream(&self, template: &Stream) -> Option<&Stream> {
        let kind = template.stream_type?;
        let mut best: Option<(u32, &Stream)> = None;
        for s in self.streams_of(kind) {
            let Some(score) = template.similarity(s) else { continue };
            if best.is_none_or(|(b, _)| score > b) {
                best = Some((score, s));
            }
        }
        best.map(|(_, s)| s)
    }
}

/// A video, audio, subtitle or lyrics stream inside a part.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Stream {
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub id: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub stream_type: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub default: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub selected: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub forced: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub hearing_impaired: bool,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub codec: Option<String>,
    /// Container stream index (matches mpv's `ff-index`). Absent for sidecar files.
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub index: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub bitrate: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub language: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub language_code: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub language_tag: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub display_title: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub extended_display_title: Option<String>,
    /// Download path for sidecar subtitles.
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub key: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub format: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub channels: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub sampling_rate: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub audio_channel_layout: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub profile: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub width: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub height: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_f64")]
    pub frame_rate: Option<f64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub bit_depth: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub color_primaries: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub color_trc: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub color_space: Option<String>,
    #[serde(rename = "DOVIPresent", default, deserialize_with = "crate::de::boolean")]
    pub dovi_present: bool,
    #[serde(rename = "DOVIProfile", default, deserialize_with = "crate::de::opt_i64")]
    pub dovi_profile: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub decision: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub location: Option<String>,
}

impl Stream {
    pub fn label(&self) -> String {
        self.extended_display_title
            .clone()
            .or_else(|| self.display_title.clone())
            .or_else(|| self.title.clone())
            .or_else(|| self.language.clone())
            .unwrap_or_else(|| format!("Track {}", self.index.unwrap_or_default()))
    }

    /// Short label: "English (AC3 5.1)" for audio, "English (SRT)" for subtitles.
    pub fn short_label(&self) -> String {
        self.display_title.clone().unwrap_or_else(|| self.label())
    }

    pub fn is_external(&self) -> bool {
        self.key.is_some() && self.index.is_none()
    }

    /// Language identity used for matching: the code, else the tag, else the name.
    fn language_key(&self) -> Option<String> {
        self.language_code
            .as_deref()
            .or(self.language_tag.as_deref())
            .or(self.language.as_deref())
            .map(str::to_ascii_lowercase)
    }

    /// How closely `other` resembles this stream, or `None` if it can't stand in
    /// for it at all (a different type or language, or forced vs. full subtitles).
    pub fn similarity(&self, other: &Stream) -> Option<u32> {
        if self.stream_type != other.stream_type || self.language_key() != other.language_key() {
            return None;
        }
        let subtitle = self.stream_type == Some(crate::stream_type::SUBTITLE);
        if subtitle && self.forced != other.forced {
            return None;
        }
        let same = |a: &Option<String>, b: &Option<String>| a.is_some() && a.as_deref().map(str::to_lowercase) == b.as_deref().map(str::to_lowercase);
        let mut score = 1;
        if same(&self.title, &other.title) {
            score += 8;
        }
        if same(&self.extended_display_title, &other.extended_display_title) {
            score += 4;
        } else if same(&self.display_title, &other.display_title) {
            score += 2;
        }
        if same(&self.codec, &other.codec) {
            score += 3;
        }
        if self.channels.is_some() && self.channels == other.channels {
            score += 3;
        }
        if self.hearing_impaired == other.hearing_impaired {
            score += 2;
        }
        if self.is_external() == other.is_external() {
            score += 1;
        }
        Some(score)
    }

    pub fn hdr_label(&self) -> Option<&'static str> {
        if self.dovi_present {
            return Some("Dolby Vision");
        }
        match self.color_trc.as_deref() {
            Some("smpte2084") => Some("HDR10"),
            Some("arib-std-b67") => Some("HLG"),
            _ => None,
        }
    }
}
