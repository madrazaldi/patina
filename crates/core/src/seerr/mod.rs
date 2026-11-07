//! Seerr: request movies and shows the server doesn't have yet, and follow or
//! manage requests. The instance is set in `[seerr]` of config.toml; with no API
//! key Patina signs in with the Plex account. Front ends get the shapes below,
//! never the key or the session.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::future::Future;
use std::str::FromStr;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use patina_plex::{Metadata, MetadataOptions, PlexServer};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

pub mod client;
pub mod models;
#[cfg(test)]
mod testing;

pub use client::{
    CreateRequest, RequestVerdict, RequestsQuery, SeerrAuth, SeerrAuthKind, SeerrClient, ServiceKind, encode_query, normalize_url,
};
pub use models::{RawMediaInfo, RawMediaRequest, RawMovieDetails, RawResult, RawTvDetails, RawUser};

use models::{RawCreatedRequest, RawRequestSeason};

/// What went wrong talking to Seerr, in words for the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeerrError {
    message: String,
    status: Option<u16>,
}

impl SeerrError {
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into(), status: None }
    }

    pub fn with_status(message: impl Into<String>, status: u16) -> Self {
        Self { message: message.into(), status: Some(status) }
    }

    /// HTTP status, when Seerr answered.
    pub fn status(&self) -> Option<u16> {
        self.status
    }
}

impl fmt::Display for SeerrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SeerrError {}

pub type Result<T, E = SeerrError> = std::result::Result<T, E>;

/// Where a title stands in Seerr (its `MediaStatus`); `None` when nobody has requested it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Availability {
    Unknown,
    Pending,
    Processing,
    Partial,
    Available,
    Blocklisted,
    Deleted,
}

/// A request's own state (`MediaRequestStatus`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RequestState {
    Pending,
    Approved,
    Declined,
    Failed,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SeerrMediaType {
    Movie,
    Tv,
}

impl SeerrMediaType {
    /// `movie` or `tv`, as Seerr's URLs and API spell it.
    pub fn as_str(self) -> &'static str {
        match self {
            SeerrMediaType::Movie => "movie",
            SeerrMediaType::Tv => "tv",
        }
    }
}

impl FromStr for SeerrMediaType {
    type Err = SeerrError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "movie" => Ok(SeerrMediaType::Movie),
            "tv" => Ok(SeerrMediaType::Tv),
            other => Err(SeerrError::new(format!("Unknown media type {other}."))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeerrTitle {
    pub media_type: SeerrMediaType,
    pub tmdb_id: i64,
    pub title: String,
    pub year: Option<i32>,
    pub overview: String,
    /// TMDB poster URL.
    pub poster: Option<String>,
    /// TMDB backdrop URL.
    pub backdrop: Option<String>,
    /// TMDB vote average, 0-10.
    pub rating: Option<f64>,
    pub status: Option<Availability>,
    pub status_4k: Option<Availability>,
    /// Rating key of the copy on the Plex server Seerr syncs with, once available
    /// (checked against this server in [`details`]).
    pub rating_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeerrSeason {
    pub number: i64,
    pub name: String,
    pub episodes: Option<i64>,
    pub air_date: Option<String>,
    pub status: Option<Availability>,
    pub status_4k: Option<Availability>,
    /// State of the newest open request covering it, if any.
    pub requested: Option<RequestState>,
    pub requested_4k: Option<RequestState>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeerrRequest {
    pub id: i64,
    pub state: RequestState,
    pub media_type: SeerrMediaType,
    pub tmdb_id: i64,
    pub title: String,
    pub year: Option<i32>,
    pub poster: Option<String>,
    pub is_4k: bool,
    pub seasons: Vec<i64>,
    pub requested_by: String,
    /// Requested by the signed-in Seerr user.
    pub mine: bool,
    pub created_at: Option<String>,
    /// Availability of the title itself.
    pub status: Option<Availability>,
}

/// A title with everything its Seerr page shows. Derefs to the [`SeerrTitle`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeerrDetails {
    #[serde(flatten)]
    pub base: SeerrTitle,
    pub tagline: String,
    pub genres: Vec<String>,
    /// Minutes (an episode's, for shows).
    pub runtime: Option<i64>,
    pub tvdb_id: Option<i64>,
    pub seasons: Vec<SeerrSeason>,
    pub requests: Vec<SeerrRequest>,
}

impl std::ops::Deref for SeerrDetails {
    type Target = SeerrTitle;
    fn deref(&self) -> &SeerrTitle {
        &self.base
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeerrPage<T> {
    pub page: u32,
    pub pages: u32,
    pub results: Vec<T>,
}

/// What the signed-in Seerr user may do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeerrCan {
    pub request_movie: bool,
    pub request_tv: bool,
    pub request_4k_movie: bool,
    pub request_4k_tv: bool,
    /// Choose the Radarr/Sonarr server, quality profile and folder.
    pub advanced: bool,
    /// Approve, decline, retry and delete anyone's requests.
    pub manage: bool,
}

/// The signed-in Seerr user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeerrUser {
    pub id: i64,
    pub name: String,
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeerrStatus {
    pub url: String,
    /// How Patina signs in.
    pub auth: SeerrAuthKind,
    pub version: Option<String>,
    pub user: SeerrUser,
    pub can: SeerrCan,
}

/// The instance as configured, without talking to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeerrConfig {
    pub url: String,
    pub auth: SeerrAuthKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestProfile {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestRootFolder {
    pub path: String,
    pub free_space: Option<f64>,
}

/// A Radarr or Sonarr server to send a request to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestOptionServer {
    pub id: i64,
    pub name: String,
    pub is_default: bool,
    pub active_profile_id: Option<i64>,
    pub active_directory: Option<String>,
    pub profiles: Vec<RequestProfile>,
    pub root_folders: Vec<RequestRootFolder>,
}

/// A request to make. Without the optional parts Seerr picks its defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewRequest {
    pub media_type: SeerrMediaType,
    pub tmdb_id: i64,
    /// Shows only.
    #[serde(default)]
    pub tvdb_id: Option<i64>,
    /// Shows only: the season numbers, at least one.
    #[serde(default)]
    pub seasons: Vec<i64>,
    #[serde(default)]
    pub is_4k: bool,
    #[serde(default)]
    pub server_id: Option<i64>,
    #[serde(default)]
    pub profile_id: Option<i64>,
    #[serde(default)]
    pub root_folder: Option<String>,
}

impl NewRequest {
    pub fn movie(tmdb_id: i64) -> Self {
        Self { media_type: SeerrMediaType::Movie, tmdb_id, tvdb_id: None, seasons: vec![], is_4k: false, server_id: None, profile_id: None, root_folder: None }
    }

    pub fn tv(tmdb_id: i64, seasons: Vec<i64>) -> Self {
        Self { media_type: SeerrMediaType::Tv, seasons, ..Self::movie(tmdb_id) }
    }
}

/// Seerr's permission bits (`server/lib/permissions.ts`). `ADMIN` implies all of them.
pub mod permission {
    pub const ADMIN: i64 = 2;
    pub const MANAGE_REQUESTS: i64 = 16;
    pub const REQUEST: i64 = 32;
    pub const REQUEST_4K: i64 = 1024;
    pub const REQUEST_4K_MOVIE: i64 = 2048;
    pub const REQUEST_4K_TV: i64 = 4096;
    pub const REQUEST_ADVANCED: i64 = 8192;
    pub const REQUEST_MOVIE: i64 = 262144;
    pub const REQUEST_TV: i64 = 524288;
}

/// Whether `permissions` grant any of `wanted` (admins have them all).
pub fn has_any(permissions: i64, wanted: &[i64]) -> bool {
    if permissions & permission::ADMIN != 0 {
        return true;
    }
    wanted.iter().any(|p| permissions & p != 0)
}

/// Which kinds of title have a 4K server set up in Seerr.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Has4k {
    pub movie: bool,
    pub tv: bool,
}

/// What the user may do, given which kinds have a 4K server set up.
pub fn capabilities(permissions: i64, has_4k: Has4k) -> SeerrCan {
    use permission::*;
    SeerrCan {
        request_movie: has_any(permissions, &[REQUEST, REQUEST_MOVIE]),
        request_tv: has_any(permissions, &[REQUEST, REQUEST_TV]),
        request_4k_movie: has_4k.movie && has_any(permissions, &[REQUEST_4K, REQUEST_4K_MOVIE]),
        request_4k_tv: has_4k.tv && has_any(permissions, &[REQUEST_4K, REQUEST_4K_TV]),
        advanced: has_any(permissions, &[REQUEST_ADVANCED, MANAGE_REQUESTS]),
        manage: has_any(permissions, &[MANAGE_REQUESTS]),
    }
}

/// A title's availability from Seerr's `MediaStatus` number.
pub fn availability(n: Option<i64>) -> Option<Availability> {
    Some(match n? {
        1 => Availability::Unknown,
        2 => Availability::Pending,
        3 => Availability::Processing,
        4 => Availability::Partial,
        5 => Availability::Available,
        6 => Availability::Blocklisted,
        7 => Availability::Deleted,
        _ => return None,
    })
}

/// A request's state from Seerr's `MediaRequestStatus` number; unknown numbers read as pending.
pub fn request_state(n: i64) -> RequestState {
    match n {
        2 => RequestState::Approved,
        3 => RequestState::Declined,
        4 => RequestState::Failed,
        5 => RequestState::Completed,
        _ => RequestState::Pending,
    }
}

/// Image sizes of TMDB artwork that Patina asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TmdbSize {
    W500,
    W1280,
}

/// TMDB artwork URL for a Seerr image path (absolute URLs pass through).
pub fn tmdb_image(path: Option<&str>, size: TmdbSize) -> Option<String> {
    let path = path.filter(|p| !p.is_empty())?;
    if path.starts_with("http://") || path.starts_with("https://") {
        return Some(path.to_owned());
    }
    let size = match size {
        TmdbSize::W500 => "w500",
        TmdbSize::W1280 => "w1280",
    };
    Some(format!("https://image.tmdb.org/t/p/{size}{}{path}", if path.starts_with('/') { "" } else { "/" }))
}

/// `Number.parseInt(date.slice(0, 4), 10)`: the leading digits of the first four characters.
fn year_of(date: Option<&str>) -> Option<i32> {
    let head: String = date?.chars().take(4).collect();
    let head = head.trim_start();
    let (sign, digits) = match head.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, head.strip_prefix('+').unwrap_or(head)),
    };
    let digits: String = digits.chars().take_while(char::is_ascii_digit).collect();
    digits.parse::<i32>().ok().map(|y| sign * y)
}

/// The first of `names` that is not empty.
fn first_set<'a>(names: &[Option<&'a str>]) -> Option<&'a str> {
    names.iter().copied().flatten().find(|n| !n.is_empty())
}

fn user_name(u: Option<&RawUser>) -> String {
    let Some(u) = u else { return "Unknown".into() };
    let email = u.email.as_deref();
    first_set(&[u.display_name.as_deref(), u.username.as_deref(), u.plex_username.as_deref(), u.jellyfin_username.as_deref(), email])
        .map_or_else(|| format!("User {}", u.id), str::to_owned)
}

/// What a search result, a movie and a show have in common.
struct View<'a> {
    id: i64,
    title: Option<&'a str>,
    name: Option<&'a str>,
    release_date: Option<&'a str>,
    first_air_date: Option<&'a str>,
    overview: Option<&'a str>,
    poster_path: Option<&'a str>,
    backdrop_path: Option<&'a str>,
    vote_average: Option<f64>,
    info: Option<&'a RawMediaInfo>,
}

impl<'a> From<&'a RawResult> for View<'a> {
    fn from(r: &'a RawResult) -> Self {
        View {
            id: r.id,
            title: r.title.as_deref(),
            name: r.name.as_deref(),
            release_date: r.release_date.as_deref(),
            first_air_date: r.first_air_date.as_deref(),
            overview: r.overview.as_deref(),
            poster_path: r.poster_path.as_deref(),
            backdrop_path: r.backdrop_path.as_deref(),
            vote_average: r.vote_average,
            info: r.media_info.as_ref(),
        }
    }
}

impl<'a> From<&'a RawMovieDetails> for View<'a> {
    fn from(r: &'a RawMovieDetails) -> Self {
        View {
            id: r.id,
            title: r.title.as_deref(),
            name: None,
            release_date: r.release_date.as_deref(),
            first_air_date: None,
            overview: r.overview.as_deref(),
            poster_path: r.poster_path.as_deref(),
            backdrop_path: r.backdrop_path.as_deref(),
            vote_average: r.vote_average,
            info: r.media_info.as_ref(),
        }
    }
}

impl<'a> From<&'a RawTvDetails> for View<'a> {
    fn from(r: &'a RawTvDetails) -> Self {
        View {
            id: r.id,
            title: None,
            name: r.name.as_deref(),
            release_date: None,
            first_air_date: r.first_air_date.as_deref(),
            overview: r.overview.as_deref(),
            poster_path: r.poster_path.as_deref(),
            backdrop_path: r.backdrop_path.as_deref(),
            vote_average: r.vote_average,
            info: r.media_info.as_ref(),
        }
    }
}

fn title_from(media_type: SeerrMediaType, v: View) -> SeerrTitle {
    let (primary, date) = match media_type {
        SeerrMediaType::Movie => (v.title, v.release_date),
        SeerrMediaType::Tv => (v.name, v.first_air_date),
    };
    let info = v.info;
    SeerrTitle {
        media_type,
        tmdb_id: v.id,
        title: primary.or(v.title).or(v.name).unwrap_or("Untitled").to_owned(),
        year: year_of(date),
        overview: v.overview.unwrap_or("").to_owned(),
        poster: tmdb_image(v.poster_path, TmdbSize::W500),
        backdrop: tmdb_image(v.backdrop_path, TmdbSize::W1280),
        rating: v.vote_average.filter(|r| *r != 0.0 && !r.is_nan()),
        status: availability(info.and_then(|i| i.status)),
        status_4k: availability(info.and_then(|i| i.status4k)),
        // A title that arrived only in 4K is named by its 4K key.
        rating_key: first_set(&[info.and_then(|i| i.rating_key.as_deref()), info.and_then(|i| i.rating_key4k.as_deref())]).map(str::to_owned),
    }
}

/// Movies and shows of a search or discover page (people and collections are left out).
pub fn titles_from(results: &[RawResult]) -> Vec<SeerrTitle> {
    results
        .iter()
        .filter_map(|r| {
            let media_type = r.media_type.as_deref()?.parse().ok()?;
            Some(title_from(media_type, r.into()))
        })
        .collect()
}

/// What a request remembers of its title: a request only names its TMDB id.
#[derive(Debug, Clone)]
struct Known {
    title: String,
    year: Option<i32>,
    poster: Option<String>,
}

impl From<&SeerrTitle> for Known {
    fn from(t: &SeerrTitle) -> Self {
        Known { title: t.title.clone(), year: t.year, poster: t.poster.clone() }
    }
}

fn request_from(r: &RawMediaRequest, me: Option<i64>, known: Option<&Known>) -> SeerrRequest {
    let media_type = r.kind.or(r.media.as_ref().and_then(|m| m.media_type)).unwrap_or(SeerrMediaType::Movie);
    let media = r.media.as_ref();
    let mut seasons: Vec<i64> = r.seasons.iter().map(|s| s.season_number).collect();
    seasons.sort_unstable();
    SeerrRequest {
        id: r.id,
        state: request_state(r.status.unwrap_or(0)),
        media_type,
        tmdb_id: media.and_then(|m| m.tmdb_id).unwrap_or(0),
        title: known.map(|k| k.title.clone()).unwrap_or_default(),
        year: known.and_then(|k| k.year),
        poster: known.and_then(|k| k.poster.clone()),
        is_4k: r.is4k,
        seasons,
        requested_by: user_name(r.requested_by.as_ref()),
        mine: me.is_some() && r.requested_by.as_ref().map(|u| u.id) == me,
        created_at: r.created_at.clone(),
        status: availability(if r.is4k { media.and_then(|m| m.status4k) } else { media.and_then(|m| m.status) }),
    }
}

/// Seasons of a show (without specials), with their availability and open requests.
pub fn seasons_from(tv: &RawTvDetails) -> Vec<SeerrSeason> {
    let info = tv.media_info.as_ref();
    let mut open: Vec<&RawMediaRequest> = info.map(|i| i.requests.iter().filter(|r| r.status != Some(3)).collect()).unwrap_or_default();
    open.sort_by(|a, b| a.created_at.as_deref().unwrap_or("").cmp(b.created_at.as_deref().unwrap_or("")));
    let requested = |n: i64, is_4k: bool| -> Option<RequestState> {
        let covers = |s: &RawRequestSeason| s.season_number == n;
        open.iter().rfind(|r| r.is4k == is_4k && r.seasons.iter().any(covers)).map(|r| request_state(r.status.unwrap_or(0)))
    };
    tv.seasons
        .iter()
        .filter(|s| s.season_number > 0)
        .map(|s| {
            let media = info.and_then(|i| i.seasons.iter().find(|m| m.season_number == s.season_number));
            SeerrSeason {
                number: s.season_number,
                name: s.name.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| format!("Season {}", s.season_number)),
                episodes: s.episode_count,
                air_date: s.air_date.clone(),
                status: availability(media.and_then(|m| m.status)),
                status_4k: availability(media.and_then(|m| m.status4k)),
                requested: requested(s.season_number, false),
                requested_4k: requested(s.season_number, true),
            }
        })
        .collect()
}

/// The client, kept while the settings it was made from stay the same (it holds the Plex session).
struct Entry {
    key: String,
    client: SeerrClient,
    me: Mutex<Option<RawUser>>,
}

static CACHED: Mutex<Option<Arc<Entry>>> = Mutex::new(None);

/// Titles of requested media, oldest first, so the oldest can be dropped.
#[derive(Default)]
struct TitleCache {
    titles: HashMap<(SeerrMediaType, i64), Known>,
    order: VecDeque<(SeerrMediaType, i64)>,
}

static TITLES: LazyLock<Mutex<TitleCache>> = LazyLock::new(Default::default);

fn remember(media_type: SeerrMediaType, tmdb_id: i64, t: &SeerrTitle) {
    let key = (media_type, tmdb_id);
    let mut cache = TITLES.lock().unwrap();
    cache.order.retain(|k| *k != key);
    cache.order.push_back(key);
    cache.titles.insert(key, t.into());
    if cache.order.len() > 1000 && let Some(oldest) = cache.order.pop_front() {
        cache.titles.remove(&oldest);
    }
}

fn known(media_type: SeerrMediaType, tmdb_id: i64) -> Option<Known> {
    TITLES.lock().unwrap().titles.get(&(media_type, tmdb_id)).cloned()
}

/// The configured instance; `None` when Seerr is not set up.
pub fn config(app: &AppState) -> Option<SeerrConfig> {
    let s = app.settings().seerr;
    let url = s.url.trim();
    if url.is_empty() {
        return None;
    }
    Some(SeerrConfig { url: url.to_owned(), auth: if s.api_key.trim().is_empty() { SeerrAuthKind::Plex } else { SeerrAuthKind::ApiKey } })
}

fn plex_token(app: &AppState) -> Result<String> {
    app.settings().auth.token.filter(|t| !t.is_empty()).ok_or_else(|| SeerrError::new("Not signed in to plex.tv: add a Seerr API key to connect."))
}

fn client_for(url: &str, api_key: &str, app: &AppState) -> Result<Arc<Entry>> {
    let (key, auth) = if api_key.is_empty() {
        let token = plex_token(app)?;
        (format!("{url}|plex|{token}"), SeerrAuth::PlexToken(token))
    } else {
        (format!("{url}|key|{api_key}"), SeerrAuth::ApiKey(api_key.to_owned()))
    };
    if let Some(entry) = CACHED.lock().unwrap().as_ref().filter(|e| e.key == key) {
        return Ok(entry.clone());
    }
    Ok(Arc::new(Entry { key, client: SeerrClient::new(url, auth)?, me: Mutex::new(None) }))
}

fn current(app: &AppState) -> Result<Arc<Entry>> {
    let s = app.settings().seerr;
    if s.url.trim().is_empty() {
        return Err(SeerrError::new("Seerr is not set up. Add it in Settings."));
    }
    let entry = client_for(s.url.trim(), s.api_key.trim(), app)?;
    *CACHED.lock().unwrap() = Some(entry.clone());
    Ok(entry)
}

async fn me(entry: &Entry) -> Result<RawUser> {
    if let Some(user) = entry.me.lock().unwrap().clone() {
        return Ok(user);
    }
    let user = entry.client.me().await?;
    *entry.me.lock().unwrap() = Some(user.clone());
    Ok(user)
}

/// Talk to the instance: version, the signed-in user and what they may do.
pub async fn status(app: &AppState) -> Result<SeerrStatus> {
    let entry = current(app)?;
    status_of(&entry).await
}

async fn status_of(entry: &Entry) -> Result<SeerrStatus> {
    let client = &entry.client;
    let (info, user) = futures::join!(client.status(), client.me());
    let user = user?;
    *entry.me.lock().unwrap() = Some(user.clone());
    // Without a 4K server there is nothing to request in 4K. Users may not be allowed to list them.
    let has_4k = async |kind| client.services(kind).await.unwrap_or_default().iter().any(|s| s.is4k);
    let (movie, tv) = futures::join!(has_4k(ServiceKind::Radarr), has_4k(ServiceKind::Sonarr));
    Ok(SeerrStatus {
        url: client.base_url().to_owned(),
        auth: client.auth_kind(),
        version: info.ok().and_then(|i| i.version),
        user: SeerrUser {
            id: user.id,
            name: user_name(Some(&user)),
            avatar: user.avatar.as_deref().filter(|a| !a.is_empty()).map(|a| avatar_url(client.base_url(), a)),
        },
        can: capabilities(user.permissions.unwrap_or(0), Has4k { movie, tv }),
    })
}

/// Avatars are plex.tv or gravatar URLs, or paths on the Seerr server.
fn avatar_url(base: &str, avatar: &str) -> String {
    if avatar.starts_with("http://") || avatar.starts_with("https://") {
        avatar.to_owned()
    } else {
        format!("{base}{}{avatar}", if avatar.starts_with('/') { "" } else { "/" })
    }
}

/// Check the instance answers and accepts the sign-in, then save it.
/// `api_key` `None` keeps the saved key; empty signs in with the Plex account.
pub async fn connect(app: &AppState, url: &str, api_key: Option<&str>) -> Result<SeerrStatus> {
    let base = normalize_url(url)?;
    let key = api_key.map_or_else(|| app.settings().seerr.api_key, str::to_owned);
    let key = key.trim().to_owned();
    let entry = client_for(&base, &key, app)?;
    let result = status_of(&entry).await?;
    app.try_update_settings(|s| {
        s.seerr.url = base;
        s.seerr.api_key = key;
        Ok(())
    })
    .map_err(|e| SeerrError::new(format!("{e:#}")))?;
    *CACHED.lock().unwrap() = Some(entry);
    Ok(result)
}

/// Forget the instance: clear the saved address and key.
pub fn disconnect(app: &AppState) -> Result<()> {
    *CACHED.lock().unwrap() = None;
    app.try_update_settings(|s| {
        s.seerr.url.clear();
        s.seerr.api_key.clear();
        Ok(())
    })
    .map_err(|e| SeerrError::new(format!("{e:#}")))
}

/// A discover list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoverList {
    Trending,
    Movies,
    Tv,
    UpcomingMovies,
    UpcomingTv,
}

impl DiscoverList {
    fn path(self) -> &'static str {
        match self {
            DiscoverList::Trending => "/discover/trending",
            DiscoverList::Movies => "/discover/movies",
            DiscoverList::Tv => "/discover/tv",
            DiscoverList::UpcomingMovies => "/discover/movies/upcoming",
            DiscoverList::UpcomingTv => "/discover/tv/upcoming",
        }
    }
}

impl FromStr for DiscoverList {
    type Err = SeerrError;

    /// `trending`, `movies`, `tv`, `upcoming-movies` or `upcoming-tv`.
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "trending" => Ok(DiscoverList::Trending),
            "movies" => Ok(DiscoverList::Movies),
            "tv" => Ok(DiscoverList::Tv),
            "upcoming-movies" => Ok(DiscoverList::UpcomingMovies),
            "upcoming-tv" => Ok(DiscoverList::UpcomingTv),
            other => Err(SeerrError::new(format!("Unknown list {other}."))),
        }
    }
}

fn count(n: i64) -> u32 {
    n.clamp(0, u32::MAX as i64) as u32
}

pub async fn discover(app: &AppState, list: DiscoverList, page: u32) -> Result<SeerrPage<SeerrTitle>> {
    let got = current(app)?.client.discover(list.path(), page.max(1)).await?;
    Ok(SeerrPage { page: count(got.page), pages: count(got.total_pages), results: titles_from(&got.results) })
}

pub async fn search(app: &AppState, query: &str, page: u32) -> Result<SeerrPage<SeerrTitle>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(SeerrPage { page: 1, pages: 0, results: vec![] });
    }
    let got = current(app)?.client.search(q, page.max(1)).await?;
    Ok(SeerrPage { page: count(got.page), pages: count(got.total_pages), results: titles_from(&got.results) })
}

/// How long the Seerr title page waits for the Plex server to confirm its copy.
const PLEX_CHECK: Duration = Duration::from_secs(5);

/// Whether a Plex item is this TMDB title: by its TMDB guid (new or legacy agent), or,
/// when it has no external ids, by kind and title.
pub fn is_tmdb_title(m: &Metadata, media_type: SeerrMediaType, tmdb_id: i64, title: &str) -> bool {
    if m.kind.as_deref() != Some(if media_type == SeerrMediaType::Tv { "show" } else { "movie" }) {
        return false;
    }
    let tmdb_of = |id: &str| -> Option<String> {
        let rest = id.strip_prefix("tmdb://").or_else(|| id.strip_prefix("com.plexapp.agents.themoviedb://"))?;
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        (!digits.is_empty()).then_some(digits)
    };
    let tmdb: Vec<String> = m.guids.iter().filter_map(|g| g.id.as_deref()).chain(m.guid.as_deref()).filter_map(tmdb_of).collect();
    if !tmdb.is_empty() {
        return tmdb.contains(&tmdb_id.to_string());
    }
    m.guids.is_empty() && m.title.as_deref().unwrap_or("").to_lowercase() == title.to_lowercase()
}

/// Seerr's rating key names an item on the Plex server Seerr syncs with, which may not be
/// the one Patina is connected to: keep it only if this server has the same title there.
pub async fn plex_copy(server: &PlexServer, t: &SeerrTitle) -> Option<String> {
    plex_copy_with(|key| async move { server.metadata(&key, MetadataOptions::full()).await }, t, PLEX_CHECK).await
}

/// [`plex_copy`] with the lookup of the Plex item and the time to wait for it given.
pub async fn plex_copy_with<F, Fut, E>(lookup: F, t: &SeerrTitle, wait: Duration) -> Option<String>
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = std::result::Result<Option<Metadata>, E>>,
{
    let key = t.rating_key.as_ref()?;
    // Optional: a slow or unreachable Plex server must not hold up the Seerr page.
    let found = tokio::time::timeout(wait, lookup(key.clone())).await.ok()?.ok()??;
    is_tmdb_title(&found, t.media_type, t.tmdb_id, &t.title).then(|| key.clone())
}

async fn raw_title(entry: &Entry, media_type: SeerrMediaType, tmdb_id: i64) -> Result<(SeerrTitle, Raw)> {
    Ok(match media_type {
        SeerrMediaType::Movie => {
            let raw = entry.client.movie(tmdb_id).await?;
            (title_from(media_type, (&raw).into()), Raw::Movie(raw))
        }
        SeerrMediaType::Tv => {
            let raw = entry.client.tv(tmdb_id).await?;
            (title_from(media_type, (&raw).into()), Raw::Tv(raw))
        }
    })
}

enum Raw {
    Movie(RawMovieDetails),
    Tv(RawTvDetails),
}

/// A title's page. `plex` is the server to check Seerr's copy against, when connected.
pub async fn details(app: &AppState, media_type: SeerrMediaType, tmdb_id: i64) -> Result<SeerrDetails> {
    let entry = current(app)?;
    let plex = if app.session().is_some() { app.server().ok() } else { None };
    details_of(&entry, media_type, tmdb_id, plex.as_ref()).await
}

async fn details_of(entry: &Entry, media_type: SeerrMediaType, tmdb_id: i64, plex: Option<&PlexServer>) -> Result<SeerrDetails> {
    let user = me(entry).await?;
    let (mut base, raw) = raw_title(entry, media_type, tmdb_id).await?;
    remember(media_type, tmdb_id, &base);
    let (tagline, genres, info, runtime, tv) = match &raw {
        Raw::Movie(m) => (&m.tagline, &m.genres, m.media_info.as_ref(), m.runtime, None),
        Raw::Tv(t) => (&t.tagline, &t.genres, t.media_info.as_ref(), t.episode_run_time.first().copied(), Some(t)),
    };
    base.rating_key = match plex {
        Some(server) => plex_copy(server, &base).await,
        None => None,
    };
    let known_title: Known = (&base).into();
    let mut requests: Vec<SeerrRequest> = info
        .map(|info| {
            info.requests
                .iter()
                .map(|r| {
                    // The request's media on top of the title's own: its status and kind.
                    let over = r.media.as_ref();
                    let media = RawMediaInfo {
                        tmdb_id: Some(tmdb_id),
                        media_type: over.and_then(|m| m.media_type).or(info.media_type),
                        status: over.and_then(|m| m.status).or(info.status),
                        status4k: over.and_then(|m| m.status4k).or(info.status4k),
                        ..Default::default()
                    };
                    let r = RawMediaRequest { kind: r.kind.or(Some(media_type)), media: Some(media), ..r.clone() };
                    request_from(&r, Some(user.id), Some(&known_title))
                })
                .collect()
        })
        .unwrap_or_default();
    requests.sort_by(|a, b| b.created_at.as_deref().unwrap_or("").cmp(a.created_at.as_deref().unwrap_or("")));
    Ok(SeerrDetails {
        tagline: tagline.clone().unwrap_or_default(),
        genres: genres.iter().map(|g| g.name.clone().unwrap_or_default()).collect(),
        runtime: runtime.map(|r| r as i64).filter(|r| *r != 0),
        tvdb_id: tv.and_then(|t| t.external_ids.as_ref().and_then(|e| e.tvdb_id)).or(info.and_then(|i| i.tvdb_id)),
        seasons: tv.map(seasons_from).unwrap_or_default(),
        requests,
        base,
    })
}

/// Make a request. Resolves to its state: `Approved` when the user's requests are approved automatically.
pub async fn request(app: &AppState, r: &NewRequest) -> Result<RequestState> {
    let entry = current(app)?;
    request_of(&entry, r).await
}

async fn request_of(entry: &Entry, r: &NewRequest) -> Result<RequestState> {
    let tv = r.media_type == SeerrMediaType::Tv;
    if tv && r.seasons.is_empty() {
        return Err(SeerrError::new("Choose at least one season."));
    }
    let made = entry
        .client
        .create_request(&CreateRequest {
            media_type: r.media_type,
            media_id: r.tmdb_id,
            tvdb_id: if tv { r.tvdb_id } else { None },
            seasons: tv.then(|| r.seasons.clone()),
            is_4k: r.is_4k,
            server_id: r.server_id,
            profile_id: r.profile_id,
            root_folder: r.root_folder.clone(),
        })
        .await?;
    // Seerr answers 202 with only a message when nothing was left to request (say, someone else just did).
    match made {
        Some(RawCreatedRequest { status: Some(n), .. }) => Ok(request_state(n)),
        other => {
            let why = other.and_then(|m| m.message).filter(|m| !m.is_empty());
            Err(SeerrError::new(why.unwrap_or_else(|| "Seerr did not create the request.".into())))
        }
    }
}

/// Radarr (movies) or Sonarr (shows) servers to send a request to, with their profiles and folders.
pub async fn request_options(app: &AppState, media_type: SeerrMediaType, is_4k: bool) -> Result<Vec<RequestOptionServer>> {
    let entry = current(app)?;
    request_options_of(&entry, media_type, is_4k).await
}

async fn request_options_of(entry: &Entry, media_type: SeerrMediaType, is_4k: bool) -> Result<Vec<RequestOptionServer>> {
    let client = &entry.client;
    let kind = if media_type == SeerrMediaType::Movie { ServiceKind::Radarr } else { ServiceKind::Sonarr };
    let servers: Vec<_> = client.services(kind).await?.into_iter().filter(|s| s.is4k == is_4k).collect();
    futures::future::try_join_all(servers.into_iter().map(async |s| {
        let d = client.service(kind, s.id).await?;
        Ok::<_, SeerrError>(RequestOptionServer {
            id: s.id,
            name: s.name.unwrap_or_default(),
            is_default: s.is_default,
            active_profile_id: d.server.active_profile_id.or(s.active_profile_id),
            active_directory: d.server.active_directory.or(s.active_directory),
            profiles: d.profiles.into_iter().map(|p| RequestProfile { id: p.id, name: p.name.unwrap_or_default() }).collect(),
            root_folders: d.root_folders.into_iter().map(|f| RequestRootFolder { path: f.path.unwrap_or_default(), free_space: f.free_space }).collect(),
        })
    }))
    .await
}

/// The filters of the request list, as Seerr names them.
pub const REQUEST_FILTERS: [&str; 7] = ["all", "pending", "approved", "processing", "available", "unavailable", "failed"];
const PAGE_SIZE: u32 = 20;

/// A filter of the request list (see [`REQUEST_FILTERS`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestFilter {
    All,
    Pending,
    Approved,
    Processing,
    Available,
    Unavailable,
    Failed,
}

impl RequestFilter {
    pub const ALL: [RequestFilter; 7] = [
        RequestFilter::All,
        RequestFilter::Pending,
        RequestFilter::Approved,
        RequestFilter::Processing,
        RequestFilter::Available,
        RequestFilter::Unavailable,
        RequestFilter::Failed,
    ];

    /// Seerr's name for it, one of [`REQUEST_FILTERS`].
    pub fn as_str(self) -> &'static str {
        REQUEST_FILTERS[self as usize]
    }
}

impl FromStr for RequestFilter {
    type Err = SeerrError;

    fn from_str(s: &str) -> Result<Self> {
        RequestFilter::ALL.into_iter().find(|f| f.as_str() == s).ok_or_else(|| SeerrError::new(format!("Unknown filter {s}.")))
    }
}

/// Requests by `filter`, 20 a page, newest first; `mine` limits them to the user's own.
pub async fn requests(app: &AppState, filter: RequestFilter, page: u32, mine: bool) -> Result<SeerrPage<SeerrRequest>> {
    let entry = current(app)?;
    requests_of(&entry, filter, page, mine).await
}

async fn requests_of(entry: &Entry, filter: RequestFilter, page: u32, mine: bool) -> Result<SeerrPage<SeerrRequest>> {
    let user = me(entry).await?;
    let p = page.max(1);
    let got = entry
        .client
        .requests(&RequestsQuery {
            take: PAGE_SIZE,
            skip: (p - 1) * PAGE_SIZE,
            filter: filter.as_str().to_owned(),
            sort: "added".into(),
            requested_by: mine.then_some(user.id),
        })
        .await?;
    let results = futures::future::join_all(got.results.iter().map(async |r| {
        let media_type = r.kind.or(r.media.as_ref().and_then(|m| m.media_type)).unwrap_or(SeerrMediaType::Movie);
        let tmdb_id = r.media.as_ref().and_then(|m| m.tmdb_id);
        let mut title = tmdb_id.and_then(|id| known(media_type, id));
        if title.is_none() && let Some(id) = tmdb_id {
            // Shown by its TMDB id instead when Seerr can't say.
            if let Ok((t, _)) = raw_title(entry, media_type, id).await {
                remember(media_type, id, &t);
                title = Some((&t).into());
            }
        }
        let r = RawMediaRequest { kind: Some(media_type), ..r.clone() };
        let mut out = request_from(&r, Some(user.id), title.as_ref());
        if out.title.is_empty() {
            out.title = format!("TMDB {}", tmdb_id.map_or_else(|| "?".to_owned(), |id| id.to_string()));
        }
        out
    }))
    .await;
    Ok(SeerrPage { page: count(got.page_info.page), pages: count(got.page_info.pages), results })
}

/// What to do to a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestAction {
    Approve,
    Decline,
    Retry,
    Delete,
}

impl FromStr for RequestAction {
    type Err = SeerrError;

    /// `approve`, `decline`, `retry` or `delete`.
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "approve" => Ok(RequestAction::Approve),
            "decline" => Ok(RequestAction::Decline),
            "retry" => Ok(RequestAction::Retry),
            "delete" => Ok(RequestAction::Delete),
            other => Err(SeerrError::new(format!("Unknown request action {other}."))),
        }
    }
}

/// Approve, decline, retry or delete a request.
pub async fn request_action(app: &AppState, id: i64, action: RequestAction) -> Result<()> {
    let entry = current(app)?;
    request_action_of(&entry, id, action).await
}

async fn request_action_of(entry: &Entry, id: i64, action: RequestAction) -> Result<()> {
    let client = &entry.client;
    match action {
        RequestAction::Approve => client.set_request_status(id, RequestVerdict::Approve).await,
        RequestAction::Decline => client.set_request_status(id, RequestVerdict::Decline).await,
        RequestAction::Retry => client.retry_request(id).await,
        RequestAction::Delete => client.delete_request(id).await,
    }
}

/// The Seerr web page of a title.
pub fn page_url(app: &AppState, media_type: SeerrMediaType, tmdb_id: i64) -> Result<String> {
    let entry = current(app)?;
    Ok(page_url_of(&entry, media_type, tmdb_id))
}

fn page_url_of(entry: &Entry, media_type: SeerrMediaType, tmdb_id: i64) -> String {
    format!("{}/{}/{tmdb_id}", entry.client.base_url(), media_type.as_str())
}

/// Forget the client and the remembered titles (tests, and a sign-out that changes the Plex token).
pub fn reset() {
    *CACHED.lock().unwrap() = None;
    let mut cache = TITLES.lock().unwrap();
    cache.titles.clear();
    cache.order.clear();
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use super::testing::{Resp, serve};
    use super::*;

    fn raw<T: serde::de::DeserializeOwned>(v: Value) -> T {
        serde_json::from_value(v).unwrap()
    }

    fn movie_title() -> SeerrTitle {
        titles_from(&[raw(json!({ "id": 949, "mediaType": "movie", "title": "Heat", "mediaInfo": { "status": 5, "ratingKey": "100" } }))]).remove(0)
    }

    fn entry(url: &str, auth: SeerrAuth) -> Entry {
        Entry { key: String::new(), client: SeerrClient::new(url, auth).unwrap(), me: Mutex::new(None) }
    }

    #[test]
    fn permissions_follow_seerr_admins_may_do_everything() {
        assert!(has_any(2, &[16]));
        assert!(!has_any(32, &[16]));
        let user = capabilities(32, Has4k { movie: true, tv: false });
        assert_eq!(
            user,
            SeerrCan { request_movie: true, request_tv: true, request_4k_movie: false, request_4k_tv: false, advanced: false, manage: false }
        );
        let movies = capabilities(262144 | 2048, Has4k { movie: true, tv: true });
        assert_eq!([movies.request_movie, movies.request_tv, movies.request_4k_movie, movies.request_4k_tv], [true, false, true, false]);
        // 4K needs a 4K server too.
        assert_eq!(
            capabilities(2, Has4k { movie: false, tv: true }),
            SeerrCan { request_movie: true, request_tv: true, request_4k_movie: false, request_4k_tv: true, advanced: true, manage: true }
        );
    }

    #[test]
    fn statuses_map_to_names() {
        use Availability::*;
        assert_eq!(availability(None), None);
        assert_eq!(availability(Some(0)), None);
        assert_eq!(
            (1..=7).map(|n| availability(Some(n))).collect::<Vec<_>>(),
            [Unknown, Pending, Processing, Partial, Available, Blocklisted, Deleted].map(Some)
        );
        assert_eq!((1..=5).map(request_state).collect::<Vec<_>>(), [
            RequestState::Pending,
            RequestState::Approved,
            RequestState::Declined,
            RequestState::Failed,
            RequestState::Completed
        ]);
        assert_eq!(serde_json::to_value(Partial).unwrap(), json!("partial"));
        assert_eq!(serde_json::to_value(SeerrMediaType::Tv).unwrap(), json!("tv"));
    }

    #[test]
    fn results_keep_movies_and_shows_with_their_artwork_and_availability() {
        let results: Vec<RawResult> = raw(json!([
            { "id": 1, "mediaType": "movie", "title": "A", "releaseDate": "2021-02-03", "posterPath": "/a.jpg", "mediaInfo": { "status": 5, "ratingKey": "100" } },
            { "id": 2, "mediaType": "tv", "name": "B", "firstAirDate": "", "posterPath": "http://127.0.0.1/b.jpg" },
            { "id": 3, "mediaType": "person", "name": "C" },
            { "id": 4, "mediaType": "movie", "title": "D", "mediaInfo": { "status": 1, "status4k": 5, "ratingKey": null, "ratingKey4k": "42" } },
        ]));
        let titles = titles_from(&results);
        let rows: Vec<_> = titles.iter().map(|t| (t.media_type, t.title.as_str(), t.year, t.poster.as_deref(), t.status, t.rating_key.as_deref())).collect();
        assert_eq!(rows, [
            (SeerrMediaType::Movie, "A", Some(2021), Some("https://image.tmdb.org/t/p/w500/a.jpg"), Some(Availability::Available), Some("100")),
            (SeerrMediaType::Tv, "B", None, Some("http://127.0.0.1/b.jpg"), None, None),
            (SeerrMediaType::Movie, "D", None, None, Some(Availability::Unknown), Some("42")),
        ]);
        assert_eq!(titles[2].status_4k, Some(Availability::Available));
        assert_eq!(tmdb_image(None, TmdbSize::W500), None);
        assert_eq!(tmdb_image(Some("b.jpg"), TmdbSize::W1280).as_deref(), Some("https://image.tmdb.org/t/p/w1280/b.jpg"));
    }

    #[test]
    fn titles_read_loosely_typed_json() {
        let results: Vec<RawResult> = raw(json!([
            { "id": "7", "mediaType": "tv", "name": "Odd", "voteAverage": "7.5", "mediaInfo": { "status": "3", "ratingKey": 55 } },
            { "id": 8, "mediaType": "movie", "title": "Zero", "voteAverage": 0 },
        ]));
        let titles = titles_from(&results);
        assert_eq!((titles[0].tmdb_id, titles[0].rating, titles[0].status), (7, Some(7.5), Some(Availability::Processing)));
        assert_eq!(titles[0].rating_key.as_deref(), Some("55"));
        assert_eq!(titles[1].rating, None);
        assert_eq!(year_of(Some("2021-02-03")), Some(2021));
        assert_eq!(year_of(Some("")), None);
        assert_eq!(year_of(Some("abcd")), None);
        assert_eq!(year_of(None), None);
    }

    #[test]
    fn seasons_show_what_is_available_and_what_is_requested_without_specials() {
        let tv: RawTvDetails = raw(json!({
            "id": 8,
            "seasons": [{ "seasonNumber": 0 }, { "seasonNumber": 1, "episodeCount": 3 }, { "seasonNumber": 2 }, { "seasonNumber": 3 }],
            "mediaInfo": {
                "status": 4,
                "seasons": [{ "seasonNumber": 1, "status": 5 }],
                "requests": [
                    { "id": 1, "status": 1, "createdAt": "2024-01-01", "seasons": [{ "seasonNumber": 2 }] },
                    { "id": 2, "status": 2, "createdAt": "2024-02-01", "seasons": [{ "seasonNumber": 2 }] },
                    { "id": 3, "status": 3, "createdAt": "2024-03-01", "seasons": [{ "seasonNumber": 3 }] },
                    { "id": 4, "status": 1, "is4k": true, "seasons": [{ "seasonNumber": 3 }] },
                ],
            },
        }));
        let seasons = seasons_from(&tv);
        let rows: Vec<_> = seasons.iter().map(|s| (s.number, s.name.as_str(), s.episodes, s.status, s.requested, s.requested_4k)).collect();
        assert_eq!(rows, [
            (1, "Season 1", Some(3), Some(Availability::Available), None, None),
            (2, "Season 2", None, None, Some(RequestState::Approved), None),
            (3, "Season 3", None, None, None, Some(RequestState::Pending)),
        ]);
    }

    #[test]
    fn a_plex_item_is_the_seerr_title_only_when_its_tmdb_id_or_without_ids_its_title_agrees() {
        let item = |extra: Value| -> Metadata {
            let mut base = json!({ "ratingKey": "100", "type": "movie", "title": "Heat" });
            base.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            raw(base)
        };
        let movie = SeerrMediaType::Movie;
        assert!(is_tmdb_title(&item(json!({ "Guid": [{ "id": "imdb://tt1" }, { "id": "tmdb://949" }] })), movie, 949, "Heat"));
        // Another server's rating key can name an unrelated title.
        assert!(!is_tmdb_title(&item(json!({ "title": "Up", "Guid": [{ "id": "tmdb://14160" }] })), movie, 949, "Heat"));
        assert!(is_tmdb_title(&item(json!({ "guid": "com.plexapp.agents.themoviedb://949?lang=en" })), movie, 949, "Heat"));
        assert!(is_tmdb_title(&item(json!({})), movie, 949, "heat"));
        assert!(!is_tmdb_title(&item(json!({})), SeerrMediaType::Tv, 949, "Heat"));
    }

    #[tokio::test]
    async fn the_plex_copy_is_checked_but_a_server_that_doesnt_answer_doesnt_hold_up_the_page() {
        let movie = movie_title();
        let found: Metadata = raw(json!({ "ratingKey": "100", "type": "movie", "title": "Heat", "Guid": [{ "id": "tmdb://949" }] }));
        let wait = Duration::from_secs(5);
        let ok = |m: Option<Metadata>| async move { Ok::<_, std::io::Error>(m) };
        assert_eq!(plex_copy_with(|_| ok(Some(found)), &movie, wait).await.as_deref(), Some("100"));
        assert_eq!(plex_copy_with(|_| ok(None), &movie, wait).await, None);
        let offline = |_| async { Err::<Option<Metadata>, _>(std::io::Error::other("offline")) };
        assert_eq!(plex_copy_with(offline, &movie, wait).await, None);
        let started = std::time::Instant::now();
        let hang = |_| std::future::pending::<std::result::Result<Option<Metadata>, std::io::Error>>();
        assert_eq!(plex_copy_with(hang, &movie, Duration::from_millis(30)).await, None);
        assert!(started.elapsed() < Duration::from_secs(1));
        // No rating key, nothing to check.
        let mut none = movie_title();
        none.rating_key = None;
        assert_eq!(plex_copy_with(|_| ok(None), &none, wait).await, None);
    }

    #[test]
    fn names_filters_and_actions_parse_like_the_commands() {
        assert_eq!("upcoming-tv".parse::<DiscoverList>().unwrap(), DiscoverList::UpcomingTv);
        assert_eq!("nope".parse::<DiscoverList>().unwrap_err().to_string(), "Unknown list nope.");
        assert_eq!(RequestFilter::ALL.map(RequestFilter::as_str), REQUEST_FILTERS);
        assert_eq!("failed".parse::<RequestFilter>().unwrap(), RequestFilter::Failed);
        assert_eq!("x".parse::<RequestFilter>().unwrap_err().to_string(), "Unknown filter x.");
        assert_eq!("retry".parse::<RequestAction>().unwrap(), RequestAction::Retry);
        assert_eq!("x".parse::<RequestAction>().unwrap_err().to_string(), "Unknown request action x.");
        assert_eq!("person".parse::<SeerrMediaType>().unwrap_err().to_string(), "Unknown media type person.");
        assert_eq!(avatar_url("http://s", "/a.png"), "http://s/a.png");
        assert_eq!(avatar_url("http://s", "a.png"), "http://s/a.png");
        assert_eq!(avatar_url("http://s", "https://plex.tv/a.png"), "https://plex.tv/a.png");
    }

    /// A fake Seerr with one show (2001) and one movie (2002) behind an API key.
    async fn fake_seerr() -> String {
        serve(|req| {
            if req.headers.get("x-api-key").map(String::as_str) != Some("key") && req.path != "/api/v1/status" {
                return Resp::json(401, json!({ "message": "Unauthorized" }));
            }
            let body = req.body.clone();
            match (req.method.as_str(), req.path.trim_start_matches("/api/v1")) {
                (_, "/status") => Resp::json(200, json!({ "version": "2.7.3" })),
                (_, "/auth/me") => Resp::json(200, json!({ "id": 5, "displayName": "Bob", "permissions": 2, "avatar": "/av/5.png" })),
                (_, "/service/radarr") => Resp::json(200, json!([
                    { "id": 1, "name": "Radarr HD", "is4k": false, "isDefault": true, "activeProfileId": 4, "activeDirectory": "/movies" },
                    { "id": 2, "name": "Radarr 4K", "is4k": true, "isDefault": false },
                ])),
                (_, "/service/sonarr") => Resp::json(200, json!([])),
                (_, "/service/radarr/1") => Resp::json(200, json!({
                    "server": { "id": 1, "name": "Radarr HD", "is4k": false, "isDefault": true, "activeProfileId": 9 },
                    "profiles": [{ "id": 4, "name": "HD" }, { "id": 9, "name": "Any" }],
                    "rootFolders": [{ "id": 1, "path": "/movies", "freeSpace": 1000 }, { "id": 2, "path": "/more" }],
                })),
                (_, "/tv/2001") => Resp::json(200, json!({
                    "id": 2001, "name": "Show", "firstAirDate": "2020-01-01", "episodeRunTime": [42], "tagline": "Tag",
                    "genres": [{ "id": 1, "name": "Drama" }], "externalIds": { "tvdbId": 77 },
                    "seasons": [{ "seasonNumber": 1, "episodeCount": 3 }],
                    "mediaInfo": { "status": 3, "tvdbId": 1, "requests": [
                        { "id": 10, "status": 1, "createdAt": "2024-01-01", "is4k": false, "seasons": [{ "seasonNumber": 1 }], "requestedBy": { "id": 5, "displayName": "Bob" } },
                        { "id": 11, "status": 2, "createdAt": "2024-02-01", "is4k": true, "seasons": [{ "seasonNumber": 1 }], "requestedBy": { "id": 6, "plexUsername": "carol" } },
                    ], "status4k": 5 },
                })),
                (_, "/movie/2002") => Resp::json(200, json!({ "id": 2002, "title": "Film", "releaseDate": "2019-05-05", "runtime": 0 })),
                ("GET", "/request") => Resp::json(200, json!({
                    "pageInfo": { "page": 2, "pages": 3, "results": 45 },
                    "results": [
                        { "id": 1, "status": 1, "type": "movie", "media": { "tmdbId": 2002, "status": 2 }, "requestedBy": { "id": 5 } },
                        { "id": 2, "status": 2, "media": { "tmdbId": 2001, "mediaType": "tv" }, "seasons": [{ "seasonNumber": 2 }, { "seasonNumber": 1 }] },
                        { "id": 3, "status": 4, "type": "movie", "media": { "tmdbId": 99999 }, "requestedBy": { "id": 1, "email": "e@x" } },
                    ],
                })),
                ("POST", "/request") => {
                    let sent: Value = serde_json::from_str(&body).unwrap();
                    match sent["mediaId"].as_i64() {
                        Some(2002) => Resp::json(201, json!({ "id": 12, "status": 2 })),
                        Some(2003) => Resp::json(202, json!({ "message": "Nothing left to request." })),
                        _ => Resp(204, vec![], String::new()),
                    }
                }
                ("POST", "/request/10/approve") | ("POST", "/request/10/retry") | ("DELETE", "/request/10") => Resp(204, vec![], String::new()),
                _ => Resp::json(404, json!({ "message": "Not found" })),
            }
        })
        .await
    }

    #[tokio::test]
    async fn details_requests_options_and_actions_against_a_fake_server() {
        let url = fake_seerr().await;
        let e = entry(&url, SeerrAuth::ApiKey("key".into()));

        let s = status_of(&e).await.unwrap();
        assert_eq!((s.version.as_deref(), s.auth, s.user.name.as_str()), (Some("2.7.3"), SeerrAuthKind::ApiKey, "Bob"));
        assert_eq!(s.user.avatar, Some(format!("{url}/av/5.png")));
        // An admin, and Radarr has a 4K server.
        assert!(s.can.manage && s.can.request_4k_movie && !s.can.request_4k_tv);

        let d = details_of(&e, SeerrMediaType::Tv, 2001, None).await.unwrap();
        assert_eq!((d.title.as_str(), d.year, d.tagline.as_str(), d.runtime, d.tvdb_id), ("Show", Some(2020), "Tag", Some(42), Some(77)));
        assert_eq!(d.genres, ["Drama"]);
        assert_eq!(d.rating_key, None);
        assert_eq!((d.seasons[0].requested, d.seasons[0].requested_4k), (Some(RequestState::Pending), Some(RequestState::Approved)));
        // Newest first, with the title, who asked and the availability of the right quality.
        let reqs: Vec<_> = d.requests.iter().map(|r| (r.id, r.title.as_str(), r.requested_by.as_str(), r.mine, r.is_4k, r.status, r.tmdb_id)).collect();
        assert_eq!(reqs, [
            (11, "Show", "carol", false, true, Some(Availability::Available), 2001),
            (10, "Show", "Bob", true, false, Some(Availability::Processing), 2001),
        ]);
        let m = details_of(&e, SeerrMediaType::Movie, 2002, None).await.unwrap();
        assert_eq!((m.runtime, m.tvdb_id, m.tagline.as_str()), (None, None, ""));

        let page = requests_of(&e, RequestFilter::All, 2, false).await.unwrap();
        assert_eq!((page.page, page.pages), (2, 3));
        let rows: Vec<_> = page.results.iter().map(|r| (r.id, r.media_type, r.title.as_str(), r.state, r.mine, r.seasons.clone())).collect();
        assert_eq!(rows, [
            (1, SeerrMediaType::Movie, "Film", RequestState::Pending, true, vec![]),
            (2, SeerrMediaType::Tv, "Show", RequestState::Approved, false, vec![1, 2]),
            (3, SeerrMediaType::Movie, "TMDB 99999", RequestState::Failed, false, vec![]),
        ]);
        assert_eq!(page.results[2].requested_by, "e@x");
        assert_eq!(page.results[0].status, Some(Availability::Pending));

        let options = request_options_of(&e, SeerrMediaType::Movie, false).await.unwrap();
        assert_eq!(options, [RequestOptionServer {
            id: 1,
            name: "Radarr HD".into(),
            is_default: true,
            active_profile_id: Some(9),
            active_directory: Some("/movies".into()),
            profiles: vec![RequestProfile { id: 4, name: "HD".into() }, RequestProfile { id: 9, name: "Any".into() }],
            root_folders: vec![
                RequestRootFolder { path: "/movies".into(), free_space: Some(1000.0) },
                RequestRootFolder { path: "/more".into(), free_space: None },
            ],
        }]);
        assert!(request_options_of(&e, SeerrMediaType::Tv, false).await.unwrap().is_empty());

        assert_eq!(request_of(&e, &NewRequest::movie(2002)).await.unwrap(), RequestState::Approved);
        assert_eq!(request_of(&e, &NewRequest::movie(2003)).await.unwrap_err().to_string(), "Nothing left to request.");
        assert_eq!(request_of(&e, &NewRequest::movie(2004)).await.unwrap_err().to_string(), "Seerr did not create the request.");
        assert_eq!(request_of(&e, &NewRequest::tv(2001, vec![])).await.unwrap_err().to_string(), "Choose at least one season.");

        for action in [RequestAction::Approve, RequestAction::Retry, RequestAction::Delete] {
            request_action_of(&e, 10, action).await.unwrap();
        }
        assert_eq!(request_action_of(&e, 11, RequestAction::Decline).await.unwrap_err().status(), Some(404));
        assert_eq!(page_url_of(&e, SeerrMediaType::Tv, 2001), format!("{url}/tv/2001"));
        assert_eq!(page_url_of(&e, SeerrMediaType::Movie, 5), format!("{url}/movie/5"));
    }

    #[test]
    fn the_title_cache_drops_the_oldest() {
        // Other tests remember titles too, so only look at what the 1000 limit must decide.
        let t = movie_title();
        for id in 100_000..101_100 {
            remember(SeerrMediaType::Tv, id, &t);
        }
        assert!(known(SeerrMediaType::Tv, 100_000).is_none());
        assert!(known(SeerrMediaType::Tv, 101_099).is_some());
        // Remembering again makes a title the newest, so it outlives the ones that came after it.
        remember(SeerrMediaType::Tv, 100_200, &t);
        for id in 200_000..200_150 {
            remember(SeerrMediaType::Tv, id, &t);
        }
        assert!(known(SeerrMediaType::Tv, 100_200).is_some());
        assert!(known(SeerrMediaType::Tv, 100_201).is_none());
    }
}
