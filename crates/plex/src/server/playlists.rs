use reqwest::Method;

use super::PlexServer;
use crate::client::Params;
use crate::error::Result;
use crate::models::*;

impl PlexServer {
    /// All playlists; `kind` is `video`, `audio` or `photo`.
    pub async fn playlists(&self, kind: Option<&str>, smart: Option<bool>) -> Result<Vec<Metadata>> {
        let p = Params::new().opt("playlistType", kind).opt("smart", smart.map(|s| s as u8));
        Ok(self.get("/playlists", &p).await?.metadata)
    }

    pub async fn playlist(&self, id: &str) -> Result<Option<Metadata>> {
        Ok(self.get(&format!("/playlists/{id}"), &Params::new()).await?.metadata.into_iter().next())
    }

    pub async fn playlist_items(&self, id: &str, start: u32, size: u32) -> Result<MediaContainer> {
        let p = Params::new().set("X-Plex-Container-Start", start).set("X-Plex-Container-Size", size);
        self.get(&format!("/playlists/{id}/items"), &p).await
    }

    /// Create a playlist from item URIs (see [`PlexServer::item_uri`]).
    /// For smart playlists pass a library URI with filters and `smart = true`.
    pub async fn create_playlist(&self, title: &str, kind: &str, uri: &str, smart: bool) -> Result<Option<Metadata>> {
        let p = Params::new().set("title", title).set("type", kind).set("uri", uri).set("smart", smart as u8);
        Ok(self.container(Method::POST, "/playlists", &p).await?.metadata.into_iter().next())
    }

    pub async fn add_to_playlist(&self, id: &str, uri: &str) -> Result<()> {
        self.put(&format!("/playlists/{id}/items"), &Params::new().set("uri", uri)).await
    }

    pub async fn remove_from_playlist(&self, id: &str, playlist_item_id: i64) -> Result<()> {
        self.delete(&format!("/playlists/{id}/items/{playlist_item_id}"), &Params::new()).await
    }

    /// Move an item after another one; `after = None` moves it to the top.
    pub async fn move_playlist_item(&self, id: &str, playlist_item_id: i64, after: Option<i64>) -> Result<()> {
        self.put(&format!("/playlists/{id}/items/{playlist_item_id}/move"), &Params::new().opt("after", after))
            .await
    }

    pub async fn clear_playlist(&self, id: &str) -> Result<()> {
        self.delete(&format!("/playlists/{id}/items"), &Params::new()).await
    }

    pub async fn update_playlist(&self, id: &str, title: Option<&str>, summary: Option<&str>) -> Result<()> {
        let p = Params::new().opt("title", title).opt("summary", summary);
        self.put(&format!("/playlists/{id}"), &p).await
    }

    pub async fn delete_playlist(&self, id: &str) -> Result<()> {
        self.delete(&format!("/playlists/{id}"), &Params::new()).await
    }

    /// Import an .m3u file that lives on the server's filesystem.
    pub async fn upload_playlist(&self, section_id: &str, path: &str, force: bool) -> Result<()> {
        let p = Params::new().set("sectionID", section_id).set("path", path).flag("force", force);
        self.post("/playlists/upload", &p).await
    }
}
