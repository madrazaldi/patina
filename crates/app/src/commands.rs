//! Tauri commands: the webview's whole API. Plex models are returned as they
//! serialize (Plex's own JSON field names); tokens never cross the boundary.

use std::collections::HashMap;

use patina_plex::{
    Activity, Account, ButlerTask, Device, Directory, Hub, LibraryQuery, MediaContainer, Metadata, MetadataOptions, Params,
    Release, ServerDevice, TranscodeSession,
};
use serde::{Serialize, Serializer};
use tauri::State;
use tauri::ipc::Channel;

use crate::config::{self, PlayerSettings, UiSettings};
use crate::player::{self, Control, PlayRequest};
use crate::session;
use crate::state::{AppState, Snapshot, Stage};
use crate::tracks;

/// Any failure, reported to the webview as its message.
pub struct Error(String);

impl<E: Into<anyhow::Error>> From<E> for Error {
    fn from(e: E) -> Self {
        Error(format!("{:#}", e.into()))
    }
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

type Res<T = ()> = Result<T, Error>;
type App<'a> = State<'a, AppState>;

// App ------------------------------------------------------------------------

#[tauri::command]
pub fn snapshot(app: App) -> Snapshot {
    app.snapshot()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    id: &'static str,
    label: &'static str,
    color: Option<&'static str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    version: &'static str,
    config_path: String,
    mpv_config_dir: String,
    mpv_conf_exists: bool,
    qualities: Vec<Choice>,
    accents: Vec<Choice>,
    /// Debug builds: `PATINA_ROUTE=/item/100` opens a page on startup.
    debug_route: Option<String>,
    /// Debug builds: `PATINA_PLAY=<ratingKey>` starts playback on startup.
    debug_play: Option<String>,
}

#[tauri::command]
pub fn info() -> Info {
    let mpv_dir = config::mpv_config_dir();
    let debug = |k: &str| if cfg!(debug_assertions) { std::env::var(k).ok() } else { None };
    Info {
        version: config::VERSION,
        config_path: config::config_dir().join("config.toml").display().to_string(),
        mpv_config_dir: mpv_dir.display().to_string(),
        mpv_conf_exists: mpv_dir.join("mpv.conf").exists(),
        qualities: config::QUALITIES.iter().map(|q| Choice { id: q.id, label: q.label, color: None }).collect(),
        accents: config::ACCENTS.iter().map(|a| Choice { id: a.id, label: a.label, color: Some(a.color) }).collect(),
        debug_route: debug("PATINA_ROUTE"),
        debug_play: debug("PATINA_PLAY"),
    }
}

#[derive(Serialize)]
pub struct PublicSettings {
    player: PlayerSettings,
    ui: UiSettings,
}

#[tauri::command]
pub fn settings(app: App) -> PublicSettings {
    let s = app.settings();
    PublicSettings { player: s.player, ui: s.ui }
}

#[tauri::command]
pub fn set_player_settings(app: App, player: PlayerSettings) {
    app.update_settings(|s| s.player = player);
}

#[tauri::command]
pub fn set_ui_settings(app: App, ui: UiSettings) {
    app.update_settings(|s| s.ui = ui);
}

/// "mpv 0.39.0" for the configured (or auto-detected) mpv.
#[tauri::command]
pub async fn probe_mpv(app: App<'_>) -> Res<String> {
    let program = app.settings().player.mpv_program();
    let version = patina_mpv::probe(&program).await.map_err(|e| anyhow::anyhow!("Could not run {program}: {e}"))?;
    // "mpv v0.41.0 Copyright © …" → "mpv v0.41.0"
    let short: String = version.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
    Ok(format!("{short} — {program}"))
}

#[tauri::command]
pub fn open_url(url: String) -> Res {
    open::that(url)?;
    Ok(())
}

#[tauri::command]
pub fn open_mpv_config() -> Res {
    let dir = config::mpv_config_dir();
    std::fs::create_dir_all(&dir)?;
    open::that(dir)?;
    Ok(())
}

// Session --------------------------------------------------------------------

#[tauri::command]
pub async fn retry(app: App<'_>) -> Res {
    app.set_stage(Stage::Booting);
    session::restore(app.inner().clone()).await;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PinView {
    id: i64,
    code: String,
    url: String,
    attempt: u64,
}

/// Start a PIN sign-in; strong PINs open the browser straight away.
#[tauri::command]
pub async fn start_pin(app: App<'_>, strong: bool) -> Res<PinView> {
    let pin = session::create_pin(&app, strong).await?;
    if strong {
        let _ = open::that(&pin.url);
    }
    Ok(PinView { id: pin.id, code: pin.code, url: pin.url, attempt: pin.attempt })
}

/// Wait for a PIN to be approved and sign in. Resolves early if cancelled.
#[tauri::command]
pub async fn complete_pin(app: App<'_>, id: i64, attempt: u64) -> Res {
    Ok(session::complete_pin(&app, id, attempt).await?)
}

#[tauri::command]
pub fn cancel_pin(app: App) {
    session::cancel_pin(&app);
}

#[tauri::command]
pub async fn password_sign_in(app: App<'_>, login: String, password: String, code: Option<String>) -> Res {
    let code = code.filter(|c| !c.trim().is_empty());
    Ok(session::password_sign_in(&app, &login, &password, code.as_deref().map(str::trim)).await?)
}

#[tauri::command]
pub async fn sign_in_manual(app: App<'_>, url: String, token: String) -> Res {
    Ok(session::sign_in_manual(&app, url.trim().to_string(), token.trim().to_string()).await?)
}

#[tauri::command]
pub async fn pick_server(app: App<'_>, id: String) -> Res {
    session::run(&app, session::pick_server(&app, &id)).await;
    Ok(())
}

#[tauri::command]
pub async fn switch_server(app: App<'_>, id: String) -> Res {
    Ok(session::switch_server(&app, &id).await?)
}

#[tauri::command]
pub async fn switch_user(app: App<'_>, uuid: String, pin: Option<String>) -> Res {
    Ok(session::switch_user(&app, &uuid, pin.as_deref()).await?)
}

#[tauri::command]
pub fn sign_out(app: App) {
    session::sign_out(&app);
}

#[tauri::command]
pub async fn reload_sections(app: App<'_>) -> Res {
    Ok(session::reload_sections(&app).await?)
}

#[tauri::command]
pub async fn home_users(app: App<'_>) -> Res<Vec<patina_plex::HomeUser>> {
    match app.tv() {
        Some(tv) => Ok(tv.home_users().await?),
        None => Ok(Vec::new()),
    }
}

// Browsing -------------------------------------------------------------------

#[tauri::command]
pub async fn home_hubs(app: App<'_>) -> Res<Vec<Hub>> {
    Ok(app.server()?.home_hubs(20).await?)
}

#[tauri::command]
pub async fn section_hubs(app: App<'_>, id: String) -> Res<Vec<Hub>> {
    Ok(app.server()?.section_hubs(&id, 18).await?)
}

#[tauri::command]
pub async fn section_sorts(app: App<'_>, id: String) -> Res<Vec<Directory>> {
    Ok(app.server()?.section_sorts(&id).await?)
}

#[tauri::command]
pub async fn section_filter_values(app: App<'_>, id: String, filter: String, kind: Option<u32>) -> Res<Vec<Directory>> {
    Ok(app.server()?.section_filter_values(&id, &filter, kind).await?)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn section_items(
    app: App<'_>,
    id: String,
    kind: Option<u32>,
    sort: Option<String>,
    unwatched: bool,
    genre: Option<String>,
    start: u32,
    size: u32,
) -> Res<MediaContainer> {
    let q = LibraryQuery {
        kind,
        sort,
        unwatched,
        filters: genre.map(|g| Params::new().set("genre", g)).unwrap_or_default(),
        start,
        size,
    };
    Ok(app.server()?.section_items(&id, &q).await?)
}

#[tauri::command]
pub async fn collections(app: App<'_>, id: String) -> Res<Vec<Metadata>> {
    Ok(app.server()?.collections(&id).await?)
}

/// Any server listing by key: "See all" for hubs, tag filters (genre, actor, ...).
#[tauri::command]
pub async fn listing(app: App<'_>, path: String, start: u32, size: u32) -> Res<MediaContainer> {
    let p = Params::new().set("X-Plex-Container-Start", start).set("X-Plex-Container-Size", size);
    Ok(app.server()?.fetch(&path, &p).await?)
}

#[tauri::command]
pub async fn search(app: App<'_>, query: String) -> Res<Vec<Hub>> {
    Ok(app.server()?.search_hubs(&query, 24, None).await?)
}

#[tauri::command]
pub async fn metadata(app: App<'_>, rating_key: String) -> Res<Option<Metadata>> {
    Ok(app.server()?.metadata(&rating_key, MetadataOptions::full()).await?)
}

/// Seasons, episodes, albums, tracks or collection items of an item.
#[tauri::command]
pub async fn children(app: App<'_>, rating_key: String, collection: bool) -> Res<Vec<Metadata>> {
    let server = app.server()?;
    let mc = if collection { server.collection_items(&rating_key).await? } else { server.children(&rating_key).await? };
    Ok(mc.metadata)
}

/// Rating key of the local copy of a plex.tv item, if the server has one.
#[tauri::command]
pub async fn find_by_guid(app: App<'_>, guid: String) -> Res<Option<String>> {
    let found = app.server()?.find_by_guid(&guid).await?;
    Ok(found.into_iter().next().and_then(|m| m.rating_key))
}

// Item actions ---------------------------------------------------------------

#[tauri::command]
pub async fn set_watched(app: App<'_>, rating_key: String, watched: bool) -> Res {
    let server = app.server()?;
    if watched { server.scrobble(&rating_key).await? } else { server.unscrobble(&rating_key).await? }
    Ok(())
}

#[tauri::command]
pub async fn rate(app: App<'_>, rating_key: String, rating: Option<f64>) -> Res {
    Ok(app.server()?.rate(&rating_key, rating).await?)
}

/// Choose the audio and/or subtitle stream of a part (0 turns subtitles off).
#[tauri::command]
pub async fn select_streams(app: App<'_>, part_id: i64, audio: Option<i64>, subtitle: Option<i64>) -> Res {
    Ok(app.server()?.select_streams(part_id, audio, subtitle, true).await?)
}

#[tauri::command]
pub async fn refresh_metadata(app: App<'_>, rating_key: String) -> Res {
    Ok(app.server()?.refresh_metadata(&rating_key, true).await?)
}

#[tauri::command]
pub async fn analyze_metadata(app: App<'_>, rating_key: String) -> Res {
    Ok(app.server()?.analyze_metadata(&rating_key).await?)
}

// Watchlist ------------------------------------------------------------------

fn tv(app: &AppState) -> Result<patina_plex::PlexTv, Error> {
    app.tv().ok_or_else(|| anyhow::anyhow!("Not signed in to plex.tv.").into())
}

#[tauri::command]
pub async fn watchlist(app: App<'_>) -> Res<Vec<Metadata>> {
    Ok(tv(&app)?.watchlist(0, 300).await?.metadata)
}

#[tauri::command]
pub async fn watchlisted(app: App<'_>, discover_id: String) -> Res<bool> {
    let state = tv(&app)?.user_state(&discover_id).await?;
    Ok(state.is_some_and(|s| s.watchlisted_at.is_some()))
}

#[tauri::command]
pub async fn set_watchlisted(app: App<'_>, discover_id: String, on: bool) -> Res {
    let tv = tv(&app)?;
    if on { tv.add_to_watchlist(&discover_id).await? } else { tv.remove_from_watchlist(&discover_id).await? }
    Ok(())
}

// Playlists ------------------------------------------------------------------

/// All playlists, or the non-smart ones of one kind (`video`, `audio`, `photo`).
#[tauri::command]
pub async fn playlists(app: App<'_>, kind: Option<String>) -> Res<Vec<Metadata>> {
    let smart = kind.is_some().then_some(false);
    Ok(app.server()?.playlists(kind.as_deref(), smart).await?)
}

#[derive(Serialize)]
pub struct PlaylistView {
    playlist: Metadata,
    items: Vec<Metadata>,
}

#[tauri::command]
pub async fn playlist(app: App<'_>, id: String) -> Res<Option<PlaylistView>> {
    let server = app.server()?;
    let Some(playlist) = server.playlist(&id).await? else { return Ok(None) };
    let items = server.playlist_items(&id, 0, 2000).await?.metadata;
    Ok(Some(PlaylistView { playlist, items }))
}

/// Add an item to an existing playlist, or to a new one called `title`.
#[tauri::command]
pub async fn add_to_playlist(app: App<'_>, rating_key: String, playlist: Option<String>, title: Option<String>, kind: String) -> Res {
    let server = app.server()?;
    let uri = server.item_uri(&[&rating_key]);
    match (playlist, title) {
        (Some(id), _) => server.add_to_playlist(&id, &uri).await?,
        (None, Some(title)) => {
            server.create_playlist(&title, &kind, &uri, false).await?;
        }
        (None, None) => return Err(anyhow::anyhow!("No playlist given.").into()),
    }
    Ok(())
}

#[tauri::command]
pub async fn remove_from_playlist(app: App<'_>, id: String, item_id: i64) -> Res {
    Ok(app.server()?.remove_from_playlist(&id, item_id).await?)
}

#[tauri::command]
pub async fn delete_playlist(app: App<'_>, id: String) -> Res {
    Ok(app.server()?.delete_playlist(&id).await?)
}

// Playback -------------------------------------------------------------------

#[tauri::command]
pub fn play(app: App, request: PlayRequest) {
    player::play(app.inner().clone(), request);
}

#[tauri::command]
pub fn control(command: Control) {
    player::control(command);
}

// Bulk audio & subtitles -----------------------------------------------------

#[tauri::command]
pub async fn tracks_load(app: App<'_>, show: String) -> Res {
    Ok(tracks::load(&app, &show).await?)
}

#[tauri::command]
pub fn tracks_view(app: App, query: tracks::Query) -> Res<tracks::View> {
    Ok(tracks::view(&app, &query)?)
}

#[tauri::command]
pub async fn tracks_apply(app: App<'_>, changes: Vec<tracks::PartChange>, progress: Channel<[usize; 2]>) -> Res {
    tracks::apply(&app, changes, |done, total| {
        let _ = progress.send([done, total]);
    })
    .await?;
    Ok(())
}

// Activity -------------------------------------------------------------------

#[tauri::command]
pub async fn sessions(app: App<'_>) -> Res<Vec<Metadata>> {
    Ok(app.server()?.sessions().await?)
}

#[tauri::command]
pub async fn terminate_session(app: App<'_>, id: String) -> Res {
    Ok(app.server()?.terminate_session(&id, "Stopped by the server owner.").await?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    items: Vec<Metadata>,
    has_more: bool,
    /// Account names by id (owners only).
    accounts: HashMap<i64, String>,
}

#[tauri::command]
pub async fn history(app: App<'_>, page: u32) -> Res<HistoryPage> {
    const SIZE: u32 = 60;
    let server = app.server()?;
    let owned = app.session().is_some_and(|s| s.owned);
    let mc = server.history(&patina_plex::HistoryQuery { start: page * SIZE, size: SIZE, ..Default::default() }).await?;
    let accounts = if owned {
        server.accounts().await.unwrap_or_default().into_iter().filter_map(|a| Some((a.id?, a.name?))).collect()
    } else {
        HashMap::new()
    };
    let has_more = mc.has_more() || mc.metadata.len() == SIZE as usize;
    Ok(HistoryPage { items: mc.metadata, has_more, accounts })
}

/// Delete a history entry by its `historyKey`.
#[tauri::command]
pub async fn delete_history(app: App<'_>, key: String) -> Res {
    let id = key.rsplit('/').next().unwrap_or("");
    Ok(app.server()?.delete_history(id).await?)
}

#[derive(Serialize)]
pub struct Tasks {
    activities: Vec<Activity>,
    transcodes: Vec<TranscodeSession>,
}

#[tauri::command]
pub async fn tasks(app: App<'_>) -> Res<Tasks> {
    let server = app.server()?;
    let (activities, transcodes) = tokio::join!(server.activities(), server.transcode_sessions());
    Ok(Tasks { activities: activities.unwrap_or_default(), transcodes: transcodes.unwrap_or_default() })
}

#[tauri::command]
pub async fn cancel_activity(app: App<'_>, uuid: String) -> Res {
    Ok(app.server()?.cancel_activity(&uuid).await?)
}

#[tauri::command]
pub async fn kill_transcode(app: App<'_>, key: String) -> Res {
    let id = key.rsplit('/').next().unwrap_or("");
    Ok(app.server()?.kill_transcode_session(id).await?)
}

// Server administration ------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    version: Option<String>,
    platform: Option<String>,
    owner: Option<String>,
    plex_pass: bool,
    machine_identifier: Option<String>,
    transcoder: bool,
    connected_via: String,
    host_cpu: f64,
    plex_cpu: f64,
    host_memory: f64,
    plex_memory: f64,
    release: Option<Release>,
    can_install: bool,
}

#[tauri::command]
pub async fn server_overview(app: App<'_>) -> Res<Overview> {
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

#[tauri::command]
pub async fn check_for_updates(app: App<'_>) -> Res {
    Ok(app.server()?.check_for_updates(true).await?)
}

#[tauri::command]
pub async fn apply_update(app: App<'_>, tonight: bool) -> Res {
    Ok(app.server()?.apply_update(tonight, false).await?)
}

/// Library maintenance: `scan`, `refresh` (re-download metadata), `analyze` or `emptyTrash`.
#[tauri::command]
pub async fn section_action(app: App<'_>, id: String, action: String) -> Res {
    let server = app.server()?;
    match action.as_str() {
        "scan" => server.refresh_section(&id, false, None).await?,
        "refresh" => server.refresh_section(&id, true, None).await?,
        "analyze" => server.analyze_section(&id).await?,
        "emptyTrash" => server.empty_trash(&id).await?,
        other => return Err(anyhow::anyhow!("Unknown library action {other}.").into()),
    }
    Ok(())
}

/// Server maintenance: `scanAll`, `cleanBundles` or `optimizeDatabase`.
#[tauri::command]
pub async fn server_action(app: App<'_>, action: String) -> Res {
    let server = app.server()?;
    match action.as_str() {
        "scanAll" => server.refresh_all_sections(false).await?,
        "cleanBundles" => server.clean_bundles().await?,
        "optimizeDatabase" => server.optimize_database().await?,
        other => return Err(anyhow::anyhow!("Unknown server action {other}.").into()),
    }
    Ok(())
}

#[tauri::command]
pub async fn butler_tasks(app: App<'_>) -> Res<Vec<ButlerTask>> {
    Ok(app.server()?.butler_tasks().await?)
}

#[tauri::command]
pub async fn butler_task(app: App<'_>, name: String, start: bool) -> Res {
    let server = app.server()?;
    if start { server.start_butler_task(&name).await? } else { server.stop_butler_task(&name).await? }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pref {
    id: String,
    label: String,
    summary: Option<String>,
    kind: String,
    value: String,
    options: Vec<(String, String)>,
    advanced: bool,
}

#[tauri::command]
pub async fn prefs(app: App<'_>) -> Res<Vec<Pref>> {
    let list = app.server()?.prefs().await?;
    Ok(list
        .into_iter()
        .filter(|s| !s.hidden)
        .map(|s| Pref {
            value: s.value_string(),
            options: s.enum_options(),
            id: s.id.clone().unwrap_or_default(),
            label: s.label.clone().or(s.id.clone()).unwrap_or_default(),
            summary: s.summary.clone(),
            kind: s.kind.clone().unwrap_or_default(),
            advanced: s.advanced,
        })
        .collect())
}

#[tauri::command]
pub async fn set_pref(app: App<'_>, id: String, value: String) -> Res {
    Ok(app.server()?.set_pref(&id, &value).await?)
}

#[derive(Serialize)]
pub struct UsersDevices {
    accounts: Vec<Account>,
    devices: Vec<Device>,
    clients: Vec<ServerDevice>,
}

#[tauri::command]
pub async fn users_devices(app: App<'_>) -> Res<UsersDevices> {
    let server = app.server()?;
    let (accounts, devices, clients) = tokio::join!(server.accounts(), server.devices(), server.clients());
    Ok(UsersDevices {
        accounts: accounts.unwrap_or_default(),
        devices: devices.unwrap_or_default(),
        clients: clients.unwrap_or_default(),
    })
}
