use super::PlexServer;
use crate::client::Params;
use crate::error::Result;
use crate::models::*;

/// Filters for watch history.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HistoryQuery {
    pub account_id: Option<i64>,
    pub section_id: Option<String>,
    pub rating_key: Option<String>,
    /// Only entries viewed at or after this unix timestamp.
    pub since: Option<i64>,
    pub start: u32,
    pub size: u32,
}

impl PlexServer {
    /// Everything currently playing on the server, with user, player and transcode info.
    pub async fn sessions(&self) -> Result<Vec<Metadata>> {
        Ok(self.get("/status/sessions", &Params::new()).await?.metadata)
    }

    /// Watch history (newest first).
    pub async fn history(&self, q: &HistoryQuery) -> Result<MediaContainer> {
        let p = Params::new()
            .set("sort", "viewedAt:desc")
            .opt("accountID", q.account_id)
            .opt("librarySectionID", q.section_id.clone())
            .opt("metadataItemID", q.rating_key.clone())
            .opt("viewedAt>", q.since)
            .set("X-Plex-Container-Start", q.start)
            .set("X-Plex-Container-Size", if q.size == 0 { 100 } else { q.size });
        self.get("/status/sessions/history/all", &p).await
    }

    pub async fn delete_history(&self, history_id: &str) -> Result<()> {
        self.delete(&format!("/status/sessions/history/{history_id}"), &Params::new()).await
    }

    /// Stop someone's stream with a message (Plex Pass).
    pub async fn terminate_session(&self, session_id: &str, reason: &str) -> Result<()> {
        let p = Params::new().set("sessionId", session_id).set("reason", reason);
        self.empty(reqwest::Method::GET, "/status/sessions/terminate", &p).await
    }
}
