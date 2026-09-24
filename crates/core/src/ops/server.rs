//! Server administration: activity, overview, settings, accounts, libraries,
//! statistics and diagnostics.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use patina_plex::{
    Account, Activity, Device, Friend, MediaContainer, MediaType, Params, Release, ServerDevice, Setting, TranscodeSession,
};
use serde::{Deserialize, Serialize};

use crate::ops::lists::last_segment;
use crate::session;
use crate::state::AppState;

#[derive(Serialize)]
pub struct Tasks {
    pub activities: Vec<Activity>,
    pub transcodes: Vec<TranscodeSession>,
}

/// Background activities and transcoder sessions.
pub async fn tasks(app: &AppState) -> Result<Tasks> {
    let server = app.server()?;
    let (activities, transcodes) = tokio::join!(server.activities(), server.transcode_sessions());
    Ok(Tasks { activities: activities.unwrap_or_default(), transcodes: transcodes.unwrap_or_default() })
}

/// Stop a transcoder session by its key.
pub async fn kill_transcode(app: &AppState, key: &str) -> Result<()> {
    Ok(app.server()?.kill_transcode_session(last_segment(key)).await?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub version: Option<String>,
    pub platform: Option<String>,
    pub owner: Option<String>,
    pub plex_pass: bool,
    pub machine_identifier: Option<String>,
    pub transcoder: bool,
    pub connected_via: String,
    pub host_cpu: f64,
    pub plex_cpu: f64,
    pub host_memory: f64,
    pub plex_memory: f64,
    pub release: Option<Release>,
    pub can_install: bool,
}

pub async fn server_overview(app: &AppState) -> Result<Overview> {
    let server = app.server()?;
    let (caps, update, resources) =
        tokio::join!(server.capabilities(), server.update_status(), server.statistics_resources(6));
    let caps = caps?;
    let update = update.ok();
    let latest = resources.unwrap_or_default().pop().unwrap_or_default();
    let flag = |k: &str| caps.attr(k).is_some_and(|v| v == "true" || v == "1");
    let platform = match (caps.attr("platform"), caps.attr("platformVersion")) {
        (Some(p), Some(v)) => Some(format!("{p} {v}")),
        (p, _) => p,
    };
    Ok(Overview {
        version: caps.attr("version"),
        platform,
        owner: caps.attr("myPlexUsername"),
        plex_pass: flag("myPlexSubscription"),
        machine_identifier: caps.attr("machineIdentifier"),
        transcoder: flag("transcoderVideo"),
        connected_via: server.base_url().to_string(),
        host_cpu: latest.host_cpu_utilization.unwrap_or(0.0),
        plex_cpu: latest.process_cpu_utilization.unwrap_or(0.0),
        host_memory: latest.host_memory_utilization.unwrap_or(0.0),
        plex_memory: latest.process_memory_utilization.unwrap_or(0.0),
        can_install: update.as_ref().is_some_and(|u| u.can_install),
        release: update.and_then(|u| u.release.into_iter().next()),
    })
}

/// A server or library setting.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pref {
    pub id: String,
    pub label: String,
    pub summary: Option<String>,
    pub kind: String,
    pub value: String,
    pub options: Vec<(String, String)>,
    pub advanced: bool,
}

impl From<Setting> for Pref {
    fn from(s: Setting) -> Self {
        Pref {
            value: s.value_string(),
            options: s.enum_options(),
            id: s.id.clone().unwrap_or_default(),
            label: s.label.clone().or(s.id.clone()).unwrap_or_default(),
            summary: s.summary.clone(),
            kind: s.kind.clone().unwrap_or_default(),
            advanced: s.advanced,
        }
    }
}

fn visible(list: Vec<Setting>) -> Vec<Pref> {
    list.into_iter().filter(|s| !s.hidden).map(Pref::from).collect()
}

/// The server's settings, without hidden ones.
pub async fn prefs(app: &AppState) -> Result<Vec<Pref>> {
    Ok(visible(app.server()?.prefs().await?))
}

/// A library's settings, without hidden ones.
pub async fn library_prefs(app: &AppState, id: &str) -> Result<Vec<Pref>> {
    Ok(visible(app.server()?.section_prefs(id).await?))
}

#[derive(Serialize)]
pub struct UsersDevices {
    pub accounts: Vec<Account>,
    pub devices: Vec<Device>,
    pub clients: Vec<ServerDevice>,
    pub friends: Vec<Friend>,
}

pub async fn users_devices(app: &AppState) -> Result<UsersDevices> {
    let server = app.server()?;
    let tv = app.tv();
    let friends = async {
        match &tv {
            Some(tv) => tv.friends().await.unwrap_or_default(),
            None => Vec::new(),
        }
    };
    let (accounts, devices, clients, friends) = tokio::join!(server.accounts(), server.devices(), server.clients(), friends);
    Ok(UsersDevices {
        accounts: accounts.unwrap_or_default(),
        devices: devices.unwrap_or_default(),
        clients: clients.unwrap_or_default(),
        friends,
    })
}

// Libraries --------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Choices {
    /// `(identifier, title)`.
    pub agents: Vec<(String, String)>,
    pub scanners: Vec<String>,
}

/// Agents and scanners for a new library of `kind` (`movie`, `show`, `artist`, `photo`).
pub async fn library_choices(app: &AppState, kind: &str) -> Result<Choices> {
    let server = app.server()?;
    let id = MediaType::from_name(kind).map(MediaType::id).unwrap_or(1);
    let (agents, scanners) = tokio::join!(server.agents(Some(id)), server.scanners(id));
    let pick = |mc: Option<MediaContainer>, key: &str| -> Vec<serde_json::Value> {
        match mc.and_then(|m| m.extra.get(key).cloned()) {
            Some(serde_json::Value::Array(a)) => a,
            Some(o @ serde_json::Value::Object(_)) => vec![o],
            _ => Vec::new(),
        }
    };
    let s = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
    Ok(Choices {
        agents: pick(agents.ok(), "Agent").iter().filter_map(|a| Some((s(a, "identifier")?, s(a, "title").unwrap_or_default()))).collect(),
        scanners: pick(scanners.ok(), "Scanner").iter().filter_map(|a| s(a, "name")).collect(),
    })
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryForm {
    pub name: String,
    pub kind: String,
    pub agent: String,
    pub scanner: String,
    pub language: String,
    pub locations: Vec<String>,
}

fn push_locations(p: &mut Params, locations: &[String]) {
    for l in locations.iter().filter(|l| !l.trim().is_empty()) {
        p.push("location", l.trim());
    }
}

pub async fn create_library(app: &AppState, form: &LibraryForm) -> Result<()> {
    let mut p = Params::new()
        .set("name", form.name.trim())
        .set("type", &form.kind)
        .set("agent", &form.agent)
        .set("scanner", &form.scanner)
        .set("language", &form.language);
    push_locations(&mut p, &form.locations);
    app.server()?.create_section(&p).await?;
    session::reload_sections(app).await
}

/// Rename a library and/or change its folders.
pub async fn edit_library(app: &AppState, id: &str, name: &str, locations: &[String]) -> Result<()> {
    let section = app.sections().into_iter().find(|s| s.key.as_deref() == Some(id));
    let mut p = Params::new()
        .set("name", name.trim())
        .opt("agent", section.as_ref().and_then(|s| s.agent.clone()))
        .opt("scanner", section.as_ref().and_then(|s| s.scanner.clone()))
        .opt("language", section.as_ref().and_then(|s| s.language.clone()));
    push_locations(&mut p, locations);
    app.server()?.edit_section(id, &p).await?;
    session::reload_sections(app).await
}

pub async fn delete_library(app: &AppState, id: &str) -> Result<()> {
    app.server()?.delete_section(id).await?;
    session::reload_sections(app).await
}

// Statistics -------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bucket {
    pub at: i64,
    pub lan: i64,
    pub wan: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub name: String,
    pub bytes: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    /// Bytes streamed per time bucket, oldest first.
    pub bandwidth: Vec<Bucket>,
    /// Bytes streamed per account, most first.
    pub accounts: Vec<Usage>,
    /// Bytes streamed per device, most first.
    pub devices: Vec<Usage>,
    pub plays: i64,
    pub play_seconds: i64,
}

/// Bandwidth and plays since `since` (unix seconds). `timespan` is Plex's
/// bucket size code: 6 for hours, 4 for days.
pub async fn server_stats(app: &AppState, timespan: u32, since: i64) -> Result<Stats> {
    let server = app.server()?;
    let (bandwidth, media) = tokio::join!(server.statistics_bandwidth(timespan), server.statistics_media(timespan));
    let bandwidth = bandwidth?;
    let names = |mc: &MediaContainer| -> (HashMap<i64, String>, HashMap<i64, String>) {
        let accounts = mc.account.iter().filter_map(|a| Some((a.id?, a.name.clone()?))).collect();
        let devices = mc.device.iter().filter_map(|d| Some((d.id?, d.name.clone()?))).collect();
        (accounts, devices)
    };
    let (account_names, device_names) = names(&bandwidth);
    let mut buckets: BTreeMap<i64, Bucket> = BTreeMap::new();
    let mut by_account: HashMap<i64, i64> = HashMap::new();
    let mut by_device: HashMap<i64, i64> = HashMap::new();
    for b in bandwidth.statistics_bandwidth.iter().filter(|b| b.at.unwrap_or(0) >= since) {
        let at = b.at.unwrap_or(0);
        let bytes = b.bytes.unwrap_or(0);
        let e = buckets.entry(at).or_insert(Bucket { at, lan: 0, wan: 0 });
        if b.lan { e.lan += bytes } else { e.wan += bytes }
        *by_account.entry(b.account_id.unwrap_or(0)).or_default() += bytes;
        *by_device.entry(b.device_id.unwrap_or(0)).or_default() += bytes;
    }
    let rank = |map: HashMap<i64, i64>, names: &HashMap<i64, String>, unknown: &str| {
        let mut v: Vec<Usage> = map
            .into_iter()
            .map(|(id, bytes)| Usage { name: names.get(&id).cloned().unwrap_or_else(|| unknown.to_string()), bytes })
            .collect();
        v.sort_by_key(|u| std::cmp::Reverse(u.bytes));
        v.truncate(8);
        v
    };
    let media = media.map(|m| m.statistics_media).unwrap_or_default();
    let recent = media.iter().filter(|m| m.at.unwrap_or(0) >= since);
    let (plays, play_seconds) = recent.fold((0, 0), |(n, s), m| (n + m.count.unwrap_or(0), s + m.duration.unwrap_or(0)));
    Ok(Stats {
        bandwidth: buckets.into_values().collect(),
        accounts: rank(by_account, &account_names, "Unknown user"),
        devices: rank(by_device, &device_names, "Unknown device"),
        plays,
        play_seconds,
    })
}

/// Save the server's logs (`logs`) or database (`database`) as a zip in the
/// user's Downloads folder (else the downloads folder). Returns the file.
pub async fn save_diagnostics(app: &AppState, what: &str) -> Result<PathBuf> {
    let server = app.server()?;
    let bytes = match what {
        "logs" => server.download_logs().await?,
        "database" => server.download_database().await?,
        other => return Err(anyhow!("Unknown diagnostics {other}.")),
    };
    let dir = directories::UserDirs::new()
        .and_then(|d| d.download_dir().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| app.settings().player.downloads_dir());
    std::fs::create_dir_all(&dir)?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let path = dir.join(format!("plex-{what}-{stamp}.zip"));
    std::fs::write(&path, bytes)?;
    Ok(path)
}
