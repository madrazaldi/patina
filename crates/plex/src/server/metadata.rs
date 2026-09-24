use reqwest::Method;

use super::PlexServer;
use crate::client::Params;
use crate::error::Result;
use crate::models::*;

/// Optional sections to include when fetching an item.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MetadataOptions {
    pub children: bool,
    pub extras: bool,
    pub related: bool,
    pub markers: bool,
    pub chapters: bool,
    pub on_deck: bool,
    pub reviews: bool,
    pub concerts: bool,
    /// Radio stations of an artist (or album / track).
    pub stations: bool,
}

impl MetadataOptions {
    /// Everything a detail page needs.
    pub fn full() -> Self {
        Self { children: false, extras: true, related: true, markers: true, chapters: true, on_deck: true, reviews: true, concerts: false, stations: false }
    }

    /// What the player needs: markers and chapters.
    pub fn playback() -> Self {
        Self { markers: true, chapters: true, ..Default::default() }
    }

    fn params(&self) -> Params {
        Params::new()
            .flag("includeChildren", self.children)
            .flag("includeExtras", self.extras)
            .flag("includeRelated", self.related)
            .flag("includeMarkers", self.markers)
            .flag("includeChapters", self.chapters)
            .flag("includeOnDeck", self.on_deck)
            .flag("includeReviews", self.reviews)
            .flag("includeConcerts", self.concerts)
            .flag("includeStations", self.stations)
            .set("includeGuids", 1)
            .set("asyncAugmentMetadata", 0)
    }
}

impl PlexServer {
    pub async fn metadata(&self, rating_key: &str, opts: MetadataOptions) -> Result<Option<Metadata>> {
        let mc = self.get(&format!("/library/metadata/{rating_key}"), &opts.params()).await?;
        Ok(mc.metadata.into_iter().next())
    }

    /// Several items in one request.
    pub async fn metadata_many(&self, rating_keys: &[&str]) -> Result<Vec<Metadata>> {
        if rating_keys.is_empty() {
            return Ok(Vec::new());
        }
        Ok(self.get(&format!("/library/metadata/{}", rating_keys.join(",")), &Params::new()).await?.metadata)
    }

    /// Direct children: seasons of a show, episodes of a season, tracks of an album.
    pub async fn children(&self, rating_key: &str) -> Result<MediaContainer> {
        self.get(&format!("/library/metadata/{rating_key}/children"), &Params::new().set("includeGuids", 1))
            .await
    }

    /// All leaves: every episode of a show, every track of an artist.
    pub async fn all_leaves(&self, rating_key: &str) -> Result<MediaContainer> {
        self.get(&format!("/library/metadata/{rating_key}/allLeaves"), &Params::new()).await
    }

    /// Every leaf under an item, with full stream details. `allLeaves` leaves
    /// streams out, so the items are fetched again in batches; any the batch
    /// endpoint still returns without streams are fetched one by one.
    pub async fn all_leaves_with_streams(&self, rating_key: &str) -> Result<Vec<Metadata>> {
        use futures::{StreamExt, TryStreamExt};

        let keys: Vec<String> = self.all_leaves(rating_key).await?.metadata.iter().map(|m| m.rating_key().to_string()).collect();
        // Owned chunks keep the future `Send` (a closure over borrowed slices
        // trips the compiler's higher-ranked lifetime check).
        let chunks: Vec<Vec<String>> = keys.chunks(40).map(<[String]>::to_vec).collect();
        let batches: Vec<Vec<Metadata>> = futures::stream::iter(chunks)
            .map(|chunk| async move {
                let refs: Vec<&str> = chunk.iter().map(String::as_str).collect();
                self.metadata_many(&refs).await
            })
            .buffered(4)
            .try_collect()
            .await?;
        let mut items: Vec<Metadata> = futures::stream::iter(batches.into_iter().flatten())
            .map(|item| async move {
                let missing = item.media.iter().flat_map(|m| &m.parts).any(|p| p.streams.is_empty());
                if !missing {
                    return Ok::<_, crate::Error>(item);
                }
                Ok(self.metadata(item.rating_key(), MetadataOptions::default()).await?.unwrap_or(item))
            })
            .buffered(8)
            .try_collect()
            .await?;
        // Keep allLeaves order even if the batch endpoint reorders.
        items.sort_by_key(|m| keys.iter().position(|k| k == m.rating_key()));
        Ok(items)
    }

    pub async fn grandchildren(&self, rating_key: &str) -> Result<MediaContainer> {
        self.get(&format!("/library/metadata/{rating_key}/grandchildren"), &Params::new()).await
    }

    /// "More like this" and related hubs.
    pub async fn related(&self, rating_key: &str) -> Result<Vec<Hub>> {
        Ok(self.get(&format!("/library/metadata/{rating_key}/related"), &Params::new()).await?.hub)
    }

    pub async fn similar(&self, rating_key: &str) -> Result<Vec<Metadata>> {
        Ok(self.get(&format!("/library/metadata/{rating_key}/similar"), &Params::new()).await?.metadata)
    }

    /// Trailers, featurettes, deleted scenes, ...
    pub async fn extras(&self, rating_key: &str) -> Result<Vec<Metadata>> {
        Ok(self.get(&format!("/library/metadata/{rating_key}/extras"), &Params::new()).await?.metadata)
    }

    /// Items surrounding an item in its parent (e.g. neighbouring episodes).
    pub async fn nearest(&self, rating_key: &str) -> Result<Vec<Metadata>> {
        Ok(self.get(&format!("/library/metadata/{rating_key}/nearest"), &Params::new()).await?.metadata)
    }

    // ----- Watch state & ratings -----------------------------------------

    pub async fn scrobble(&self, rating_key: &str) -> Result<()> {
        self.empty(Method::GET, "/:/scrobble", &lib_key(rating_key)).await
    }

    pub async fn unscrobble(&self, rating_key: &str) -> Result<()> {
        self.empty(Method::GET, "/:/unscrobble", &lib_key(rating_key)).await
    }

    /// Rate on a 0–10 scale; `None` clears the rating.
    pub async fn rate(&self, rating_key: &str, rating: Option<f64>) -> Result<()> {
        let p = lib_key(rating_key).set("rating", rating.unwrap_or(-1.0));
        self.put("/:/rate", &p).await
    }

    /// Hide an item from Continue Watching / On Deck until it is played again.
    pub async fn remove_from_continue_watching(&self, rating_key: &str) -> Result<()> {
        self.put("/actions/removeFromContinueWatching", &Params::new().set("ratingKey", rating_key)).await
    }

    /// The first radio station of an artist, album or track (sonic analysis required).
    pub async fn station(&self, rating_key: &str) -> Result<Option<Metadata>> {
        let opts = MetadataOptions { stations: true, ..Default::default() };
        Ok(self.metadata(rating_key, opts).await?.and_then(|m| m.stations.into_iter().next()))
    }

    /// Store a resume position without a playback session.
    pub async fn set_progress(&self, rating_key: &str, time_ms: i64, state: &str) -> Result<()> {
        let p = lib_key(rating_key).set("time", time_ms).set("state", state);
        self.empty(Method::GET, "/:/progress", &p).await
    }

    // ----- Maintenance & editing -----------------------------------------

    pub async fn refresh_metadata(&self, rating_key: &str, force: bool) -> Result<()> {
        self.put(&format!("/library/metadata/{rating_key}/refresh"), &Params::new().flag("force", force)).await
    }

    pub async fn analyze_metadata(&self, rating_key: &str) -> Result<()> {
        self.put(&format!("/library/metadata/{rating_key}/analyze"), &Params::new()).await
    }

    /// Delete an item and its files from disk (requires the server setting to allow it).
    pub async fn delete_metadata(&self, rating_key: &str) -> Result<()> {
        self.delete(&format!("/library/metadata/{rating_key}"), &Params::new()).await
    }

    /// Edit fields: pairs like `("title", "New")`, `("summary", "...")`.
    /// Each field is written as `<field>.value` and optionally locked.
    /// Uses the section endpoint when `section_id` is known (works on all server versions).
    pub async fn edit_metadata(
        &self,
        rating_key: &str,
        section_id: Option<i64>,
        kind: u32,
        fields: &[(&str, &str)],
        lock: bool,
    ) -> Result<()> {
        let mut p = Params::new().set("type", kind).set("id", rating_key);
        for (field, value) in fields {
            p.push(format!("{field}.value"), value);
            if lock {
                p.push(format!("{field}.locked"), 1);
            }
        }
        match section_id {
            Some(section) => self.put(&format!("/library/sections/{section}/all"), &p).await,
            None => self.put(&format!("/library/metadata/{rating_key}"), &p).await,
        }
    }

    /// Lock or unlock fields (`title`, `summary`, `label`, ...) without changing
    /// their values; locked fields are kept when agents refresh the item.
    pub async fn lock_fields(&self, rating_key: &str, section_id: Option<i64>, kind: u32, fields: &[(&str, bool)]) -> Result<()> {
        let mut p = Params::new().set("type", kind).set("id", rating_key);
        for (field, locked) in fields {
            p.push(format!("{field}.locked"), *locked as u8);
        }
        match section_id {
            Some(section) => self.put(&format!("/library/sections/{section}/all"), &p).await,
            None => self.put(&format!("/library/metadata/{rating_key}"), &p).await,
        }
    }

    /// Add or remove tags (genre, collection, label, ...).
    pub async fn edit_tags(&self, rating_key: &str, kind: u32, tag: &str, add: &[&str], remove: &[&str]) -> Result<()> {
        let mut p = Params::new().set("type", kind).set("id", rating_key);
        for (i, v) in add.iter().enumerate() {
            p.push(format!("{tag}[{i}].tag.tag"), v);
        }
        if !remove.is_empty() {
            p.push(format!("{tag}[].tag.tag-"), remove.join(","));
        }
        self.put(&format!("/library/metadata/{rating_key}"), &p).await
    }

    /// Item-level preferences (e.g. show episode ordering, collection mode).
    pub async fn set_metadata_prefs(&self, rating_key: &str, params: &Params) -> Result<()> {
        self.put(&format!("/library/metadata/{rating_key}/prefs"), params).await
    }

    // ----- Matching --------------------------------------------------------

    pub async fn matches(&self, rating_key: &str, title: Option<&str>, year: Option<i64>, agent: Option<&str>) -> Result<Vec<MatchResult>> {
        let p = Params::new().set("manual", 1).opt("title", title).opt("year", year).opt("agent", agent);
        Ok(self.get(&format!("/library/metadata/{rating_key}/matches"), &p).await?.search_result)
    }

    pub async fn apply_match(&self, rating_key: &str, guid: &str, name: Option<&str>) -> Result<()> {
        let p = Params::new().set("guid", guid).opt("name", name);
        self.put(&format!("/library/metadata/{rating_key}/match"), &p).await
    }

    pub async fn unmatch(&self, rating_key: &str) -> Result<()> {
        self.put(&format!("/library/metadata/{rating_key}/unmatch"), &Params::new()).await
    }

    // ----- Artwork ---------------------------------------------------------

    pub async fn posters(&self, rating_key: &str) -> Result<Vec<Metadata>> {
        Ok(self.get(&format!("/library/metadata/{rating_key}/posters"), &Params::new()).await?.metadata)
    }

    pub async fn arts(&self, rating_key: &str) -> Result<Vec<Metadata>> {
        Ok(self.get(&format!("/library/metadata/{rating_key}/arts"), &Params::new()).await?.metadata)
    }

    /// Select a poster by its URL/key from [`Self::posters`].
    pub async fn set_poster(&self, rating_key: &str, url: &str) -> Result<()> {
        self.put(&format!("/library/metadata/{rating_key}/poster"), &Params::new().set("url", url)).await
    }

    pub async fn set_art(&self, rating_key: &str, url: &str) -> Result<()> {
        self.put(&format!("/library/metadata/{rating_key}/art"), &Params::new().set("url", url)).await
    }

    // ----- Streams & subtitles ---------------------------------------------

    /// Persist the preferred audio / subtitle stream for a part.
    /// `subtitle = Some(0)` disables subtitles.
    pub async fn select_streams(&self, part_id: i64, audio: Option<i64>, subtitle: Option<i64>, all_parts: bool) -> Result<()> {
        let p = Params::new()
            .opt("audioStreamID", audio)
            .opt("subtitleStreamID", subtitle)
            .flag("allParts", all_parts);
        self.put(&format!("/library/parts/{part_id}"), &p).await
    }

    /// Search online subtitle providers for an item.
    pub async fn search_subtitles(&self, rating_key: &str, language: &str, hearing_impaired: bool, forced: bool) -> Result<Vec<Stream>> {
        let p = Params::new()
            .set("language", language)
            .flag("hearingImpaired", hearing_impaired)
            .flag("forced", forced);
        let v = self.raw(Method::GET, &format!("/library/metadata/{rating_key}/subtitles"), &p).await?;
        let list = v.pointer("/MediaContainer/Stream").cloned().unwrap_or_default();
        Ok(serde_json::from_value(list).unwrap_or_default())
    }

    /// Download an online subtitle found with [`Self::search_subtitles`].
    pub async fn download_subtitle(&self, rating_key: &str, key: &str) -> Result<()> {
        self.put(&format!("/library/metadata/{rating_key}/subtitles"), &Params::new().set("key", key)).await
    }

    pub async fn delete_stream(&self, stream_id: i64) -> Result<()> {
        self.delete(&format!("/library/streams/{stream_id}"), &Params::new()).await
    }

    /// Direct URL for a part (the original file), with the token embedded.
    pub fn part_url(&self, part_key: &str, download: bool) -> Result<url::Url> {
        self.authed_url(part_key, &Params::new().flag("download", download))
    }
}

fn lib_key(rating_key: &str) -> Params {
    Params::new().set("key", rating_key).set("identifier", "com.plexapp.plugins.library")
}
