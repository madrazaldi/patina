use reqwest::Method;
use serde_json::Value;
use url::Url;

use crate::client::{ClientInfo, Http, Params};
use crate::error::{Error, Result};
use crate::models::*;

const PLEX_TV: &str = "https://plex.tv";
const CLIENTS: &str = "https://clients.plex.tv";
const DISCOVER: &str = "https://discover.provider.plex.tv";
const METADATA: &str = "https://metadata.provider.plex.tv";

/// Client for plex.tv account services.
#[derive(Clone)]
pub struct PlexTv {
    http: Http,
    token: Option<String>,
}

impl PlexTv {
    pub fn new(info: ClientInfo, token: Option<String>) -> Result<Self> {
        Ok(Self { http: Http::new(info)?, token })
    }

    pub fn with_token(&self, token: impl Into<String>) -> Self {
        Self { http: self.http.clone(), token: Some(token.into()) }
    }

    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    pub fn client_info(&self) -> &ClientInfo {
        &self.http.info
    }

    pub(crate) fn http(&self) -> &Http {
        &self.http
    }

    fn url(base: &str, path: &str) -> Result<Url> {
        Ok(Url::parse(base)?.join(path)?)
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, base: &str, path: &str, params: &Params) -> Result<T> {
        let rb = self.http.request(Method::GET, Self::url(base, path)?, self.token.as_deref(), params);
        self.http.json(rb).await
    }

    /// Escape hatch: call any plex.tv endpoint and get raw JSON back.
    pub async fn raw(&self, method: Method, url: &str, params: &Params) -> Result<Value> {
        let rb = self.http.request(method, Url::parse(url)?, self.token.as_deref(), params);
        self.http.json(rb).await
    }

    // ----- Sign-in -------------------------------------------------------

    /// Create a PIN for the link flow. `strong` PINs are long and meant to be
    /// embedded in the app.plex.tv auth URL rather than typed.
    pub async fn create_pin(&self, strong: bool) -> Result<Pin> {
        let rb = self.http.request(
            Method::POST,
            Self::url(PLEX_TV, "/api/v2/pins")?,
            None,
            &Params::new().flag("strong", strong),
        );
        self.http.json(rb).await
    }

    /// Poll a PIN; `auth_token` is set once the user approved it.
    pub async fn check_pin(&self, id: i64) -> Result<Pin> {
        let rb = self.http.request(Method::GET, Self::url(PLEX_TV, &format!("/api/v2/pins/{id}"))?, None, &Params::new());
        self.http.json(rb).await
    }

    /// URL the user opens in a browser to approve a strong PIN.
    pub fn auth_url(&self, code: &str) -> String {
        let info = &self.http.info;
        let query = Params::new()
            .set("clientID", &info.identifier)
            .set("code", code)
            .set("context[device][product]", &info.product)
            .set("context[device][platform]", &info.platform)
            .set("context[device][device]", &info.device)
            .set("context[device][deviceName]", &info.device_name)
            .set("context[device][version]", &info.version)
            .to_query_string();
        format!("https://app.plex.tv/auth#?{query}")
    }

    /// Username/password sign-in. `code` is the 2FA verification code, if enabled.
    pub async fn sign_in(&self, login: &str, password: &str, code: Option<&str>) -> Result<User> {
        let mut form = vec![("login", login), ("password", password), ("rememberMe", "true")];
        if let Some(code) = code {
            form.push(("verificationCode", code));
        }
        let rb = self
            .http
            .request(Method::POST, Self::url(PLEX_TV, "/api/v2/users/signin")?, None, &Params::new())
            .form(&form);
        self.http.json(rb).await
    }

    /// Validates the current token and returns the account.
    pub async fn user(&self) -> Result<User> {
        self.get(PLEX_TV, "/api/v2/user", &Params::new()).await
    }

    // ----- Resources -----------------------------------------------------

    /// All devices the account can access, including servers with their connections.
    pub async fn resources(&self) -> Result<Vec<Resource>> {
        let params = Params::new().set("includeHttps", 1).set("includeRelay", 1).set("includeIPv6", 1);
        self.get(CLIENTS, "/api/v2/resources", &params).await
    }

    pub async fn servers(&self) -> Result<Vec<Resource>> {
        Ok(self.resources().await?.into_iter().filter(Resource::is_server).collect())
    }

    // ----- Plex Home -----------------------------------------------------

    pub async fn home_users(&self) -> Result<Vec<HomeUser>> {
        let v: Value = self.get(CLIENTS, "/api/v2/home/users", &Params::new()).await?;
        let list = match v {
            Value::Array(_) => v,
            Value::Object(mut o) => o.remove("users").unwrap_or(Value::Array(vec![])),
            _ => Value::Array(vec![]),
        };
        serde_json::from_value(list).map_err(|e| Error::Other(format!("home users: {e}")))
    }

    /// Switch to another Plex Home user. Returns that user with a fresh `auth_token`.
    pub async fn switch_home_user(&self, uuid: &str, pin: Option<&str>) -> Result<User> {
        let params = Params::new().opt("pin", pin);
        let rb = self.http.request(
            Method::POST,
            Self::url(CLIENTS, &format!("/api/v2/home/users/{uuid}/switch"))?,
            self.token.as_deref(),
            &params,
        );
        self.http.json(rb).await
    }

    pub async fn friends(&self) -> Result<Vec<Friend>> {
        self.get(PLEX_TV, "/api/v2/friends", &Params::new()).await
    }

    // ----- Discover & watchlist -----------------------------------------

    /// The account watchlist. Items carry discover ids in `rating_key`.
    pub async fn watchlist(&self, start: u32, size: u32) -> Result<MediaContainer> {
        let params = Params::new()
            .set("includeCollections", 1)
            .set("includeExternalMedia", 1)
            .set("sort", "watchlistedAt:desc")
            .set("X-Plex-Container-Start", start)
            .set("X-Plex-Container-Size", size);
        let env: Envelope = self.get(DISCOVER, "/library/sections/watchlist/all", &params).await?;
        Ok(env.media_container)
    }

    pub async fn add_to_watchlist(&self, discover_id: &str) -> Result<()> {
        self.watchlist_action("addToWatchlist", discover_id).await
    }

    pub async fn remove_from_watchlist(&self, discover_id: &str) -> Result<()> {
        self.watchlist_action("removeFromWatchlist", discover_id).await
    }

    async fn watchlist_action(&self, action: &str, discover_id: &str) -> Result<()> {
        let rb = self.http.request(
            Method::PUT,
            Self::url(DISCOVER, &format!("/actions/{action}"))?,
            self.token.as_deref(),
            &Params::new().set("ratingKey", discover_id),
        );
        self.http.empty(rb).await
    }

    /// Watchlist/view state for a discover item.
    pub async fn user_state(&self, discover_id: &str) -> Result<Option<UserState>> {
        let v: Value = self.get(DISCOVER, &format!("/library/metadata/{discover_id}/userState"), &Params::new()).await?;
        let state = v.pointer("/MediaContainer/UserState").cloned();
        Ok(state.and_then(|s| {
            let s = if let Value::Array(mut a) = s { a.pop().unwrap_or_default() } else { s };
            serde_json::from_value(s).ok()
        }))
    }

    /// Full discover metadata (cast, reviews, ratings) for a discover id.
    pub async fn discover_metadata(&self, discover_id: &str) -> Result<Option<Metadata>> {
        let env: Envelope = self.get(METADATA, &format!("/library/metadata/{discover_id}"), &Params::new()).await?;
        Ok(env.media_container.metadata.into_iter().next())
    }

    /// Search Plex's global catalogue (movies & shows not necessarily on any server).
    pub async fn discover_search(&self, query: &str, limit: u32) -> Result<Vec<Metadata>> {
        let params = Params::new()
            .set("query", query)
            .set("limit", limit)
            .set("searchTypes", "movies,tv")
            .set("searchProviders", "discover")
            .set("includeMetadata", 1);
        let v: Value = self.get(DISCOVER, "/library/search", &params).await?;
        let mut out = Vec::new();
        if let Some(groups) = v.pointer("/MediaContainer/SearchResults").and_then(Value::as_array) {
            for group in groups {
                for result in group.get("SearchResult").and_then(Value::as_array).into_iter().flatten() {
                    if let Some(m) = result.get("Metadata")
                        && let Ok(m) = serde_json::from_value::<Metadata>(m.clone()) {
                            out.push(m);
                        }
                }
            }
        }
        Ok(out)
    }
}
