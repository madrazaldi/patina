use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A PIN used for the plex.tv/link sign-in flow.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Pin {
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub id: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub code: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub auth_token: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub expires_at: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub expires_in: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub trusted: bool,
}

/// The signed-in plex.tv account.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct User {
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub id: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub uuid: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub username: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub friendly_name: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub email: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub thumb: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub auth_token: Option<String>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub home_admin: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub restricted: bool,
    #[serde(default)]
    pub subscription: Option<Subscription>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl User {
    pub fn display_name(&self) -> &str {
        self.friendly_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .or(self.title.as_deref())
            .or(self.username.as_deref())
            .unwrap_or("Plex user")
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Subscription {
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub active: bool,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub status: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub plan: Option<String>,
}

/// A device linked to the account (servers, players) from `/api/v2/resources`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub product: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub product_version: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub platform: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub platform_version: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub device: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub client_identifier: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub created_at: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub last_seen_at: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub provides: Option<String>,
    #[serde(rename = "ownerId", default, deserialize_with = "crate::de::opt_i64")]
    pub owner_id: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub source_title: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub public_address: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub access_token: Option<String>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub owned: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub home: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub synced: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub relay: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub presence: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub https_required: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub public_address_matches: bool,
    #[serde(default)]
    pub connections: Vec<Connection>,
}

impl Resource {
    pub fn name(&self) -> &str {
        self.name.as_deref().unwrap_or("Plex Media Server")
    }

    pub fn is_server(&self) -> bool {
        self.provides.as_deref().unwrap_or("").split(',').any(|p| p == "server")
    }

    pub fn id(&self) -> &str {
        self.client_identifier.as_deref().unwrap_or("")
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub protocol: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub address: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub port: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub uri: Option<String>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub local: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub relay: bool,
    #[serde(rename = "IPv6", default, deserialize_with = "crate::de::boolean")]
    pub ipv6: bool,
}

/// A member of the Plex Home.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HomeUser {
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub id: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub uuid: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub username: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub email: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub thumb: Option<String>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub restricted: bool,
    /// Whether switching to this user requires a PIN.
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub protected: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub admin: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub guest: bool,
}

impl HomeUser {
    pub fn name(&self) -> &str {
        self.title.as_deref().or(self.username.as_deref()).unwrap_or("User")
    }
}

/// A friend / shared user.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Friend {
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub id: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub uuid: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub username: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub email: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub thumb: Option<String>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub restricted: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub home: bool,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub status: Option<String>,
}

/// Per-user state for a discover item (watchlist membership, views).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UserState {
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub watchlisted_at: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub view_count: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub view_offset: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub last_viewed_at: Option<i64>,
}
