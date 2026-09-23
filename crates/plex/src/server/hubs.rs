use super::PlexServer;
use crate::client::Params;
use crate::error::Result;
use crate::models::*;

impl PlexServer {
    /// Home screen hubs across all libraries.
    pub async fn home_hubs(&self, count: u32) -> Result<Vec<Hub>> {
        let p = Params::new().set("count", count).set("includeLibraryPlaylists", 1).set("includeStations", 0);
        Ok(self.get("/hubs", &p).await?.hub)
    }

    /// Promoted hubs (what Plex apps show on the home screen), optionally per section.
    pub async fn promoted_hubs(&self, section_ids: &[&str], count: u32) -> Result<Vec<Hub>> {
        let ids = (!section_ids.is_empty()).then(|| section_ids.join(","));
        let p = Params::new().set("count", count).opt("contentDirectoryID", ids);
        Ok(self.get("/hubs/promoted", &p).await?.hub)
    }

    /// "Continue Watching" (partially watched items plus next episodes).
    pub async fn continue_watching(&self, count: u32) -> Result<Vec<Metadata>> {
        let p = Params::new().set("count", count);
        let hubs = self.get("/hubs/home/continueWatching", &p).await?.hub;
        Ok(hubs.into_iter().flat_map(|h| h.metadata).collect())
    }

    pub async fn home_on_deck(&self, count: u32) -> Result<Vec<Metadata>> {
        let hubs = self.get("/hubs/home/onDeck", &Params::new().set("count", count)).await?.hub;
        Ok(hubs.into_iter().flat_map(|h| h.metadata).collect())
    }

    /// Recently added across libraries; `kind` filters by media type.
    pub async fn home_recently_added(&self, kind: Option<u32>, count: u32) -> Result<Vec<Hub>> {
        let p = Params::new().opt("type", kind).set("count", count);
        Ok(self.get("/hubs/home/recentlyAdded", &p).await?.hub)
    }

    /// Hubs for one library section (recommended view).
    pub async fn section_hubs(&self, section_id: &str, count: u32) -> Result<Vec<Hub>> {
        let p = Params::new().set("count", count).set("includeLibraryPlaylists", 1);
        Ok(self.get(&format!("/hubs/sections/{section_id}"), &p).await?.hub)
    }

    /// Related hubs for an item.
    pub async fn metadata_hubs(&self, rating_key: &str, count: u32) -> Result<Vec<Hub>> {
        let p = Params::new().set("count", count);
        Ok(self.get(&format!("/hubs/metadata/{rating_key}"), &p).await?.hub)
    }

    /// Items of a hub (via its `key`), paginated.
    pub async fn hub_items(&self, hub_key: &str, start: u32, size: u32) -> Result<MediaContainer> {
        let p = Params::new().set("X-Plex-Container-Start", start).set("X-Plex-Container-Size", size);
        self.get(hub_key, &p).await
    }

    /// Search grouped into hubs (movies, shows, episodes, actors, genres, ...).
    pub async fn search_hubs(&self, query: &str, limit: u32, section_id: Option<&str>) -> Result<Vec<Hub>> {
        let p = Params::new()
            .set("query", query)
            .set("limit", limit)
            .set("includeCollections", 1)
            .set("includeExternalMedia", 0)
            .opt("sectionId", section_id);
        Ok(self.get("/hubs/search", &p).await?.hub)
    }

    /// Fuzzier search tuned for spoken queries.
    pub async fn search_voice(&self, query: &str, limit: u32, kind: Option<u32>) -> Result<Vec<Hub>> {
        let p = Params::new().set("query", query).set("limit", limit).opt("type", kind);
        Ok(self.get("/hubs/search/voice", &p).await?.hub)
    }

    /// Legacy flat search.
    pub async fn search(&self, query: &str, kind: Option<u32>) -> Result<Vec<Metadata>> {
        let p = Params::new().set("query", query).opt("type", kind);
        Ok(self.get("/search", &p).await?.metadata)
    }
}
