use reqwest::Method;
use serde_json::Value;

use super::PlexServer;
use crate::client::Params;
use crate::error::Result;
use crate::models::*;

impl PlexServer {
    // ----- Preferences -----------------------------------------------------

    pub async fn prefs(&self) -> Result<Vec<Setting>> {
        Ok(self.get("/:/prefs", &Params::new()).await?.setting)
    }

    pub async fn set_pref(&self, id: &str, value: &str) -> Result<()> {
        self.put("/:/prefs", &Params::new().set(id, value)).await
    }

    pub async fn set_prefs(&self, params: &Params) -> Result<()> {
        self.put("/:/prefs", params).await
    }

    // ----- Accounts, devices, clients --------------------------------------

    pub async fn accounts(&self) -> Result<Vec<Account>> {
        Ok(self.get("/accounts", &Params::new()).await?.account)
    }

    pub async fn devices(&self) -> Result<Vec<Device>> {
        Ok(self.get("/devices", &Params::new()).await?.device)
    }

    /// Players discovered on the local network (GDM).
    pub async fn clients(&self) -> Result<Vec<ServerDevice>> {
        Ok(self.get("/clients", &Params::new()).await?.server)
    }

    pub async fn servers(&self) -> Result<Vec<ServerDevice>> {
        Ok(self.get("/servers", &Params::new()).await?.server)
    }

    /// Media providers exposed by the server (library, live TV, ...), raw.
    pub async fn media_providers(&self) -> Result<Value> {
        self.raw(Method::GET, "/media/providers", &Params::new()).await
    }

    // ----- Activities ------------------------------------------------------

    pub async fn activities(&self) -> Result<Vec<Activity>> {
        Ok(self.get("/activities", &Params::new()).await?.activity)
    }

    pub async fn cancel_activity(&self, uuid: &str) -> Result<()> {
        self.delete(&format!("/activities/{uuid}"), &Params::new()).await
    }

    // ----- Butler (scheduled maintenance) ----------------------------------

    pub async fn butler_tasks(&self) -> Result<Vec<ButlerTask>> {
        let v = self.raw(Method::GET, "/butler", &Params::new()).await?;
        let list = v.pointer("/ButlerTasks/ButlerTask").cloned().unwrap_or_default();
        Ok(serde_json::from_value(list).unwrap_or_default())
    }

    pub async fn start_all_butler_tasks(&self) -> Result<()> {
        self.post("/butler", &Params::new()).await
    }

    pub async fn stop_all_butler_tasks(&self) -> Result<()> {
        self.delete("/butler", &Params::new()).await
    }

    pub async fn start_butler_task(&self, name: &str) -> Result<()> {
        self.post(&format!("/butler/{name}"), &Params::new()).await
    }

    pub async fn stop_butler_task(&self, name: &str) -> Result<()> {
        self.delete(&format!("/butler/{name}"), &Params::new()).await
    }

    // ----- Updates -----------------------------------------------------------

    pub async fn update_status(&self) -> Result<MediaContainer> {
        self.get("/updater/status", &Params::new()).await
    }

    pub async fn check_for_updates(&self, download: bool) -> Result<()> {
        self.put("/updater/check", &Params::new().flag("download", download)).await
    }

    /// Install a downloaded update now, or `tonight` during maintenance.
    pub async fn apply_update(&self, tonight: bool, skip: bool) -> Result<()> {
        self.put("/updater/apply", &Params::new().flag("tonight", tonight).flag("skip", skip)).await
    }

    // ----- Statistics --------------------------------------------------------

    /// Plays per account/device, bucketed by the server-defined `timespan` code.
    pub async fn statistics_media(&self, timespan: u32) -> Result<MediaContainer> {
        self.get("/statistics/media", &Params::new().set("timespan", timespan)).await
    }

    /// CPU and memory utilisation samples.
    pub async fn statistics_resources(&self, timespan: u32) -> Result<Vec<StatisticsResources>> {
        Ok(self.get("/statistics/resources", &Params::new().set("timespan", timespan)).await?.statistics_resources)
    }

    pub async fn statistics_bandwidth(&self, timespan: u32) -> Result<MediaContainer> {
        self.get("/statistics/bandwidth", &Params::new().set("timespan", timespan)).await
    }

    // ----- Logging -----------------------------------------------------------

    /// Write a line into the server log. `level`: 0 error .. 4 verbose.
    pub async fn log(&self, level: u8, message: &str, source: &str) -> Result<()> {
        let p = Params::new().set("level", level).set("message", message).set("source", source);
        self.empty(Method::GET, "/log", &p).await
    }

    /// Diagnostics archive with the server logs (zip bytes).
    pub async fn download_logs(&self) -> Result<Vec<u8>> {
        Ok(self.bytes("/diagnostics/logs", &Params::new()).await?.0)
    }

    /// Download the server database backup (zip bytes).
    pub async fn download_database(&self) -> Result<Vec<u8>> {
        Ok(self.bytes("/diagnostics/databases", &Params::new()).await?.0)
    }
}
