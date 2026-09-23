use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{ACCEPT, HeaderMap, HeaderName, HeaderValue};
use reqwest::{Method, RequestBuilder, Response};
use serde::de::DeserializeOwned;

use crate::error::{Error, Result};

/// Identifies this application to Plex. Every request carries these values as
/// `X-Plex-*` headers; `identifier` must be stable per installation.
#[derive(Debug, Clone)]
pub struct ClientInfo {
    pub identifier: String,
    pub product: String,
    pub version: String,
    pub platform: String,
    pub platform_version: String,
    pub device: String,
    pub device_name: String,
}

impl ClientInfo {
    pub fn new(identifier: impl Into<String>, product: impl Into<String>, version: impl Into<String>) -> Self {
        let platform = match std::env::consts::OS {
            "macos" => "macOS",
            "windows" => "Windows",
            "linux" => "Linux",
            other => other,
        }
        .to_string();
        let device_name = hostname().unwrap_or_else(|| platform.clone());
        Self {
            identifier: identifier.into(),
            product: product.into(),
            version: version.into(),
            platform_version: String::new(),
            device: platform.clone(),
            platform,
            device_name,
        }
    }

    pub fn headers(&self) -> Vec<(&'static str, String)> {
        vec![
            ("X-Plex-Client-Identifier", self.identifier.clone()),
            ("X-Plex-Product", self.product.clone()),
            ("X-Plex-Version", self.version.clone()),
            ("X-Plex-Platform", self.platform.clone()),
            ("X-Plex-Platform-Version", self.platform_version.clone()),
            ("X-Plex-Device", self.device.clone()),
            ("X-Plex-Device-Name", self.device_name.clone()),
            ("X-Plex-Model", "desktop".into()),
            ("X-Plex-Provides", "player".into()),
        ]
    }

    fn header_map(&self) -> HeaderMap {
        let mut map = HeaderMap::new();
        map.insert(ACCEPT, HeaderValue::from_static("application/json"));
        for (k, v) in self.headers() {
            if let (Ok(name), Ok(v)) = (HeaderName::from_bytes(k.as_bytes()), HeaderValue::from_str(&v)) {
                map.insert(name, v);
            }
        }
        map
    }
}

fn hostname() -> Option<String> {
    std::env::var("HOSTNAME")
        .ok()
        .or_else(|| {
            std::process::Command::new("hostname")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
        })
        .map(|h| h.trim().trim_end_matches(".local").to_string())
        .filter(|h| !h.is_empty())
}

/// Ordered query parameters. Plex endpoints take almost everything as query
/// parameters, including on PUT/POST.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Params(pub Vec<(String, String)>);

impl Params {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(mut self, key: impl Into<String>, value: impl ToString) -> Self {
        self.push(key, value);
        self
    }

    pub fn opt<V: ToString>(mut self, key: impl Into<String>, value: Option<V>) -> Self {
        if let Some(v) = value {
            self.push(key, v);
        }
        self
    }

    /// Adds `key=1` when `flag` is true.
    pub fn flag(self, key: impl Into<String>, flag: bool) -> Self {
        if flag { self.set(key, 1) } else { self }
    }

    pub fn push(&mut self, key: impl Into<String>, value: impl ToString) {
        self.0.push((key.into(), value.to_string()));
    }

    pub fn extend(mut self, other: &Params) -> Self {
        self.0.extend(other.0.iter().cloned());
        self
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn to_query_string(&self) -> String {
        url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(self.0.iter())
            .finish()
    }
}

impl<K: Into<String>, V: ToString> FromIterator<(K, V)> for Params {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut p = Params::new();
        for (k, v) in iter {
            p.push(k, v);
        }
        p
    }
}

/// Shared HTTP plumbing for plex.tv and server clients.
#[derive(Clone)]
pub(crate) struct Http {
    pub client: reqwest::Client,
    pub info: Arc<ClientInfo>,
}

impl Http {
    pub fn new(info: ClientInfo) -> Result<Self> {
        let client = reqwest::Client::builder()
            .default_headers(info.header_map())
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(60))
            .gzip(true)
            .build()?;
        Ok(Self { client, info: Arc::new(info) })
    }

    pub fn request(&self, method: Method, url: url::Url, token: Option<&str>, params: &Params) -> RequestBuilder {
        let mut rb = self.client.request(method, url).query(&params.0);
        if let Some(token) = token {
            rb = rb.header("X-Plex-Token", token);
        }
        rb
    }

    pub async fn send(&self, rb: RequestBuilder) -> Result<Response> {
        let req = rb.build()?;
        let method = req.method().to_string();
        let path = req.url().path().to_string();
        tracing::debug!(%method, url = %redact(req.url()), "plex request");
        let resp = self.client.execute(req).await?;
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        if status.as_u16() == 401 {
            return Err(Error::Unauthorized);
        }
        let body = resp.text().await.unwrap_or_default();
        Err(Error::Status { method, path, status: status.as_u16(), body })
    }

    pub async fn json<T: DeserializeOwned>(&self, rb: RequestBuilder) -> Result<T> {
        let resp = self.send(rb).await?;
        let path = resp.url().path().to_string();
        let bytes = resp.bytes().await?;
        decode(&path, &bytes)
    }

    pub async fn empty(&self, rb: RequestBuilder) -> Result<()> {
        self.send(rb).await?;
        Ok(())
    }
}

pub(crate) fn decode<T: DeserializeOwned>(path: &str, bytes: &[u8]) -> Result<T> {
    // Some endpoints answer with an empty body where a container is expected.
    let bytes: &[u8] = if bytes.iter().all(u8::is_ascii_whitespace) { b"{}" } else { bytes };
    let de = &mut serde_json::Deserializer::from_slice(bytes);
    serde_path_to_error::deserialize(de).map_err(|source| Error::Decode { path: path.to_string(), source })
}

fn redact(url: &url::Url) -> String {
    let mut u = url.clone();
    let pairs: Vec<(String, String)> = u
        .query_pairs()
        .map(|(k, v)| {
            let v = if k.eq_ignore_ascii_case("X-Plex-Token") { "***".into() } else { v.into_owned() };
            (k.into_owned(), v)
        })
        .collect();
    if pairs.is_empty() {
        u.set_query(None);
    } else {
        u.query_pairs_mut().clear().extend_pairs(pairs);
    }
    u.to_string()
}
