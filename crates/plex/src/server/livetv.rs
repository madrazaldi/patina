//! Live TV & DVR. These endpoints vary a lot between tuner setups and server
//! versions, so they return raw JSON for the caller to pick apart.

use reqwest::Method;
use serde_json::Value;

use super::PlexServer;
use crate::client::Params;
use crate::error::Result;

impl PlexServer {
    /// Configured DVRs, each with its tuner devices, lineup and channel mappings.
    pub async fn dvrs(&self) -> Result<Value> {
        self.raw(Method::GET, "/livetv/dvrs", &Params::new()).await
    }

    /// Channels of a DVR lineup. `epg` is the EPG provider path, e.g. `/tv.plex.providers.epg.cloud:2`.
    pub async fn lineup_channels(&self, epg: &str, lineup: &str) -> Result<Value> {
        let p = Params::new().set("lineup", lineup);
        self.raw(Method::GET, &format!("{}/lineups/dvr/channels", epg.trim_end_matches('/')), &p).await
    }

    /// Guide listings between two unix timestamps.
    pub async fn epg_grid(&self, epg: &str, begins: i64, ends: i64) -> Result<Value> {
        let p = Params::new().set("type", "1,4").set("beginsAt<", ends).set("endsAt>", begins);
        self.raw(Method::GET, &format!("{}/grid", epg.trim_end_matches('/')), &p).await
    }

    /// Start watching a channel. The response describes the new live session.
    pub async fn tune(&self, dvr_id: &str, channel: &str) -> Result<Value> {
        let path = format!("/livetv/dvrs/{dvr_id}/channels/{}/tune", urlencode(channel));
        self.raw(Method::POST, &path, &Params::new().set("autoPreview", 1)).await
    }

    /// Recording rules, with their upcoming grabs.
    pub async fn subscriptions(&self) -> Result<Value> {
        self.raw(Method::GET, "/media/subscriptions", &Params::new().set("includeGrabs", 1)).await
    }

    /// Upcoming recordings.
    pub async fn scheduled_recordings(&self) -> Result<Value> {
        self.raw(Method::GET, "/media/subscriptions/scheduled", &Params::new()).await
    }

    /// The recording template for a guide item (its default rule parameters).
    pub async fn subscription_template(&self, guid: &str) -> Result<Value> {
        self.raw(Method::GET, "/media/subscriptions/template", &Params::new().set("guid", guid)).await
    }

    /// Create a recording rule. `params` usually come from [`Self::subscription_template`].
    pub async fn create_subscription(&self, params: &Params) -> Result<Value> {
        self.raw(Method::POST, "/media/subscriptions", params).await
    }

    pub async fn delete_subscription(&self, id: &str) -> Result<()> {
        self.delete(&format!("/media/subscriptions/{id}"), &Params::new()).await
    }
}

fn urlencode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}
