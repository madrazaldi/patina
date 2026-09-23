use reqwest::Method;

use super::PlexServer;
use crate::client::Params;
use crate::error::Result;
use crate::models::*;

/// Query for browsing a library section.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LibraryQuery {
    /// Plex media type id (see [`crate::MediaType`]); e.g. episodes of a show library.
    pub kind: Option<u32>,
    /// Sort key, e.g. `titleSort`, `addedAt:desc`, `year:desc`, `rating:desc`.
    pub sort: Option<String>,
    pub unwatched: bool,
    /// Free-form filters: `genre=12`, `year=1999`, `title=alien`, `resolution=4k`...
    pub filters: Params,
    pub start: u32,
    pub size: u32,
}

impl LibraryQuery {
    pub fn page(start: u32, size: u32) -> Self {
        Self { start, size, ..Default::default() }
    }

    pub fn params(&self) -> Params {
        let mut p = Params::new()
            .opt("type", self.kind)
            .opt("sort", self.sort.clone())
            .flag("unwatched", self.unwatched)
            .extend(&self.filters)
            .set("includeGuids", 1)
            .set("X-Plex-Container-Start", self.start);
        if self.size > 0 {
            p.push("X-Plex-Container-Size", self.size);
        }
        p
    }
}

impl PlexServer {
    /// All library sections.
    pub async fn sections(&self) -> Result<Vec<Directory>> {
        Ok(self.get("/library/sections", &Params::new()).await?.directory)
    }

    /// Section details, including the browse sub-directories (unwatched, genre, year, ...).
    pub async fn section(&self, id: &str) -> Result<MediaContainer> {
        self.get(&format!("/library/sections/{id}"), &Params::new().set("includeDetails", 1)).await
    }

    /// Browse a section's items.
    pub async fn section_items(&self, id: &str, query: &LibraryQuery) -> Result<MediaContainer> {
        self.get(&format!("/library/sections/{id}/all"), &query.params()).await
    }

    /// Any section sub-listing: `unwatched`, `newest`, `recentlyAdded`, `onDeck`,
    /// `recentlyViewed`, `genre`, `year`, `decade`, `director`, `actor`,
    /// `contentRating`, `resolution`, `firstCharacter`, `collection`, `folder`, ...
    pub async fn section_listing(&self, id: &str, listing: &str, params: &Params) -> Result<MediaContainer> {
        self.get(&format!("/library/sections/{id}/{listing}"), params).await
    }

    pub async fn section_filters(&self, id: &str) -> Result<Vec<Directory>> {
        Ok(self.section_listing(id, "filters", &Params::new()).await?.directory)
    }

    pub async fn section_sorts(&self, id: &str) -> Result<Vec<Directory>> {
        Ok(self.section_listing(id, "sorts", &Params::new()).await?.directory)
    }

    /// Values of a filter (e.g. all genres) as `Directory` entries whose `key` is the filter value.
    pub async fn section_filter_values(&self, id: &str, filter: &str, kind: Option<u32>) -> Result<Vec<Directory>> {
        Ok(self.section_listing(id, filter, &Params::new().opt("type", kind)).await?.directory)
    }

    /// Types, filters, sorts and editable fields for a section (`includeMeta=1`).
    pub async fn section_meta(&self, id: &str) -> Result<Option<LibraryMeta>> {
        let params = Params::new().set("includeMeta", 1).set("X-Plex-Container-Size", 0);
        Ok(self.get(&format!("/library/sections/{id}/all"), &params).await?.meta)
    }

    pub async fn section_first_characters(&self, id: &str, kind: Option<u32>) -> Result<Vec<Directory>> {
        Ok(self.section_listing(id, "firstCharacter", &Params::new().opt("type", kind)).await?.directory)
    }

    pub async fn section_on_deck(&self, id: &str) -> Result<Vec<Metadata>> {
        Ok(self.section_listing(id, "onDeck", &Params::new()).await?.metadata)
    }

    pub async fn section_recently_added(&self, id: &str, size: u32) -> Result<Vec<Metadata>> {
        let p = Params::new().set("X-Plex-Container-Start", 0).set("X-Plex-Container-Size", size);
        Ok(self.section_listing(id, "recentlyAdded", &p).await?.metadata)
    }

    /// Browse a section by folder. `parent` is a key from a previous call.
    pub async fn section_folders(&self, id: &str, parent: Option<&str>) -> Result<MediaContainer> {
        match parent {
            Some(key) => self.get(key, &Params::new()).await,
            None => self.section_listing(id, "folder", &Params::new()).await,
        }
    }

    // ----- Library-wide ---------------------------------------------------

    pub async fn on_deck(&self) -> Result<Vec<Metadata>> {
        Ok(self.get("/library/onDeck", &Params::new()).await?.metadata)
    }

    pub async fn recently_added(&self, size: u32) -> Result<Vec<Metadata>> {
        let p = Params::new().set("X-Plex-Container-Start", 0).set("X-Plex-Container-Size", size);
        Ok(self.get("/library/recentlyAdded", &p).await?.metadata)
    }

    /// Search all libraries with arbitrary filters (`/library/all`).
    pub async fn library_all(&self, params: &Params) -> Result<MediaContainer> {
        self.get("/library/all", params).await
    }

    /// Find the local copy of a plex:// (or imdb://, tmdb://, tvdb://) guid.
    pub async fn find_by_guid(&self, guid: &str) -> Result<Vec<Metadata>> {
        Ok(self.library_all(&Params::new().set("guid", guid)).await?.metadata)
    }

    /// Every tag of a given kind (`genre`, `actor`, ...), optionally for one type.
    pub async fn tags(&self, tag_type: u32) -> Result<Vec<Directory>> {
        Ok(self.get("/library/tags", &Params::new().set("type", tag_type)).await?.directory)
    }

    // ----- Maintenance ----------------------------------------------------

    /// Scan a section for new files. `force` re-reads metadata; `path` limits to a subfolder.
    pub async fn refresh_section(&self, id: &str, force: bool, path: Option<&str>) -> Result<()> {
        let p = Params::new().flag("force", force).opt("path", path);
        self.empty(Method::GET, &format!("/library/sections/{id}/refresh"), &p).await
    }

    pub async fn cancel_refresh_section(&self, id: &str) -> Result<()> {
        self.delete(&format!("/library/sections/{id}/refresh"), &Params::new()).await
    }

    pub async fn refresh_all_sections(&self, force: bool) -> Result<()> {
        self.empty(Method::GET, "/library/sections/all/refresh", &Params::new().flag("force", force)).await
    }

    pub async fn empty_trash(&self, id: &str) -> Result<()> {
        self.put(&format!("/library/sections/{id}/emptyTrash"), &Params::new()).await
    }

    pub async fn analyze_section(&self, id: &str) -> Result<()> {
        self.put(&format!("/library/sections/{id}/analyze"), &Params::new()).await
    }

    pub async fn clean_bundles(&self) -> Result<()> {
        self.put("/library/clean/bundles", &Params::new()).await
    }

    pub async fn optimize_database(&self) -> Result<()> {
        self.put("/library/optimize", &Params::new().set("async", 1)).await
    }

    /// Create a library. Params: `name`, `type` (movie/show/artist/photo), `agent`,
    /// `scanner`, `language`, `location` (repeatable), plus `prefs[...]`.
    pub async fn create_section(&self, params: &Params) -> Result<MediaContainer> {
        self.container(Method::POST, "/library/sections", params).await
    }

    pub async fn edit_section(&self, id: &str, params: &Params) -> Result<()> {
        self.put(&format!("/library/sections/{id}"), params).await
    }

    pub async fn delete_section(&self, id: &str) -> Result<()> {
        self.delete(&format!("/library/sections/{id}"), &Params::new()).await
    }

    /// Per-section preferences.
    pub async fn section_prefs(&self, id: &str) -> Result<Vec<Setting>> {
        Ok(self.get(&format!("/library/sections/{id}/prefs"), &Params::new()).await?.setting)
    }

    pub async fn set_section_prefs(&self, id: &str, params: &Params) -> Result<()> {
        self.put(&format!("/library/sections/{id}/prefs"), params).await
    }

    /// Available metadata agents for a media type.
    pub async fn agents(&self, media_type: Option<u32>) -> Result<MediaContainer> {
        self.get("/system/agents", &Params::new().opt("mediaType", media_type)).await
    }

    /// Available scanners for a media type.
    pub async fn scanners(&self, media_type: u32) -> Result<MediaContainer> {
        self.get("/system/scanners", &Params::new().set("type", media_type)).await
    }
}
