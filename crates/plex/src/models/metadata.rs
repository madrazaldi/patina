use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::*;

/// A metadata item: movie, show, season, episode, artist, album, track, photo,
/// clip, playlist, collection, history entry, or live session.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub rating_key: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub key: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub guid: Option<String>,
    #[serde(rename = "type", default, deserialize_with = "crate::de::opt_string")]
    pub kind: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub subtype: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub title_sort: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub original_title: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub edition_title: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub studio: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub content_rating: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub summary: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub tagline: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub year: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub index: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub parent_index: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_f64")]
    pub rating: Option<f64>,
    #[serde(default, deserialize_with = "crate::de::opt_f64")]
    pub audience_rating: Option<f64>,
    #[serde(default, deserialize_with = "crate::de::opt_f64")]
    pub user_rating: Option<f64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub rating_image: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub audience_rating_image: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub view_count: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub skip_count: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub view_offset: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub last_viewed_at: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub last_rated_at: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub added_at: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub updated_at: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub originally_available_at: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub duration: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub thumb: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub art: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub banner: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub theme: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub composite: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub chapter_source: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub primary_extra_key: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub extra_type: Option<i64>,

    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub parent_rating_key: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub parent_key: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub parent_guid: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub parent_title: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub parent_thumb: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub parent_art: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub parent_theme: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub parent_year: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub parent_studio: Option<String>,

    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub grandparent_rating_key: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub grandparent_key: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub grandparent_guid: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub grandparent_title: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub grandparent_thumb: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub grandparent_art: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub grandparent_theme: Option<String>,

    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub leaf_count: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub viewed_leaf_count: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub child_count: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub season_count: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub album_count: Option<i64>,

    #[serde(rename = "librarySectionID", default, deserialize_with = "crate::de::opt_i64")]
    pub library_section_id: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub library_section_title: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub library_section_key: Option<String>,

    // Playlists & collections.
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub playlist_type: Option<String>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub smart: bool,
    #[serde(rename = "playlistItemID", default, deserialize_with = "crate::de::opt_i64")]
    pub playlist_item_id: Option<i64>,
    #[serde(rename = "playQueueItemID", default, deserialize_with = "crate::de::opt_i64")]
    pub play_queue_item_id: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub min_year: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub max_year: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub collection_mode: Option<i64>,

    // History entries.
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub history_key: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub viewed_at: Option<i64>,
    #[serde(rename = "accountID", default, deserialize_with = "crate::de::opt_i64")]
    pub account_id: Option<i64>,
    #[serde(rename = "deviceID", default, deserialize_with = "crate::de::opt_i64")]
    pub device_id: Option<i64>,

    // Watchlist / discover.
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub watchlisted_at: Option<i64>,

    // Live sessions (/status/sessions).
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub session_key: Option<String>,
    #[serde(rename = "User", default)]
    pub user: Option<SessionUser>,
    #[serde(rename = "Player", default)]
    pub player: Option<Player>,
    #[serde(rename = "Session", default)]
    pub session: Option<SessionInfo>,
    #[serde(rename = "TranscodeSession", default)]
    pub transcode_session: Option<TranscodeSession>,

    #[serde(rename = "Media", default)]
    pub media: Vec<Media>,
    #[serde(rename = "Genre", default)]
    pub genres: Vec<Tag>,
    #[serde(rename = "Director", default)]
    pub directors: Vec<Tag>,
    #[serde(rename = "Writer", default)]
    pub writers: Vec<Tag>,
    #[serde(rename = "Producer", default)]
    pub producers: Vec<Tag>,
    #[serde(rename = "Country", default)]
    pub countries: Vec<Tag>,
    #[serde(rename = "Collection", default)]
    pub collections: Vec<Tag>,
    #[serde(rename = "Label", default)]
    pub labels: Vec<Tag>,
    #[serde(rename = "Mood", default)]
    pub moods: Vec<Tag>,
    #[serde(rename = "Style", default)]
    pub styles: Vec<Tag>,
    #[serde(rename = "Similar", default)]
    pub similar: Vec<Tag>,
    #[serde(rename = "Role", default)]
    pub roles: Vec<Tag>,
    #[serde(rename = "Guid", default)]
    pub guids: Vec<GuidRef>,
    #[serde(rename = "Marker", default)]
    pub markers: Vec<Marker>,
    #[serde(rename = "Chapter", default)]
    pub chapters: Vec<Chapter>,
    #[serde(rename = "Image", default)]
    pub images: Vec<Image>,
    #[serde(rename = "Rating", default)]
    pub ratings: Vec<RatingSource>,
    #[serde(rename = "Review", default)]
    pub reviews: Vec<Review>,
    #[serde(rename = "UltraBlurColors", default)]
    pub ultra_blur_colors: Option<UltraBlurColors>,
    #[serde(rename = "Extras", default)]
    pub extras: Option<Nested>,
    #[serde(rename = "Related", default)]
    pub related: Option<Nested>,
    #[serde(rename = "OnDeck", default)]
    pub on_deck: Option<Nested>,
    #[serde(rename = "Children", default)]
    pub children: Option<Nested>,
    /// Radio stations (`includeStations=1`); each is a playlist-like item whose key feeds a play queue.
    #[serde(rename = "Stations", default, deserialize_with = "crate::de::one_or_many")]
    pub stations: Vec<Metadata>,

    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Key under which Patina tags an item with the server it came from (see [`Metadata::server_id`]).
pub const SERVER_TAG: &str = "patinaServer";

impl Metadata {
    /// The machine identifier of the server this item came from, when it is not the
    /// session's own server. Patina sets it on items found on other servers so that
    /// navigation, playback and reporting stay with the right one. Plex never sends it.
    pub fn server_id(&self) -> Option<&str> {
        self.extra.get(SERVER_TAG).and_then(Value::as_str).filter(|s| !s.is_empty())
    }

    /// Tag the item with the server it lives on (`None` clears the tag).
    pub fn set_server_id(&mut self, server: Option<&str>) {
        match server {
            Some(id) => {
                self.extra.insert(SERVER_TAG.into(), Value::String(id.into()));
            }
            None => {
                self.extra.remove(SERVER_TAG);
            }
        }
    }

    pub fn rating_key(&self) -> &str {
        self.rating_key.as_deref().unwrap_or("")
    }

    pub fn title(&self) -> &str {
        self.title.as_deref().unwrap_or("")
    }

    pub fn kind(&self) -> &str {
        self.kind.as_deref().unwrap_or("")
    }

    /// Whether this item can be handed to a player directly.
    pub fn is_playable(&self) -> bool {
        matches!(self.kind(), "movie" | "episode" | "track" | "clip" | "trailer" | "photo")
            || (!self.media.is_empty() && !matches!(self.kind(), "show" | "season" | "artist" | "album"))
    }

    /// Whether this item is a container of other items (show, season, album, ...).
    pub fn is_container(&self) -> bool {
        matches!(
            self.kind(),
            "show" | "season" | "artist" | "album" | "collection" | "playlist" | "photoalbum"
        )
    }

    pub fn is_watched(&self) -> bool {
        match (self.leaf_count, self.viewed_leaf_count) {
            (Some(total), Some(seen)) if total > 0 => seen >= total,
            _ => self.view_count.unwrap_or(0) > 0 && self.view_offset.unwrap_or(0) == 0,
        }
    }

    pub fn unwatched_leaves(&self) -> Option<i64> {
        Some(self.leaf_count? - self.viewed_leaf_count.unwrap_or(0))
    }

    /// Resume progress in `0.0..=1.0`, if partially watched.
    pub fn progress(&self) -> Option<f64> {
        let offset = self.view_offset? as f64;
        let duration = self.duration? as f64;
        (duration > 0.0 && offset > 0.0).then(|| (offset / duration).clamp(0.0, 1.0))
    }

    /// Best available image for a poster-shaped card.
    pub fn poster(&self) -> Option<&str> {
        match self.kind() {
            "episode" => self.grandparent_thumb.as_deref().or(self.parent_thumb.as_deref()).or(self.thumb.as_deref()),
            "season" => self.thumb.as_deref().or(self.parent_thumb.as_deref()),
            "playlist" => self.composite.as_deref().or(self.thumb.as_deref()),
            _ => self.thumb.as_deref().or(self.composite.as_deref()).or(self.parent_thumb.as_deref()),
        }
    }

    /// Best available background art.
    pub fn backdrop(&self) -> Option<&str> {
        self.art
            .as_deref()
            .or(self.grandparent_art.as_deref())
            .or(self.parent_art.as_deref())
            .or_else(|| self.image_of_type("background"))
    }

    pub fn image_of_type(&self, kind: &str) -> Option<&str> {
        self.images.iter().find(|i| i.kind.as_deref() == Some(kind)).and_then(|i| i.url.as_deref())
    }

    /// Title including show/season context, e.g. "Show — S01E02 · Pilot".
    pub fn full_title(&self) -> String {
        match self.kind() {
            "episode" => format!(
                "{} — {} · {}",
                self.grandparent_title.as_deref().unwrap_or(""),
                self.episode_code(),
                self.title()
            ),
            "season" => format!("{} — {}", self.parent_title.as_deref().unwrap_or(""), self.title()),
            "track" => format!("{} — {}", self.grandparent_title.as_deref().unwrap_or(""), self.title()),
            _ => match self.year {
                Some(y) => format!("{} ({y})", self.title()),
                None => self.title().to_string(),
            },
        }
    }

    /// "S01E02" style code for episodes.
    pub fn episode_code(&self) -> String {
        match (self.parent_index, self.index) {
            (Some(s), Some(e)) => format!("S{s:02}E{e:02}"),
            (None, Some(e)) => format!("E{e:02}"),
            _ => String::new(),
        }
    }

    /// The plex.tv discover id derived from a `plex://type/<id>` guid.
    pub fn discover_id(&self) -> Option<&str> {
        let guid = self.guid.as_deref()?;
        guid.strip_prefix("plex://")?.rsplit('/').next()
    }

    pub fn markers_of(&self, kind: &str) -> impl Iterator<Item = &Marker> {
        self.markers.iter().filter(move |m| m.kind.as_deref() == Some(kind))
    }

    pub fn extras(&self) -> &[Metadata] {
        self.extras.as_ref().map(|e| e.metadata.as_slice()).unwrap_or(&[])
    }

    pub fn related_hubs(&self) -> &[Hub] {
        self.related.as_ref().map(|e| e.hub.as_slice()).unwrap_or(&[])
    }

    pub fn on_deck_item(&self) -> Option<&Metadata> {
        self.on_deck.as_ref().and_then(|d| d.metadata.first())
    }
}

/// Nested `{ "Metadata": [...], "Hub": [...] }` blocks (Extras, Related, OnDeck).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Nested {
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub size: Option<i64>,
    #[serde(rename = "Metadata", default, deserialize_with = "crate::de::one_or_many")]
    pub metadata: Vec<Metadata>,
    #[serde(rename = "Hub", default, deserialize_with = "crate::de::one_or_many")]
    pub hub: Vec<Hub>,
}

/// A tag reference: genre, director, actor (role), collection, label, ...
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub id: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub tag: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub tag_key: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub filter: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub role: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub thumb: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub count: Option<i64>,
}

impl Tag {
    pub fn name(&self) -> &str {
        self.tag.as_deref().unwrap_or("")
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GuidRef {
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub id: Option<String>,
}

/// Intro / credits / commercial markers.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Marker {
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub id: Option<i64>,
    #[serde(rename = "type", default, deserialize_with = "crate::de::opt_string")]
    pub kind: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub start_time_offset: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub end_time_offset: Option<i64>,
    #[serde(rename = "final", default, deserialize_with = "crate::de::boolean")]
    pub is_final: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Chapter {
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub id: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub index: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub tag: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub start_time_offset: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub end_time_offset: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub thumb: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Image {
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub alt: Option<String>,
    #[serde(rename = "type", default, deserialize_with = "crate::de::opt_string")]
    pub kind: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RatingSource {
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub image: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_f64")]
    pub value: Option<f64>,
    #[serde(rename = "type", default, deserialize_with = "crate::de::opt_string")]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Review {
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub tag: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub text: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub image: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub source: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub link: Option<String>,
}

/// Four corner colours (hex, no `#`) for Plex's "ultra blur" backgrounds.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UltraBlurColors {
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub top_left: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub top_right: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub bottom_right: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub bottom_left: Option<String>,
}

/// Match candidate from `/library/metadata/{id}/matches`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MatchResult {
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub guid: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub score: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub year: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub thumb: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub summary: Option<String>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub matched: bool,
}
