use reqwest::Method;

use super::PlexServer;
use crate::client::Params;
use crate::error::Result;
use crate::models::*;

/// Parameters for creating a play queue.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayQueueRequest {
    /// Library URI of the source (item, show, season, album, section filter...).
    pub uri: Option<String>,
    /// Build from a playlist instead of a URI.
    pub playlist_id: Option<String>,
    /// `video`, `audio` or `photo`.
    pub kind: String,
    /// Item to start at (a `/library/metadata/<id>` key).
    pub key: Option<String>,
    pub shuffle: bool,
    pub repeat: bool,
    /// Continue with the following items (next episodes) after `key`.
    pub continuous: bool,
    pub include_chapters: bool,
    pub include_markers: bool,
}

impl PlayQueueRequest {
    pub fn video(uri: String) -> Self {
        Self { uri: Some(uri), kind: "video".into(), ..Default::default() }
    }
}

impl PlexServer {
    pub async fn create_play_queue(&self, req: &PlayQueueRequest) -> Result<MediaContainer> {
        let p = Params::new()
            .set("type", &req.kind)
            .opt("uri", req.uri.clone())
            .opt("playlistID", req.playlist_id.clone())
            .opt("key", req.key.clone())
            .set("shuffle", req.shuffle as u8)
            .set("repeat", req.repeat as u8)
            .set("continuous", req.continuous as u8)
            .flag("includeChapters", req.include_chapters)
            .flag("includeMarkers", req.include_markers)
            .set("own", 1);
        self.container(Method::POST, "/playQueues", &p).await
    }

    /// Fetch a play queue; `window` limits items around the selected one.
    pub async fn play_queue(&self, id: i64, window: Option<u32>) -> Result<MediaContainer> {
        let p = Params::new()
            .opt("window", window)
            .set("includeMarkers", 1)
            .set("includeChapters", 1);
        self.get(&format!("/playQueues/{id}"), &p).await
    }

    /// Add items; `next` inserts after the current item ("Play Next").
    pub async fn add_to_play_queue(&self, id: i64, uri: &str, next: bool) -> Result<MediaContainer> {
        let p = Params::new().set("uri", uri).flag("next", next);
        self.container(Method::PUT, &format!("/playQueues/{id}"), &p).await
    }

    pub async fn remove_from_play_queue(&self, id: i64, item_id: i64) -> Result<MediaContainer> {
        self.container(Method::DELETE, &format!("/playQueues/{id}/items/{item_id}"), &Params::new()).await
    }

    pub async fn move_play_queue_item(&self, id: i64, item_id: i64, after: Option<i64>) -> Result<MediaContainer> {
        self.container(Method::PUT, &format!("/playQueues/{id}/items/{item_id}/move"), &Params::new().opt("after", after))
            .await
    }

    pub async fn shuffle_play_queue(&self, id: i64) -> Result<MediaContainer> {
        self.container(Method::PUT, &format!("/playQueues/{id}/shuffle"), &Params::new()).await
    }

    pub async fn unshuffle_play_queue(&self, id: i64) -> Result<MediaContainer> {
        self.container(Method::PUT, &format!("/playQueues/{id}/unshuffle"), &Params::new()).await
    }

    pub async fn clear_play_queue(&self, id: i64) -> Result<MediaContainer> {
        self.container(Method::DELETE, &format!("/playQueues/{id}/items"), &Params::new()).await
    }

    /// Reset the queue position to the first item.
    pub async fn reset_play_queue(&self, id: i64) -> Result<MediaContainer> {
        self.container(Method::PUT, &format!("/playQueues/{id}/reset"), &Params::new()).await
    }
}
