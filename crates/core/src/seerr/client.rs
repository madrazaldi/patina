//! Client of the Seerr API (`/api/v1`, <https://docs.seerr.dev/api/seerr-api/>),
//! which Overseerr and Jellyseerr share. It signs in with an API key
//! (`X-Api-Key`), or with the Plex account token (`POST /auth/plex`), whose
//! session cookie it keeps and renews when Seerr forgets it.

use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::FutureExt;
use futures::future::{BoxFuture, Shared};
use reqwest::Method;
use reqwest::header::{ACCEPT, CONTENT_TYPE, COOKIE, SET_COOKIE};
use serde::Serialize;
use serde::de::{DeserializeOwned, IgnoredAny};

use super::models::*;
use super::{SeerrError, SeerrMediaType};

/// Requests give up after this long.
const TIMEOUT: Duration = Duration::from_secs(30);

/// How the client signs in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeerrAuth {
    ApiKey(String),
    /// The plex.tv account token; Seerr answers with a session cookie.
    PlexToken(String),
}

/// How requests are signed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
pub enum SeerrAuthKind {
    #[serde(rename = "apiKey")]
    ApiKey,
    #[serde(rename = "plex")]
    Plex,
}

/// Radarr (movies) or Sonarr (shows) servers set up in Seerr.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceKind {
    Radarr,
    Sonarr,
}

impl ServiceKind {
    fn name(self) -> &'static str {
        match self {
            ServiceKind::Radarr => "radarr",
            ServiceKind::Sonarr => "sonarr",
        }
    }
}

/// A change to the list of requests (`GET /request`).
#[derive(Debug, Clone)]
pub struct RequestsQuery {
    pub take: u32,
    pub skip: u32,
    pub filter: String,
    pub sort: String,
    pub requested_by: Option<i64>,
}

/// The body of `POST /request`; absent fields are left out.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRequest {
    pub media_type: SeerrMediaType,
    pub media_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tvdb_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seasons: Option<Vec<i64>>,
    #[serde(rename = "is4k")]
    pub is_4k: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_folder: Option<String>,
}

/// `approve` or `decline` a pending request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestVerdict {
    Approve,
    Decline,
}

impl RequestVerdict {
    fn name(self) -> &'static str {
        match self {
            RequestVerdict::Approve => "approve",
            RequestVerdict::Decline => "decline",
        }
    }
}

/// The base URL as typed by the user, without a trailing slash or `/api/v1`.
/// A missing scheme means http, as Seerr usually runs on a LAN port.
pub fn normalize_url(input: &str) -> Result<String, SeerrError> {
    let typed = input.trim();
    if typed.is_empty() {
        return Err(SeerrError::new("Enter the address of your Seerr server."));
    }
    let has_scheme = typed.split_once("://").is_some_and(|(scheme, _)| {
        let mut chars = scheme.chars();
        chars.next().is_some_and(|c| c.is_ascii_alphabetic())
            && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
    });
    let full = if has_scheme { typed.to_owned() } else { format!("http://{typed}") };
    let mut parsed = url::Url::parse(&full).map_err(|_| SeerrError::new(format!("“{typed}” is not a valid address.")))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(SeerrError::new("The address must start with http:// or https://."));
    }
    parsed.set_query(None);
    parsed.set_fragment(None);
    let text = parsed.to_string();
    let text = text.trim_end_matches('/');
    Ok(text.strip_suffix("/api/v1").unwrap_or(text).to_owned())
}

/// Seerr validates queries against its OpenAPI schema and rejects some characters
/// unless they are percent-encoded, so encode everything but the unreserved
/// characters (`A-Z a-z 0-9 - _ . ~`). Parameters that are `None` are left out.
pub fn encode_query(params: &[(&str, Option<String>)]) -> String {
    fn enc(out: &mut String, v: &str) {
        for b in v.bytes() {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                out.push(b as char);
            } else {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    let mut out = String::new();
    for (k, v) in params {
        let Some(v) = v else { continue };
        if !out.is_empty() {
            out.push('&');
        }
        enc(&mut out, k);
        out.push('=');
        enc(&mut out, v);
    }
    out
}

type SignIn = Shared<BoxFuture<'static, Result<(), SeerrError>>>;

struct Inner {
    base_url: String,
    auth: SeerrAuth,
    http: reqwest::Client,
    /// `connect.sid=...` after a Plex sign-in.
    cookie: Mutex<Option<String>>,
    signing_in: Mutex<Option<SignIn>>,
}

/// A client of one Seerr instance. Cheap to clone; clones share the session.
#[derive(Clone)]
pub struct SeerrClient(Arc<Inner>);

impl fmt::Debug for SeerrClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SeerrClient").field("base_url", &self.0.base_url).field("auth", &self.auth_kind()).finish()
    }
}

impl SeerrClient {
    /// `url` is normalized with [`normalize_url`].
    pub fn new(url: &str, auth: SeerrAuth) -> Result<Self, SeerrError> {
        let base_url = normalize_url(url)?;
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(TIMEOUT)
            .gzip(true)
            .build()
            .map_err(|e| SeerrError::new(format!("Could not set up the connection to Seerr: {e}")))?;
        Ok(Self(Arc::new(Inner { base_url, auth, http, cookie: Mutex::new(None), signing_in: Mutex::new(None) })))
    }

    /// The address of the instance, without a trailing slash.
    pub fn base_url(&self) -> &str {
        &self.0.base_url
    }

    /// How requests are signed in.
    pub fn auth_kind(&self) -> SeerrAuthKind {
        match self.0.auth {
            SeerrAuth::ApiKey(_) => SeerrAuthKind::ApiKey,
            SeerrAuth::PlexToken(_) => SeerrAuthKind::Plex,
        }
    }

    /// Version of the server; needs no sign-in.
    pub async fn status(&self) -> Result<RawStatus, SeerrError> {
        self.request(Method::GET, "/status", &[], None, false).await
    }

    /// The signed-in user.
    pub async fn me(&self) -> Result<RawUser, SeerrError> {
        self.get("/auth/me", &[]).await
    }

    pub async fn search(&self, query: &str, page: u32) -> Result<RawPage<RawResult>, SeerrError> {
        self.get("/search", &[("query", Some(query.to_owned())), ("page", Some(page.to_string()))]).await
    }

    /// `path` is a discover list such as `/discover/trending` or `/discover/movies/upcoming`.
    pub async fn discover(&self, path: &str, page: u32) -> Result<RawPage<RawResult>, SeerrError> {
        self.get(path, &[("page", Some(page.to_string()))]).await
    }

    pub async fn movie(&self, tmdb_id: i64) -> Result<RawMovieDetails, SeerrError> {
        self.get(&format!("/movie/{tmdb_id}"), &[]).await
    }

    pub async fn tv(&self, tmdb_id: i64) -> Result<RawTvDetails, SeerrError> {
        self.get(&format!("/tv/{tmdb_id}"), &[]).await
    }

    pub async fn requests(&self, q: &RequestsQuery) -> Result<RawRequestPage, SeerrError> {
        let query = [
            ("take", Some(q.take.to_string())),
            ("skip", Some(q.skip.to_string())),
            ("filter", Some(q.filter.clone())),
            ("sort", Some(q.sort.clone())),
            ("requestedBy", q.requested_by.map(|id| id.to_string())),
        ];
        self.get("/request", &query).await
    }

    /// `None` when Seerr answered with no body.
    pub async fn create_request(&self, body: &CreateRequest) -> Result<Option<RawCreatedRequest>, SeerrError> {
        let body = serde_json::to_value(body).map_err(|e| SeerrError::new(e.to_string()))?;
        self.request(Method::POST, "/request", &[], Some(body), true).await
    }

    /// `approve` or `decline` a pending request.
    /// Seerr answers with the updated request, which is not needed.
    pub async fn set_request_status(&self, id: i64, verdict: RequestVerdict) -> Result<(), SeerrError> {
        self.request::<IgnoredAny>(Method::POST, &format!("/request/{id}/{}", verdict.name()), &[], None, true).await.map(drop)
    }

    pub async fn retry_request(&self, id: i64) -> Result<(), SeerrError> {
        self.request::<IgnoredAny>(Method::POST, &format!("/request/{id}/retry"), &[], None, true).await.map(drop)
    }

    pub async fn delete_request(&self, id: i64) -> Result<(), SeerrError> {
        self.request::<IgnoredAny>(Method::DELETE, &format!("/request/{id}"), &[], None, true).await.map(drop)
    }

    /// Radarr or Sonarr servers set up in Seerr.
    pub async fn services(&self, kind: ServiceKind) -> Result<Vec<RawService>, SeerrError> {
        self.get(&format!("/service/{}", kind.name()), &[]).await
    }

    /// Quality profiles and root folders of one Radarr or Sonarr server.
    pub async fn service(&self, kind: ServiceKind, id: i64) -> Result<RawServiceDetails, SeerrError> {
        self.get(&format!("/service/{}/{id}", kind.name()), &[]).await
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, query: &[(&str, Option<String>)]) -> Result<T, SeerrError> {
        self.request(Method::GET, path, query, None, true).await
    }

    async fn sign_in(&self) -> Result<(), SeerrError> {
        let SeerrAuth::PlexToken(token) = &self.0.auth else { return Ok(()) };
        let body = serde_json::json!({ "authToken": token });
        let resp = self.send(&Method::POST, "/auth/plex", &[], Some(&body), None).await?;
        if !resp.status().is_success() {
            return Err(failure(resp, "POST", "/auth/plex").await);
        }
        let cookie = resp
            .headers()
            .get_all(SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .filter_map(|c| c.split(';').next())
            .find(|c| c.starts_with("connect.sid="))
            .map(str::to_owned);
        let Some(cookie) = cookie else {
            return Err(SeerrError::new("Seerr accepted the Plex sign-in but sent no session."));
        };
        *self.0.cookie.lock().unwrap() = Some(cookie);
        Ok(())
    }

    /// Sign in once, however many requests are waiting for it.
    async fn ensure_signed_in(&self) -> Result<(), SeerrError> {
        let pending = {
            let mut slot = self.0.signing_in.lock().unwrap();
            slot.get_or_insert_with(|| {
                let this = self.clone();
                async move {
                    let done = this.sign_in().await;
                    *this.0.signing_in.lock().unwrap() = None;
                    done
                }
                .boxed()
                .shared()
            })
            .clone()
        };
        pending.await
    }

    fn cookie(&self) -> Option<String> {
        self.0.cookie.lock().unwrap().clone()
    }

    async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, Option<String>)],
        body: Option<serde_json::Value>,
        authed: bool,
    ) -> Result<T, SeerrError> {
        let plex = authed && matches!(self.0.auth, SeerrAuth::PlexToken(_));
        if plex && self.cookie().is_none() {
            self.ensure_signed_in().await?;
        }
        let sent = if authed { self.cookie() } else { None };
        let mut resp = self.send(&method, path, query, body.as_ref(), sent.as_deref()).await?;
        // Seerr restarted or the session expired: sign in again (once for all requests that noticed), and retry once.
        if plex && let Some(sent) = &sent && matches!(resp.status().as_u16(), 401 | 403) {
            {
                let mut cookie = self.0.cookie.lock().unwrap();
                if cookie.as_ref() == Some(sent) {
                    *cookie = None;
                }
            }
            if self.cookie().is_none() {
                self.ensure_signed_in().await?;
            }
            resp = self.send(&method, path, query, body.as_ref(), self.cookie().as_deref()).await?;
        }
        if !resp.status().is_success() {
            return Err(failure(resp, method.as_str(), path).await);
        }
        let text = resp.text().await.map_err(|e| self.unreachable(&e))?;
        let value = if text.trim().is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_str(&text).map_err(|_| self.not_seerr())?
        };
        serde_json::from_value(value).map_err(|e| {
            tracing::debug!("{method} {path}: unexpected answer from Seerr: {e}");
            self.not_seerr()
        })
    }

    async fn send(
        &self,
        method: &Method,
        path: &str,
        query: &[(&str, Option<String>)],
        body: Option<&serde_json::Value>,
        cookie: Option<&str>,
    ) -> Result<reqwest::Response, SeerrError> {
        let qs = encode_query(query);
        let url = format!("{}/api/v1{path}{}{qs}", self.0.base_url, if qs.is_empty() { "" } else { "?" });
        let mut rb = self.0.http.request(method.clone(), url).header(ACCEPT, "application/json");
        if let SeerrAuth::ApiKey(key) = &self.0.auth {
            rb = rb.header("X-Api-Key", key);
        }
        if let Some(cookie) = cookie {
            rb = rb.header(COOKIE, cookie);
        }
        if let Some(body) = body {
            rb = rb.header(CONTENT_TYPE, "application/json").body(body.to_string());
        }
        rb.send().await.map_err(|e| self.unreachable(&e))
    }

    fn unreachable(&self, e: &reqwest::Error) -> SeerrError {
        let why = if e.is_timeout() {
            "timed out".to_owned()
        } else {
            // The cause (refused, unknown host, ...) rather than "error sending request".
            let mut cause: &dyn std::error::Error = e;
            while let Some(next) = std::error::Error::source(cause) {
                cause = next;
            }
            cause.to_string()
        };
        SeerrError::new(format!("Could not reach Seerr at {}: {why}", self.0.base_url))
    }

    fn not_seerr(&self) -> SeerrError {
        SeerrError::new(format!("{} did not answer like a Seerr server.", self.0.base_url))
    }
}

/// The error of a failed response, with Seerr's own message when it sent one.
async fn failure(resp: reqwest::Response, method: &str, path: &str) -> SeerrError {
    let status = resp.status().as_u16();
    let detail = resp
        .text()
        .await
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|json| json.get("message").and_then(|m| m.as_str().map(str::to_owned)))
        .unwrap_or_default();
    // Seerr answers a bad key or session with 403 too, so a refused sign-in check is a sign-in problem.
    if status == 401 || (status == 403 && path.starts_with("/auth/")) {
        return SeerrError::with_status(
            "Seerr did not accept the sign-in. Check the API key, or that your Plex account has access.",
            status,
        );
    }
    if status == 403 {
        let message = if detail.is_empty() { format!("Seerr refused {method} {path}.") } else { format!("Seerr refused: {detail}") };
        return SeerrError::with_status(message, status);
    }
    if (300..400).contains(&status) {
        return SeerrError::with_status(format!("Seerr redirected {method} {path}; check the address."), status);
    }
    let message = if detail.is_empty() { format!("{method} {path} failed with HTTP {status}") } else { detail };
    SeerrError::with_status(message, status)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use tokio::net::TcpListener;

    use super::*;
    use crate::seerr::testing::{Resp, serve};

    #[test]
    fn addresses_are_normalized() {
        assert_eq!(normalize_url("seerr.local:5055").unwrap(), "http://seerr.local:5055");
        assert_eq!(normalize_url(" https://requests.example.com/ ").unwrap(), "https://requests.example.com");
        assert_eq!(normalize_url("http://10.0.0.2:5055/api/v1/").unwrap(), "http://10.0.0.2:5055");
        assert_eq!(normalize_url("https://example.com/seerr/?x=1#top").unwrap(), "https://example.com/seerr");
        assert!(normalize_url("").unwrap_err().to_string().contains("Enter the address"));
        assert!(normalize_url("ftp://example.com").unwrap_err().to_string().contains("http:// or https://"));
        assert_eq!(normalize_url("http://").unwrap_err().to_string(), "“http://” is not a valid address.");
    }

    #[test]
    fn queries_encode_the_characters_seerr_rejects() {
        let query = [("query", Some("Kid's Own (Director's Cut)!*".to_owned())), ("page", Some("1".to_owned())), ("skip", None)];
        assert_eq!(encode_query(&query), "query=Kid%27s%20Own%20%28Director%27s%20Cut%29%21%2A&page=1");
        assert_eq!(encode_query(&[("q", Some("é ~-_.".to_owned()))]), "q=%C3%A9%20~-_.");
    }

    #[tokio::test]
    async fn a_plex_sign_in_is_kept_and_renewed_once_the_session_is_gone() {
        let sessions = Arc::new(AtomicUsize::new(0));
        let valid = Arc::new(Mutex::new(String::new()));
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let url = {
            let (sessions, valid, seen) = (sessions.clone(), valid.clone(), seen.clone());
            serve(move |req| {
                seen.lock().unwrap().push(format!("{} {}", req.method, req.path));
                if req.path == "/api/v1/auth/plex" {
                    assert_eq!(serde_json::from_str::<serde_json::Value>(&req.body).unwrap(), serde_json::json!({ "authToken": "plex-token" }));
                    let id = format!("s{}", sessions.fetch_add(1, Ordering::SeqCst) + 1);
                    *valid.lock().unwrap() = id.clone();
                    return Resp(200, vec![("Set-Cookie".into(), format!("connect.sid={id}; Path=/; HttpOnly"))], r#"{"id":2}"#.into());
                }
                let expected = format!("connect.sid={}", valid.lock().unwrap());
                if req.headers.get("cookie") != Some(&expected) {
                    return Resp::json(403, serde_json::json!({ "message": "no" }));
                }
                Resp::json(200, serde_json::json!({ "id": 2, "displayName": "Alice", "permissions": 32 }))
            })
            .await
        };
        let client = SeerrClient::new(&url, SeerrAuth::PlexToken("plex-token".into())).unwrap();
        assert_eq!(client.auth_kind(), SeerrAuthKind::Plex);
        let (a, b) = tokio::join!(client.me(), client.me());
        a.unwrap();
        b.unwrap();
        assert_eq!(sessions.load(Ordering::SeqCst), 1);

        *valid.lock().unwrap() = "expired".into();
        // Requests that find the session gone at the same time share one new sign-in.
        let (a, b, c) = tokio::join!(client.me(), client.me(), client.me());
        let names: Vec<_> = [a, b, c].into_iter().map(|u| u.unwrap().display_name.unwrap()).collect();
        assert_eq!(names, ["Alice", "Alice", "Alice"]);
        assert_eq!(sessions.load(Ordering::SeqCst), 2);
        assert_eq!(seen.lock().unwrap().iter().filter(|s| *s == "POST /api/v1/auth/plex").count(), 2);
    }

    #[tokio::test]
    async fn errors_carry_seerrs_message_and_a_bad_key_reads_as_a_sign_in_problem() {
        let url = serve(|req| {
            if req.headers.get("x-api-key").map(String::as_str) != Some("good") {
                return Resp::json(401, serde_json::json!({ "message": "Unauthorized" }));
            }
            Resp::json(409, serde_json::json!({ "message": "Request for this media already exists." }))
        })
        .await;
        let bad = SeerrClient::new(&url, SeerrAuth::ApiKey("bad".into())).unwrap().me().await.unwrap_err();
        assert!(bad.to_string().contains("did not accept the sign-in"));
        assert_eq!(bad.status(), Some(401));
        let good = SeerrClient::new(&url, SeerrAuth::ApiKey("good".into())).unwrap();
        let body = CreateRequest {
            media_type: SeerrMediaType::Movie,
            media_id: 1,
            tvdb_id: None,
            seasons: None,
            is_4k: false,
            server_id: None,
            profile_id: None,
            root_folder: None,
        };
        let err = good.create_request(&body).await.unwrap_err();
        assert_eq!(err.to_string(), "Request for this media already exists.");
        assert_eq!(err.status(), Some(409));
    }

    #[tokio::test]
    async fn requests_have_the_expected_shape() {
        let seen = Arc::new(Mutex::new(Vec::<(String, String, String, String, Option<String>)>::new()));
        let url = {
            let seen = seen.clone();
            serve(move |req| {
                seen.lock().unwrap().push((
                    req.method.clone(),
                    req.path.clone(),
                    req.query.clone(),
                    req.body.clone(),
                    req.headers.get("x-api-key").cloned(),
                ));
                match req.path.as_str() {
                    "/api/v1/request/7" => Resp(204, vec![], String::new()),
                    "/api/v1/forbidden" => Resp::json(403, serde_json::json!({ "message": "Not allowed" })),
                    "/api/v1/moved" => Resp(302, vec![("Location".into(), "/elsewhere".into())], String::new()),
                    "/api/v1/html" => Resp(200, vec![], "<html>".into()),
                    "/api/v1/status" => Resp::json(200, serde_json::json!({ "version": "2.7.3" })),
                    _ => Resp::json(200, serde_json::json!({ "page": 1, "totalPages": 2, "results": [{ "id": 3, "mediaType": "movie" }] })),
                }
            })
            .await
        };
        // A path at the end of the address and a trailing /api/v1 both work.
        let client = SeerrClient::new(&format!("{url}/api/v1/"), SeerrAuth::ApiKey("k".into())).unwrap();
        assert_eq!(client.base_url(), url);

        let page = client.search("a b", 2).await.unwrap();
        assert_eq!((page.page, page.total_pages, page.results[0].id), (1, 2, 3));
        let body = CreateRequest {
            media_type: SeerrMediaType::Tv,
            media_id: 8001,
            tvdb_id: Some(7001),
            seasons: Some(vec![1, 2]),
            is_4k: true,
            server_id: None,
            profile_id: Some(4),
            root_folder: Some("/tv".into()),
        };
        client.create_request(&body).await.unwrap();
        client.delete_request(7).await.unwrap();
        client.status().await.unwrap();
        client
            .requests(&RequestsQuery { take: 20, skip: 40, filter: "all".into(), sort: "added".into(), requested_by: Some(2) })
            .await
            .unwrap();

        {
            let seen = seen.lock().unwrap();
            assert_eq!(seen[0], ("GET".into(), "/api/v1/search".into(), "query=a%20b&page=2".into(), String::new(), Some("k".into())));
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&seen[1].3).unwrap(),
                serde_json::json!({ "mediaType": "tv", "mediaId": 8001, "tvdbId": 7001, "seasons": [1, 2], "is4k": true, "profileId": 4, "rootFolder": "/tv" })
            );
            assert_eq!((seen[1].0.as_str(), seen[1].1.as_str()), ("POST", "/api/v1/request"));
            assert_eq!((seen[2].0.as_str(), seen[2].1.as_str()), ("DELETE", "/api/v1/request/7"));
            assert_eq!(seen[4].2, "take=20&skip=40&filter=all&sort=added&requestedBy=2");
        }

        assert_eq!(client.get::<serde_json::Value>("/forbidden", &[]).await.unwrap_err().to_string(), "Seerr refused: Not allowed");
        let moved = client.get::<serde_json::Value>("/moved", &[]).await.unwrap_err();
        assert_eq!(moved.to_string(), "Seerr redirected GET /moved; check the address.");
        assert_eq!(moved.status(), Some(302));
        let html = client.get::<serde_json::Value>("/html", &[]).await.unwrap_err();
        assert_eq!(html.to_string(), format!("{url} did not answer like a Seerr server."));
    }

    #[tokio::test]
    async fn request_changes_accept_the_updated_request_in_the_answer() {
        let url = serve(|req| match req.method.as_str() {
            "DELETE" => Resp(204, vec![], String::new()),
            _ => Resp::json(200, serde_json::json!({ "id": 7, "status": 2, "media": { "id": 1 } })),
        })
        .await;
        let client = SeerrClient::new(&url, SeerrAuth::ApiKey("k".into())).unwrap();
        client.set_request_status(7, RequestVerdict::Approve).await.unwrap();
        client.set_request_status(7, RequestVerdict::Decline).await.unwrap();
        client.retry_request(7).await.unwrap();
        client.delete_request(7).await.unwrap();
    }

    #[tokio::test]
    async fn an_unreachable_server_is_named() {
        // Nothing listens on a port that was just freed.
        let url = {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            format!("http://{}", listener.local_addr().unwrap())
        };
        let err = SeerrClient::new(&url, SeerrAuth::ApiKey("k".into())).unwrap().me().await.unwrap_err();
        assert!(err.to_string().starts_with(&format!("Could not reach Seerr at {url}: ")), "{err}");
        assert_eq!(err.status(), None);
    }
}
