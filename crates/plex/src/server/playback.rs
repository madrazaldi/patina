use reqwest::Method;
use url::Url;

use super::PlexServer;
use crate::client::Params;
use crate::error::Result;
use crate::models::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineState {
    Playing,
    Paused,
    Buffering,
    Stopped,
}

impl TimelineState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Playing => "playing",
            Self::Paused => "paused",
            Self::Buffering => "buffering",
            Self::Stopped => "stopped",
        }
    }
}

/// A playback progress report. Plex uses these to track resume points,
/// mark items watched and show the session on the dashboard.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineUpdate {
    pub rating_key: String,
    pub key: String,
    pub state: TimelineState,
    pub time_ms: i64,
    pub duration_ms: i64,
    pub play_queue_item_id: Option<i64>,
    /// The session id used for transcoding, if any.
    pub session: Option<String>,
}

/// Options for the universal transcoder.
#[derive(Debug, Clone, PartialEq)]
pub struct TranscodeOptions {
    pub session: String,
    pub media_index: usize,
    pub part_index: usize,
    /// Maximum video bitrate in kbps.
    pub max_video_bitrate: Option<u32>,
    /// e.g. `1920x1080`.
    pub video_resolution: Option<String>,
    pub video_quality: Option<u32>,
    pub direct_stream: bool,
    pub direct_stream_audio: bool,
    /// `burn`, `sidecar`, `embedded`, `none` or `auto`.
    pub subtitles: String,
    pub subtitle_size: Option<u32>,
    pub audio_boost: Option<u32>,
    pub offset_secs: f64,
    pub protocol: String,
}

impl Default for TranscodeOptions {
    fn default() -> Self {
        Self {
            session: uuid::Uuid::new_v4().to_string(),
            media_index: 0,
            part_index: 0,
            max_video_bitrate: None,
            video_resolution: None,
            video_quality: None,
            direct_stream: true,
            direct_stream_audio: true,
            subtitles: "auto".into(),
            subtitle_size: None,
            audio_boost: None,
            offset_secs: 0.0,
            protocol: "hls".into(),
        }
    }
}

impl PlexServer {
    /// Report playback progress.
    pub async fn timeline(&self, u: &TimelineUpdate) -> Result<()> {
        let p = Params::new()
            .set("ratingKey", &u.rating_key)
            .set("key", &u.key)
            .set("state", u.state.as_str())
            .set("time", u.time_ms.max(0))
            .set("duration", u.duration_ms.max(0))
            .set("identifier", "com.plexapp.plugins.library")
            .set("hasMDE", 1)
            .opt("playQueueItemID", u.play_queue_item_id)
            .opt("X-Plex-Session-Identifier", u.session.clone());
        self.empty(Method::GET, "/:/timeline", &p).await
    }

    fn transcode_params(&self, path: &str, o: &TranscodeOptions) -> Params {
        let info = self.client_info();
        Params::new()
            .set("path", path)
            .set("mediaIndex", o.media_index)
            .set("partIndex", o.part_index)
            .set("protocol", &o.protocol)
            .set("fastSeek", 1)
            .set("directPlay", 0)
            .set("directStream", o.direct_stream as u8)
            .set("directStreamAudio", o.direct_stream_audio as u8)
            .set("subtitles", &o.subtitles)
            .opt("subtitleSize", o.subtitle_size)
            .opt("audioBoost", o.audio_boost)
            .opt("maxVideoBitrate", o.max_video_bitrate)
            .opt("videoBitrate", o.max_video_bitrate)
            .opt("videoResolution", o.video_resolution.clone())
            .opt("videoQuality", o.video_quality)
            .set("offset", format!("{:.3}", o.offset_secs))
            .set("copyts", 1)
            .set("mediaBufferSize", 102400)
            .set("session", &o.session)
            .set("X-Plex-Session-Identifier", &o.session)
            .set("X-Plex-Client-Identifier", &info.identifier)
            .set("X-Plex-Product", &info.product)
            .set("X-Plex-Version", &info.version)
            .set("X-Plex-Device", &info.device)
            // The "Chrome" profile gives broadly compatible HLS (h264/hevc + aac/ac3).
            .set("X-Plex-Platform", "Chrome")
            .set(
                "X-Plex-Client-Profile-Extra",
                "add-transcode-target-audio-codec(type=videoProfile&context=streaming&protocol=hls&audioCodec=ac3,eac3)",
            )
    }

    /// Ask the server what it would do (direct play/stream/transcode) for these options.
    pub async fn transcode_decision(&self, path: &str, o: &TranscodeOptions) -> Result<MediaContainer> {
        self.get("/video/:/transcode/universal/decision", &self.transcode_params(path, o)).await
    }

    /// HLS playlist URL for a transcoded stream of `path` (`/library/metadata/<id>`).
    pub fn transcode_url(&self, path: &str, o: &TranscodeOptions) -> Result<Url> {
        self.authed_url("/video/:/transcode/universal/start.m3u8", &self.transcode_params(path, o))
    }

    /// Transcoded audio stream URL (for music).
    pub fn audio_transcode_url(&self, path: &str, o: &TranscodeOptions) -> Result<Url> {
        let mut p = self.transcode_params(path, o);
        p.0.retain(|(k, _)| k != "protocol");
        p.push("protocol", "http");
        p.push("musicBitrate", o.max_video_bitrate.unwrap_or(320));
        self.authed_url("/music/:/transcode/universal/start.mp3", &p)
    }

    /// Keep a transcode session alive while paused.
    pub async fn ping_transcode(&self, session: &str) -> Result<()> {
        self.empty(Method::GET, "/video/:/transcode/universal/ping", &Params::new().set("session", session)).await
    }

    pub async fn stop_transcode(&self, session: &str) -> Result<()> {
        self.empty(Method::GET, "/video/:/transcode/universal/stop", &Params::new().set("session", session)).await
    }

    /// Active transcoder sessions.
    pub async fn transcode_sessions(&self) -> Result<Vec<TranscodeSession>> {
        Ok(self.get("/transcode/sessions", &Params::new()).await?.transcode_session)
    }

    pub async fn kill_transcode_session(&self, key: &str) -> Result<()> {
        self.delete(&format!("/transcode/sessions/{key}"), &Params::new()).await
    }
}
