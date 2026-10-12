//! Playback failures: why mpv (or the setup before it) gave up, and what can be tried next.
//!
//! The player reports a failure as one [`PlaybackFailure`] event carrying a
//! readable reason and the actions that make sense for it. Front ends offer
//! them and answer with [`recover`]. The request needed to try again stays
//! here, so a front end only sends the choice.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use patina_plex::Metadata;
use serde::{Deserialize, Serialize};

use crate::config::{self, QUALITIES, Quality};
use crate::player::PlayRequest;
use crate::recovery::Scope;
use crate::state::{AppState, CoreEvent, Session};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FailureKind {
    /// The stream could not be loaded or was cut off: the connection or the server.
    Network,
    /// mpv could not make sense of the file.
    Format,
    /// mpv could not open an audio or video device.
    Output,
    Other,
}

/// Whether playback never got going or broke after it had.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Start,
    Playing,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct QualityChoice {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackFailure {
    /// Distinguishes this failure from earlier ones.
    pub id: u64,
    pub item: Metadata,
    /// A sentence for people.
    pub reason: String,
    /// What mpv (or the underlying error) said.
    pub detail: Option<String>,
    pub kind: FailureKind,
    pub phase: Phase,
    /// Where trying again resumes.
    pub position_ms: i64,
    /// The version (media) that failed.
    pub media_index: usize,
    /// The quality preset that was in use.
    pub quality: String,
    /// The next step down, when transcoding more aggressively is an option.
    pub lower_quality: Option<QualityChoice>,
    pub can_retry: bool,
    /// Items after the failed one that stay queued.
    pub remaining: usize,
}

/// What to do about a failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Recover {
    /// Try again from where it stopped.
    Retry,
    /// Try another version (media) of the item.
    Version { index: usize },
    /// Transcode at the next lower quality.
    LowerQuality,
}

struct Failed {
    failure: PlaybackFailure,
    request: PlayRequest,
    /// The account and server it failed on: its rating keys mean nothing elsewhere.
    scope: Option<Scope>,
    at: Instant,
    auto_retried: bool,
}

static FAILED: Mutex<Option<Failed>> = Mutex::new(None);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// The failure waiting for a decision, if any.
pub fn current() -> Option<PlaybackFailure> {
    FAILED.lock().unwrap().as_ref().map(|f| f.failure.clone())
}

/// Forget the failure (dismissed, or something else started playing).
pub fn clear(app: &AppState) {
    if FAILED.lock().unwrap().take().is_some() {
        app.emit(CoreEvent::PlaybackFailure(None));
    }
}

fn scope_of(app: &AppState) -> Option<Scope> {
    app.session().as_ref().map(Scope::of)
}

/// The account, profile or server changed: a failure from another one can't be acted on here.
pub fn session_changed(app: &AppState, session: Option<&Session>) {
    let scope = session.map(Scope::of);
    let stale = FAILED.lock().unwrap().take_if(|f| f.scope != scope).is_some();
    if stale {
        app.emit(CoreEvent::PlaybackFailure(None));
    }
}

/// Everything about a failure that the player knows.
pub struct Report {
    pub item: Metadata,
    pub error: Option<String>,
    pub kind: Option<FailureKind>,
    pub phase: Phase,
    pub position_ms: i64,
    pub media_index: usize,
    pub quality: String,
    pub remaining: usize,
    /// How to start the same playback again (from the failed item on).
    pub request: PlayRequest,
}

/// Publish a failure and keep what is needed to act on it.
pub fn record(app: &AppState, report: Report) {
    // A failure from before mpv was involved already carries readable text; mpv's is translated.
    let (kind, reason, detail) = match report.kind {
        Some(kind) => (kind, report.error.unwrap_or_default(), None),
        None => {
            let (kind, text) = explain(report.error.as_deref().unwrap_or(""));
            (kind, text.to_string(), report.error.filter(|e| !e.is_empty()))
        }
    };
    let video = matches!(report.item.kind(), "movie" | "episode" | "clip" | "trailer");
    let lower = video.then(|| lower_quality(&report.quality)).flatten();
    let live = report.request.live;
    let failure = PlaybackFailure {
        id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        reason,
        detail,
        kind,
        phase: report.phase,
        position_ms: report.position_ms.max(0),
        media_index: report.media_index,
        quality: report.quality,
        lower_quality: lower.map(|q| QualityChoice { id: q.id.into(), label: q.label.into() }),
        can_retry: !live,
        remaining: report.remaining,
        item: report.item,
    };
    app.error(format!("Couldn't play {}: {}", failure.item.full_title(), failure.reason));
    app.emit(CoreEvent::PlaybackFailure(Some(Box::new(failure.clone()))));
    let scope = scope_of(app);
    *FAILED.lock().unwrap() = Some(Failed { failure, request: report.request, scope, at: Instant::now(), auto_retried: false });
}

/// Turn what mpv said into a kind of failure and a sentence for people.
pub fn explain(error: &str) -> (FailureKind, &'static str) {
    let e = error.to_ascii_lowercase();
    if e.contains("loading failed") {
        (FailureKind::Network, "The stream could not be loaded. The connection to the server may have dropped, or the server could not serve this file.")
    } else if e.contains("unrecognized file format") || e.contains("unknown format") {
        (FailureKind::Format, "mpv does not recognise this file's format.")
    } else if e.contains("no audio or video") || e.contains("nothing to play") {
        (FailureKind::Format, "The file has no audio or video that mpv could play.")
    } else if e.contains("demux") || e.contains("decod") {
        (FailureKind::Format, "mpv could not read or decode this file.")
    } else if e.contains("audio output") {
        (FailureKind::Output, "mpv could not open the audio output. Check your sound device.")
    } else if e.contains("video output") {
        (FailureKind::Output, "mpv could not open the video output. Check your graphics settings or mpv's vo setting.")
    } else {
        (FailureKind::Other, "mpv stopped with an error.")
    }
}

/// Classify an error from before mpv was involved (building the queue, starting the player).
pub fn explain_error(e: &anyhow::Error) -> (FailureKind, String) {
    let kind = e
        .chain()
        .find_map(|cause| match cause.downcast_ref::<patina_plex::Error>() {
            Some(patina_plex::Error::Http(h)) if h.is_connect() || h.is_timeout() || h.is_request() => Some(FailureKind::Network),
            Some(patina_plex::Error::NoConnection(_)) => Some(FailureKind::Network),
            _ => None,
        })
        .unwrap_or(FailureKind::Other);
    (kind, format!("{e:#}"))
}

/// The preset one step below `current`, as a fallback for a stream that will not play or keep up.
/// Direct play steps down to a 1080p transcode first; a 4K transcode is only offered by choice.
pub fn lower_quality(current: &str) -> Option<&'static Quality> {
    let find = |id: &str| QUALITIES.iter().find(|q| q.id == id);
    if current == "original" || config::quality(current).is_none() {
        return find("20m-1080");
    }
    let at = QUALITIES.iter().position(|q| q.id == current)?;
    QUALITIES.get(at + 1)
}

/// Act on the failure shown to the user.
pub fn recover(app: &AppState, action: Recover) -> Result<()> {
    let failed = FAILED.lock().unwrap().take().ok_or_else(|| anyhow!("There is nothing to retry."))?;
    let Failed { failure, mut request, scope, .. } = failed;
    if scope != scope_of(app) {
        app.emit(CoreEvent::PlaybackFailure(None));
        anyhow::bail!("The account or server changed since this failed, so it can't be retried.");
    }
    match action {
        Recover::Retry => {}
        Recover::Version { index } => {
            anyhow::ensure!(index < failure.item.media.len().max(1), "That version does not exist.");
            request.media_index = Some(index);
        }
        Recover::LowerQuality => {
            let q = failure.lower_quality.as_ref().ok_or_else(|| anyhow!("There is no lower quality to try."))?;
            request.quality = Some(q.id.clone());
        }
    }
    app.emit(CoreEvent::PlaybackFailure(None));
    request.resume = true;
    crate::player::play_from(app.clone(), request, failure.position_ms);
    Ok(())
}

/// The server answered again after a network failure: try once more, on our own.
pub fn on_reconnect(app: &AppState) {
    let retry = {
        let mut guard = FAILED.lock().unwrap();
        match guard.as_mut() {
            Some(f)
                if f.failure.kind == FailureKind::Network
                    && f.failure.can_retry
                    && f.scope == scope_of(app)
                    && !f.auto_retried
                    && f.at.elapsed() < Duration::from_secs(15 * 60)
                    && !crate::player::is_active() =>
            {
                f.auto_retried = true;
                true
            }
            _ => false,
        }
    };
    if retry {
        app.toast("Reconnected; resuming playback");
        let _ = recover(app, Recover::Retry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explains_what_mpv_says() {
        assert_eq!(explain("loading failed").0, FailureKind::Network);
        assert_eq!(explain("unrecognized file format").0, FailureKind::Format);
        assert_eq!(explain("no audio or video data played").0, FailureKind::Format);
        assert_eq!(explain("audio output initialization failed").0, FailureKind::Output);
        assert_eq!(explain("video output initialization failed").0, FailureKind::Output);
        assert_eq!(explain("").0, FailureKind::Other);
    }

    #[test]
    fn lowers_quality_one_step_at_a_time() {
        assert_eq!(lower_quality("original").map(|q| q.id), Some("20m-1080"));
        assert_eq!(lower_quality("20m-1080").map(|q| q.id), Some("12m-1080"));
        assert_eq!(lower_quality("2m-720").map(|q| q.id), Some("1m-480"));
        assert_eq!(lower_quality("1m-480"), None);
        assert_eq!(lower_quality("nonsense").map(|q| q.id), Some("20m-1080"));
    }

    #[test]
    fn network_errors_are_recognised_through_the_error_chain() {
        let e = anyhow::Error::new(patina_plex::Error::NoConnection("Home".into())).context("creating play queue");
        assert_eq!(explain_error(&e).0, FailureKind::Network);
        assert_eq!(explain_error(&anyhow!("mpv is missing")).0, FailureKind::Other);
    }
}
