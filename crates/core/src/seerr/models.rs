//! Seerr's JSON, as far as Patina reads it. Every field is optional: Seerr and
//! TMDB leave many out, and Overseerr, Jellyseerr and Seerr differ slightly.

use patina_plex::de;
use serde::Deserialize;
use serde::de::{DeserializeOwned, Deserializer};

use super::SeerrMediaType;

/// A required number that is `0` when missing or not a number.
fn int<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    Ok(de::opt_i64(d)?.unwrap_or(0))
}

/// `"movie"` or `"tv"`; anything else (people, collections) reads as absent.
fn media_type<'de, D: Deserializer<'de>>(d: D) -> Result<Option<SeerrMediaType>, D::Error> {
    Ok(match de::opt_string(d)?.as_deref() {
        Some("movie") => Some(SeerrMediaType::Movie),
        Some("tv") => Some(SeerrMediaType::Tv),
        _ => None,
    })
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawUser {
    #[serde(default, deserialize_with = "int")]
    pub id: i64,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub email: Option<String>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub username: Option<String>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub plex_username: Option<String>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub jellyfin_username: Option<String>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub display_name: Option<String>,
    #[serde(default, deserialize_with = "de::opt_i64")]
    pub permissions: Option<i64>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub avatar: Option<String>,
}

/// A season of a show in Seerr's media info.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawMediaSeason {
    #[serde(default, deserialize_with = "int")]
    pub season_number: i64,
    #[serde(default, deserialize_with = "de::opt_i64")]
    pub status: Option<i64>,
    #[serde(default, deserialize_with = "de::opt_i64")]
    pub status4k: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawMediaInfo {
    #[serde(default, deserialize_with = "de::opt_i64")]
    pub id: Option<i64>,
    #[serde(default, deserialize_with = "de::opt_i64")]
    pub tmdb_id: Option<i64>,
    #[serde(default, deserialize_with = "de::opt_i64")]
    pub tvdb_id: Option<i64>,
    /// Only on the media of a request.
    #[serde(default, deserialize_with = "media_type")]
    pub media_type: Option<SeerrMediaType>,
    #[serde(default, deserialize_with = "de::opt_i64")]
    pub status: Option<i64>,
    #[serde(default, deserialize_with = "de::opt_i64")]
    pub status4k: Option<i64>,
    /// Plex rating key of the copy on the server, once available.
    #[serde(default, deserialize_with = "de::opt_string")]
    pub rating_key: Option<String>,
    #[serde(default, rename = "ratingKey4k", deserialize_with = "de::opt_string")]
    pub rating_key4k: Option<String>,
    #[serde(default, deserialize_with = "de::one_or_many")]
    pub requests: Vec<RawMediaRequest>,
    #[serde(default, deserialize_with = "de::one_or_many")]
    pub seasons: Vec<RawMediaSeason>,
}

/// A season named by a request.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawRequestSeason {
    #[serde(default, deserialize_with = "int")]
    pub season_number: i64,
    #[serde(default, deserialize_with = "de::opt_i64")]
    pub status: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawMediaRequest {
    #[serde(default, deserialize_with = "int")]
    pub id: i64,
    /// Seerr's `MediaRequestStatus`; missing reads as pending.
    #[serde(default, deserialize_with = "de::opt_i64")]
    pub status: Option<i64>,
    pub media: Option<RawMediaInfo>,
    #[serde(default, rename = "type", deserialize_with = "media_type")]
    pub kind: Option<SeerrMediaType>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub created_at: Option<String>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub updated_at: Option<String>,
    pub requested_by: Option<RawUser>,
    pub modified_by: Option<RawUser>,
    #[serde(default, rename = "is4k", deserialize_with = "de::boolean")]
    pub is4k: bool,
    #[serde(default, deserialize_with = "de::one_or_many")]
    pub seasons: Vec<RawRequestSeason>,
}

/// What Seerr answers to creating a request: the request, or just a message
/// when there was nothing left to request.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawCreatedRequest {
    #[serde(default, deserialize_with = "de::opt_i64")]
    pub status: Option<i64>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub message: Option<String>,
}

/// An entry of a search or discover page; `media_type` is `movie`, `tv`, `person` or `collection`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawResult {
    #[serde(default, deserialize_with = "int")]
    pub id: i64,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub media_type: Option<String>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub release_date: Option<String>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub first_air_date: Option<String>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub overview: Option<String>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub poster_path: Option<String>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub backdrop_path: Option<String>,
    #[serde(default, deserialize_with = "de::opt_f64")]
    pub vote_average: Option<f64>,
    pub media_info: Option<RawMediaInfo>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", bound(deserialize = "T: DeserializeOwned"))]
pub struct RawPage<T> {
    #[serde(default, deserialize_with = "int")]
    pub page: i64,
    #[serde(default, deserialize_with = "int")]
    pub total_pages: i64,
    #[serde(default, deserialize_with = "de::opt_i64")]
    pub total_results: Option<i64>,
    #[serde(default = "Vec::new", deserialize_with = "de::one_or_many")]
    pub results: Vec<T>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawPageInfo {
    #[serde(default, deserialize_with = "int")]
    pub page: i64,
    #[serde(default, deserialize_with = "int")]
    pub pages: i64,
    #[serde(default, deserialize_with = "int")]
    pub results: i64,
    #[serde(default, deserialize_with = "de::opt_i64")]
    pub page_size: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawRequestPage {
    #[serde(default)]
    pub page_info: RawPageInfo,
    #[serde(default, deserialize_with = "de::one_or_many")]
    pub results: Vec<RawMediaRequest>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawGenre {
    #[serde(default, deserialize_with = "int")]
    pub id: i64,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawMovieDetails {
    #[serde(default, deserialize_with = "int")]
    pub id: i64,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub overview: Option<String>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub poster_path: Option<String>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub backdrop_path: Option<String>,
    #[serde(default, deserialize_with = "de::opt_f64")]
    pub vote_average: Option<f64>,
    #[serde(default, deserialize_with = "de::one_or_many")]
    pub genres: Vec<RawGenre>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub tagline: Option<String>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub status: Option<String>,
    pub media_info: Option<RawMediaInfo>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub release_date: Option<String>,
    #[serde(default, deserialize_with = "de::opt_f64")]
    pub runtime: Option<f64>,
}

/// A season in a show's details.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawTvSeason {
    #[serde(default, deserialize_with = "int")]
    pub season_number: i64,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "de::opt_i64")]
    pub episode_count: Option<i64>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub air_date: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawExternalIds {
    #[serde(default, deserialize_with = "de::opt_i64")]
    pub tvdb_id: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawTvDetails {
    #[serde(default, deserialize_with = "int")]
    pub id: i64,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub overview: Option<String>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub poster_path: Option<String>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub backdrop_path: Option<String>,
    #[serde(default, deserialize_with = "de::opt_f64")]
    pub vote_average: Option<f64>,
    #[serde(default, deserialize_with = "de::one_or_many")]
    pub genres: Vec<RawGenre>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub tagline: Option<String>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub status: Option<String>,
    pub media_info: Option<RawMediaInfo>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub first_air_date: Option<String>,
    #[serde(default, deserialize_with = "de::one_or_many")]
    pub episode_run_time: Vec<f64>,
    #[serde(default, deserialize_with = "de::opt_i64")]
    pub number_of_seasons: Option<i64>,
    #[serde(default, deserialize_with = "de::one_or_many")]
    pub seasons: Vec<RawTvSeason>,
    pub external_ids: Option<RawExternalIds>,
}

/// A Radarr or Sonarr server set up in Seerr.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawService {
    #[serde(default, deserialize_with = "int")]
    pub id: i64,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub name: Option<String>,
    #[serde(default, rename = "is4k", deserialize_with = "de::boolean")]
    pub is4k: bool,
    #[serde(default, deserialize_with = "de::boolean")]
    pub is_default: bool,
    #[serde(default, deserialize_with = "de::opt_i64")]
    pub active_profile_id: Option<i64>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub active_directory: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawProfile {
    #[serde(default, deserialize_with = "int")]
    pub id: i64,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawRootFolder {
    #[serde(default, deserialize_with = "int")]
    pub id: i64,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub path: Option<String>,
    #[serde(default, deserialize_with = "de::opt_f64")]
    pub free_space: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawServiceDetails {
    #[serde(default)]
    pub server: RawService,
    #[serde(default, deserialize_with = "de::one_or_many")]
    pub profiles: Vec<RawProfile>,
    #[serde(default, deserialize_with = "de::one_or_many")]
    pub root_folders: Vec<RawRootFolder>,
}

/// `GET /status`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawStatus {
    #[serde(default, deserialize_with = "de::opt_string")]
    pub version: Option<String>,
    #[serde(default, deserialize_with = "de::opt_string")]
    pub commit_tag: Option<String>,
}
