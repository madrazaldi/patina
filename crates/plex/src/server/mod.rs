mod admin;
mod collections;
mod hubs;
mod library;
mod livetv;
mod metadata;
mod notifications;
mod playback;
mod playlists;
mod playqueue;
mod status;

use reqwest::Method;
use serde::de::DeserializeOwned;
use serde_json::Value;
use url::Url;

use crate::client::{ClientInfo, Http, Params};
use crate::error::Result;
use crate::models::*;

pub use library::LibraryQuery;
pub use metadata::MetadataOptions;
pub use notifications::Notification;
pub use playback::{TimelineState, TimelineUpdate, TranscodeOptions};
pub use playqueue::PlayQueueRequest;
pub use status::HistoryQuery;

/// Client bound to one Plex Media Server connection.
#[derive(Clone)]
pub struct PlexServer {
    http: Http,
    base: Url,
    token: String,
    machine_identifier: Option<String>,
}

impl std::fmt::Debug for PlexServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlexServer")
            .field("base", &self.base.as_str())
            .field("machine_identifier", &self.machine_identifier)
            .finish()
    }
}

impl PartialEq for PlexServer {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base && self.token == other.token
    }
}

impl PlexServer {
    pub fn new(info: ClientInfo, base_url: &str, token: &str) -> Result<Self> {
        Self::with_http(Http::new(info)?, base_url, token)
    }

    pub(crate) fn with_http(http: Http, base_url: &str, token: &str) -> Result<Self> {
        let mut base = Url::parse(base_url)?;
        if !base.path().ends_with('/') {
            let path = format!("{}/", base.path());
            base.set_path(&path);
        }
        Ok(Self { http, base, token: token.to_string(), machine_identifier: None })
    }

    pub fn with_machine_identifier(mut self, id: Option<String>) -> Self {
        self.machine_identifier = id;
        self
    }

    pub fn base_url(&self) -> &Url {
        &self.base
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn client_info(&self) -> &ClientInfo {
        &self.http.info
    }

    pub fn machine_identifier(&self) -> Option<&str> {
        self.machine_identifier.as_deref()
    }

    /// Resolve a server path (`/library/...`) or absolute URL.
    pub fn url(&self, path: &str) -> Result<Url> {
        if path.starts_with("http://") || path.starts_with("https://") {
            return Ok(Url::parse(path)?);
        }
        // "./" keeps a colon in the first segment (`/tv.plex.providers.epg.cloud:2/...`) from reading as a scheme.
        Ok(self.base.join(&format!("./{}", path.trim_start_matches('/')))?)
    }

    /// A URL with the token embedded, for handing to external programs.
    pub fn authed_url(&self, path: &str, params: &Params) -> Result<Url> {
        let mut url = self.url(path)?;
        {
            let mut q = url.query_pairs_mut();
            for (k, v) in &params.0 {
                q.append_pair(k, v);
            }
            q.append_pair("X-Plex-Token", &self.token);
        }
        Ok(url)
    }

    /// `server://<machine>/com.plexapp.plugins.library<path>` URIs used by
    /// playlists, collections and play queues.
    pub fn library_uri(&self, path: &str) -> String {
        format!(
            "server://{}/com.plexapp.plugins.library{}",
            self.machine_identifier.as_deref().unwrap_or(""),
            path
        )
    }

    /// Library URI for one or more items (`/library/metadata/1,2,3`).
    pub fn item_uri(&self, rating_keys: &[&str]) -> String {
        self.library_uri(&format!("/library/metadata/{}", rating_keys.join(",")))
    }

    // ----- request helpers ----------------------------------------------

    pub(crate) async fn request<T: DeserializeOwned>(&self, method: Method, path: &str, params: &Params) -> Result<T> {
        let rb = self.http.request(method, self.url(path)?, Some(&self.token), params);
        self.http.json(rb).await
    }

    pub(crate) async fn container(&self, method: Method, path: &str, params: &Params) -> Result<MediaContainer> {
        let env: Envelope = self.request(method, path, params).await?;
        Ok(env.media_container)
    }

    pub(crate) async fn get(&self, path: &str, params: &Params) -> Result<MediaContainer> {
        self.container(Method::GET, path, params).await
    }

    pub(crate) async fn empty(&self, method: Method, path: &str, params: &Params) -> Result<()> {
        let rb = self.http.request(method, self.url(path)?, Some(&self.token), params);
        self.http.empty(rb).await
    }

    pub(crate) async fn put(&self, path: &str, params: &Params) -> Result<()> {
        self.empty(Method::PUT, path, params).await
    }

    pub(crate) async fn delete(&self, path: &str, params: &Params) -> Result<()> {
        self.empty(Method::DELETE, path, params).await
    }

    pub(crate) async fn post(&self, path: &str, params: &Params) -> Result<()> {
        self.empty(Method::POST, path, params).await
    }

    /// Escape hatch: any endpoint, raw JSON back.
    pub async fn raw(&self, method: Method, path: &str, params: &Params) -> Result<Value> {
        self.request(method, path, params).await
    }

    /// Escape hatch returning the typed container.
    pub async fn fetch(&self, path: &str, params: &Params) -> Result<MediaContainer> {
        self.get(path, params).await
    }

    /// GET raw bytes (images, subtitles, downloads).
    pub async fn bytes(&self, path: &str, params: &Params) -> Result<(Vec<u8>, Option<String>)> {
        let rb = self.http.request(Method::GET, self.url(path)?, Some(&self.token), params);
        let resp = self.http.send(rb).await?;
        let ctype = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        Ok((resp.bytes().await?.to_vec(), ctype))
    }

    /// Streaming GET, for large downloads.
    pub async fn stream(&self, path: &str, params: &Params) -> Result<reqwest::Response> {
        let rb = self.http.request(Method::GET, self.url(path)?, Some(&self.token), params);
        self.http.send(rb).await
    }

    /// Streaming GET without the usual request timeout, for whole media files.
    pub async fn download(&self, path: &str, params: &Params) -> Result<reqwest::Response> {
        let rb = self
            .http
            .request(Method::GET, self.url(path)?, Some(&self.token), params)
            .timeout(std::time::Duration::from_secs(60 * 60 * 24));
        self.http.send(rb).await
    }

    /// Resume a media transfer. Identity encoding keeps byte offsets meaningful.
    pub async fn download_range(&self, path: &str, offset: u64, validator: Option<&str>) -> Result<reqwest::Response> {
        let mut rb = self.http.request(Method::GET, self.url(path)?, Some(&self.token), &Params::new().set("download", 1))
            .header("Accept-Encoding", "identity")
            .timeout(std::time::Duration::from_secs(60 * 60 * 24));
        if offset > 0 {
            rb = rb.header("Range", format!("bytes={offset}-"));
            if let Some(v) = validator { rb = rb.header("If-Range", v); }
        }
        self.http.send(rb).await
    }

    // ----- server info ---------------------------------------------------

    /// Server root: capabilities, version, flags (in `extra`) and feature directories.
    pub async fn capabilities(&self) -> Result<MediaContainer> {
        self.get("/", &Params::new()).await
    }

    /// Unauthenticated identity check; used to probe connections.
    pub async fn identity(&self) -> Result<Identity> {
        #[derive(serde::Deserialize)]
        struct Wrap {
            #[serde(rename = "MediaContainer")]
            mc: Identity,
        }
        let w: Wrap = self.request(Method::GET, "/identity", &Params::new()).await?;
        Ok(w.mc)
    }

    /// Resized image URL via the photo transcoder. `source` is a server path or absolute URL.
    pub fn image_url(&self, source: &str, width: u32, height: u32) -> Result<Url> {
        let params = Params::new()
            .set("url", source)
            .set("width", width)
            .set("height", height)
            .set("minSize", 1)
            .set("upscale", 1);
        self.authed_url("/photo/:/transcode", &params)
    }

    /// Fetch a resized image through the photo transcoder.
    pub async fn image(&self, source: &str, width: u32, height: u32) -> Result<(Vec<u8>, Option<String>)> {
        let params = Params::new()
            .set("url", source)
            .set("width", width)
            .set("height", height)
            .set("minSize", 1)
            .set("upscale", 1);
        self.bytes("/photo/:/transcode", &params).await
    }
}
