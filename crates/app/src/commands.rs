//! Tauri commands: the webview's whole API. Plex models are returned as they
//! serialize (Plex's own JSON field names); tokens never cross the boundary.

use std::collections::HashMap;

use patina_plex::{
    Activity, Account, ButlerTask, Device, Directory, Friend, Hub, LibraryQuery, MatchResult, MediaContainer, MediaType,
    Metadata, MetadataOptions, Params, Release, ServerDevice, Stream, TranscodeSession,
};
use serde::{Serialize, Serializer};
use tauri::State;
use tauri::ipc::Channel;

use crate::config::{self, PlayerSettings, UiSettings};
use crate::player::{self, Control, PlayRequest, QueueCommand};
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
    filters: Vec<(String, String)>,
    start: u32,
    size: u32,
) -> Res<MediaContainer> {
    let q = LibraryQuery { kind, sort, unwatched, filters: filters.into_iter().collect(), start, size };
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

fn pref_view(s: patina_plex::Setting) -> Pref {
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

#[tauri::command]
pub async fn prefs(app: App<'_>) -> Res<Vec<Pref>> {
    let list = app.server()?.prefs().await?;
    Ok(list.into_iter().filter(|s| !s.hidden).map(pref_view).collect())
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
    friends: Vec<Friend>,
}

#[tauri::command]
pub async fn users_devices(app: App<'_>) -> Res<UsersDevices> {
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

// Live updates, queue, lyrics ----------------------------------------------

#[tauri::command]
pub fn queue(app: App, command: QueueCommand) {
    player::queue(app.inner().clone(), command);
}

#[tauri::command]
pub async fn lyrics(app: App<'_>, rating_key: String) -> Res<Option<crate::lyrics::Lyrics>> {
    Ok(crate::lyrics::load(&app.server()?, &rating_key).await?)
}

// Library browsing: filters, jump bar, folders, related ---------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterView {
    filter: String,
    title: String,
    /// `string`, `integer`, `tag` or `boolean`.
    kind: String,
}

/// Filters for a section and media type (genre, year, resolution, unwatched, ...).
#[tauri::command]
pub async fn section_filters(app: App<'_>, id: String, kind: Option<u32>) -> Res<Vec<FilterView>> {
    let server = app.server()?;
    let view = |d: &Directory| {
        Some(FilterView {
            filter: d.filter.clone()?,
            title: d.title.clone().unwrap_or_default(),
            kind: d.filter_type.clone().unwrap_or_else(|| "string".into()),
        })
    };
    if let Some(meta) = server.section_meta(&id).await? {
        let of_kind = |t: &&patina_plex::LibraryMetaType| t.kind.as_deref().and_then(MediaType::from_name).map(MediaType::id) == kind;
        if let Some(t) = meta.types.iter().find(of_kind).or(meta.types.first()) {
            return Ok(t.filters.iter().filter_map(view).collect());
        }
    }
    Ok(server.section_filters(&id).await?.iter().filter_map(view).collect())
}

#[tauri::command]
pub async fn first_characters(app: App<'_>, id: String, kind: Option<u32>) -> Res<Vec<Directory>> {
    Ok(app.server()?.section_first_characters(&id, kind).await?)
}

#[derive(Serialize)]
pub struct FolderView {
    title: Option<String>,
    directories: Vec<Directory>,
    items: Vec<Metadata>,
}

#[tauri::command]
pub async fn section_folders(app: App<'_>, id: String, key: Option<String>) -> Res<FolderView> {
    let mc = app.server()?.section_folders(&id, key.as_deref()).await?;
    Ok(FolderView { title: mc.title2.or(mc.title1), directories: mc.directory, items: mc.metadata })
}

/// Extra hubs for a detail page, plus sonically similar music.
#[tauri::command]
pub async fn item_hubs(app: App<'_>, rating_key: String, kind: String) -> Res<Vec<Hub>> {
    let server = app.server()?;
    let mut hubs = server.metadata_hubs(&rating_key, 12).await.unwrap_or_default();
    let similar = match kind.as_str() {
        "track" | "album" => server.nearest(&rating_key).await.ok().map(|m| ("Sonically similar", m)),
        "artist" => server.similar(&rating_key).await.ok().map(|m| ("Similar artists", m)),
        _ => None,
    };
    if let Some((title, metadata)) = similar.filter(|(_, m)| !m.is_empty()) {
        hubs.push(Hub {
            title: Some(title.into()),
            hub_identifier: Some(format!("patina.{kind}.similar")),
            metadata: metadata.into_iter().filter(|m| m.rating_key.as_deref() != Some(&rating_key)).collect(),
            ..Default::default()
        });
    }
    Ok(hubs)
}

// Item actions ---------------------------------------------------------------

#[tauri::command]
pub async fn remove_from_continue_watching(app: App<'_>, rating_key: String) -> Res {
    Ok(app.server()?.remove_from_continue_watching(&rating_key).await?)
}

/// Forget the resume position without changing the watched state.
#[tauri::command]
pub async fn clear_progress(app: App<'_>, rating_key: String) -> Res {
    Ok(app.server()?.set_progress(&rating_key, 0, "stopped").await?)
}

#[tauri::command]
pub async fn delete_item(app: App<'_>, rating_key: String) -> Res {
    Ok(app.server()?.delete_metadata(&rating_key).await?)
}

#[derive(serde::Deserialize)]
pub struct TagEdit {
    /// `genre`, `label`, `collection`, `mood`, ...
    tag: String,
    add: Vec<String>,
    remove: Vec<String>,
}

/// Edit fields (locking them so refreshes keep the change) and tags.
#[tauri::command]
pub async fn edit_metadata(app: App<'_>, rating_key: String, fields: Vec<(String, String)>, tags: Vec<TagEdit>) -> Res {
    let server = app.server()?;
    let item = server.metadata(&rating_key, MetadataOptions::default()).await?.ok_or_else(|| anyhow::anyhow!("Item not found."))?;
    let kind = MediaType::from_name(item.kind()).map(MediaType::id).ok_or_else(|| anyhow::anyhow!("This item cannot be edited."))?;
    if !fields.is_empty() {
        let pairs: Vec<(&str, &str)> = fields.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        server.edit_metadata(&rating_key, item.library_section_id, kind, &pairs, true).await?;
    }
    for t in tags.iter().filter(|t| !t.add.is_empty() || !t.remove.is_empty()) {
        let add: Vec<&str> = t.add.iter().map(String::as_str).collect();
        let remove: Vec<&str> = t.remove.iter().map(String::as_str).collect();
        server.edit_tags(&rating_key, kind, &t.tag, &add, &remove).await?;
    }
    Ok(())
}

/// Poster or background choices (`kind` is `poster` or `art`).
#[tauri::command]
pub async fn artwork(app: App<'_>, rating_key: String, kind: String) -> Res<Vec<Metadata>> {
    let server = app.server()?;
    Ok(if kind == "art" { server.arts(&rating_key).await? } else { server.posters(&rating_key).await? })
}

#[tauri::command]
pub async fn set_artwork(app: App<'_>, rating_key: String, kind: String, url: String) -> Res {
    let server = app.server()?;
    if kind == "art" { server.set_art(&rating_key, &url).await? } else { server.set_poster(&rating_key, &url).await? }
    Ok(())
}

#[tauri::command]
pub async fn matches(app: App<'_>, rating_key: String, title: Option<String>, year: Option<i64>) -> Res<Vec<MatchResult>> {
    let title = title.filter(|t| !t.trim().is_empty());
    Ok(app.server()?.matches(&rating_key, title.as_deref(), year, None).await?)
}

#[tauri::command]
pub async fn apply_match(app: App<'_>, rating_key: String, guid: String, name: Option<String>) -> Res {
    Ok(app.server()?.apply_match(&rating_key, &guid, name.as_deref()).await?)
}

#[tauri::command]
pub async fn unmatch(app: App<'_>, rating_key: String) -> Res {
    Ok(app.server()?.unmatch(&rating_key).await?)
}

#[tauri::command]
pub async fn search_subtitles(app: App<'_>, rating_key: String, language: String, hearing_impaired: bool, forced: bool) -> Res<Vec<Stream>> {
    Ok(app.server()?.search_subtitles(&rating_key, &language, hearing_impaired, forced).await?)
}

#[tauri::command]
pub async fn download_subtitle(app: App<'_>, rating_key: String, key: String) -> Res {
    Ok(app.server()?.download_subtitle(&rating_key, &key).await?)
}

#[tauri::command]
pub async fn delete_stream(app: App<'_>, id: i64) -> Res {
    Ok(app.server()?.delete_stream(id).await?)
}

// Collections ------------------------------------------------------------------

/// Add an item to a collection, or to a new one called `title`.
#[tauri::command]
pub async fn add_to_collection(app: App<'_>, rating_key: String, collection: Option<String>, title: Option<String>) -> Res {
    let server = app.server()?;
    let uri = server.item_uri(&[&rating_key]);
    match (collection, title) {
        (Some(id), _) => server.add_to_collection(&id, &uri).await?,
        (None, Some(title)) => {
            let item =
                server.metadata(&rating_key, MetadataOptions::default()).await?.ok_or_else(|| anyhow::anyhow!("Item not found."))?;
            let section = item.library_section_id.ok_or_else(|| anyhow::anyhow!("The item is not in a library."))?;
            let kind = MediaType::from_name(item.kind()).map(MediaType::id).unwrap_or(1);
            server.create_collection(&section.to_string(), &title, kind, Some(&uri), false).await?;
        }
        (None, None) => return Err(anyhow::anyhow!("No collection given.").into()),
    }
    Ok(())
}

#[tauri::command]
pub async fn remove_from_collection(app: App<'_>, id: String, rating_key: String) -> Res {
    Ok(app.server()?.remove_from_collection(&id, &rating_key).await?)
}

#[tauri::command]
pub async fn move_collection_item(app: App<'_>, id: String, rating_key: String, after: Option<String>) -> Res {
    Ok(app.server()?.move_collection_item(&id, &rating_key, after.as_deref()).await?)
}

#[tauri::command]
pub async fn delete_collection(app: App<'_>, id: String) -> Res {
    Ok(app.server()?.delete_collection(&id).await?)
}

/// Collection display settings: `mode` (-1..2) and/or `sort` (0..2).
#[tauri::command]
pub async fn collection_settings(app: App<'_>, id: String, mode: Option<i32>, sort: Option<i32>) -> Res {
    let server = app.server()?;
    if let Some(m) = mode {
        server.set_collection_mode(&id, m).await?;
    }
    if let Some(s) = sort {
        server.set_collection_sort(&id, s).await?;
    }
    Ok(())
}

// Playlists ------------------------------------------------------------------

#[tauri::command]
pub async fn move_playlist_item(app: App<'_>, id: String, item_id: i64, after: Option<i64>) -> Res {
    Ok(app.server()?.move_playlist_item(&id, item_id, after).await?)
}

#[tauri::command]
pub async fn update_playlist(app: App<'_>, id: String, title: String, summary: String) -> Res {
    Ok(app.server()?.update_playlist(&id, Some(title.trim()), Some(summary.trim())).await?)
}

#[tauri::command]
pub async fn clear_playlist(app: App<'_>, id: String) -> Res {
    Ok(app.server()?.clear_playlist(&id).await?)
}

// Downloads ------------------------------------------------------------------

#[tauri::command]
pub fn downloads() -> Vec<crate::downloads::Download> {
    crate::downloads::list()
}

#[tauri::command]
pub async fn download(app: App<'_>, rating_key: String) -> Res<usize> {
    Ok(crate::downloads::enqueue(&app, &rating_key).await?)
}

#[tauri::command]
pub fn remove_download(app: App, id: String) {
    crate::downloads::remove(&app, &id);
}

#[tauri::command]
pub fn retry_download(app: App, id: String) {
    crate::downloads::retry(&app, &id);
}

#[tauri::command]
pub fn play_downloads(app: App, ids: Vec<String>) -> Res {
    let files = crate::downloads::files(&ids);
    if files.is_empty() {
        return Err(anyhow::anyhow!("The downloaded file is missing.").into());
    }
    player::play_files(app.inner().clone(), files);
    Ok(())
}

#[tauri::command]
pub fn open_downloads_folder(app: App) -> Res {
    let dir = app.settings().player.downloads_dir();
    std::fs::create_dir_all(&dir)?;
    open::that(dir)?;
    Ok(())
}

// Live TV ----------------------------------------------------------------------

#[tauri::command]
pub async fn livetv_available(app: App<'_>) -> Res<bool> {
    Ok(crate::livetv::available(&app.server()?).await)
}

#[tauri::command]
pub async fn livetv_guide(app: App<'_>, hours: i64) -> Res<Vec<crate::livetv::Channel>> {
    Ok(crate::livetv::guide(&app.server()?, hours.clamp(1, 24)).await?)
}

#[tauri::command]
pub async fn livetv_watch(app: App<'_>, dvr: String, channel: String, title: String) -> Res {
    let mut item = crate::livetv::tune(&app.server()?, &dvr, &channel).await?;
    if item.title.is_none() {
        item.title = Some(title);
    }
    player::play(app.inner().clone(), PlayRequest { item, live: true, resume: false, ..Default::default() });
    Ok(())
}

#[tauri::command]
pub async fn recordings(app: App<'_>) -> Res<crate::livetv::Recordings> {
    Ok(crate::livetv::recordings(&app.server()?).await?)
}

#[tauri::command]
pub async fn record(app: App<'_>, guid: String) -> Res {
    Ok(crate::livetv::record(&app.server()?, &guid).await?)
}

#[tauri::command]
pub async fn delete_recording_rule(app: App<'_>, id: String) -> Res {
    let id = id.rsplit('/').next().unwrap_or(&id).to_string();
    Ok(app.server()?.delete_subscription(&id).await?)
}

// Server administration: libraries, stats, diagnostics -------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Choices {
    agents: Vec<(String, String)>,
    scanners: Vec<String>,
}

/// Agents and scanners for a new library of `kind` (`movie`, `show`, `artist`, `photo`).
#[tauri::command]
pub async fn library_choices(app: App<'_>, kind: String) -> Res<Choices> {
    let server = app.server()?;
    let id = MediaType::from_name(&kind).map(MediaType::id).unwrap_or(1);
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

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryForm {
    name: String,
    kind: String,
    agent: String,
    scanner: String,
    language: String,
    locations: Vec<String>,
}

#[tauri::command]
pub async fn create_library(app: App<'_>, form: LibraryForm) -> Res {
    let mut p = Params::new()
        .set("name", form.name.trim())
        .set("type", &form.kind)
        .set("agent", &form.agent)
        .set("scanner", &form.scanner)
        .set("language", &form.language);
    for l in form.locations.iter().filter(|l| !l.trim().is_empty()) {
        p.push("location", l.trim());
    }
    app.server()?.create_section(&p).await?;
    crate::session::reload_sections(&app).await?;
    Ok(())
}

/// Rename a library and/or change its folders.
#[tauri::command]
pub async fn edit_library(app: App<'_>, id: String, name: String, locations: Vec<String>) -> Res {
    let section = app.sections().into_iter().find(|s| s.key.as_deref() == Some(id.as_str()));
    let mut p = Params::new()
        .set("name", name.trim())
        .opt("agent", section.as_ref().and_then(|s| s.agent.clone()))
        .opt("scanner", section.as_ref().and_then(|s| s.scanner.clone()))
        .opt("language", section.as_ref().and_then(|s| s.language.clone()));
    for l in locations.iter().filter(|l| !l.trim().is_empty()) {
        p.push("location", l.trim());
    }
    app.server()?.edit_section(&id, &p).await?;
    crate::session::reload_sections(&app).await?;
    Ok(())
}

#[tauri::command]
pub async fn delete_library(app: App<'_>, id: String) -> Res {
    app.server()?.delete_section(&id).await?;
    crate::session::reload_sections(&app).await?;
    Ok(())
}

#[tauri::command]
pub async fn cancel_scan(app: App<'_>, id: String) -> Res {
    Ok(app.server()?.cancel_refresh_section(&id).await?)
}

#[tauri::command]
pub async fn library_prefs(app: App<'_>, id: String) -> Res<Vec<Pref>> {
    let list = app.server()?.section_prefs(&id).await?;
    Ok(list.into_iter().filter(|s| !s.hidden).map(pref_view).collect())
}

#[tauri::command]
pub async fn set_library_pref(app: App<'_>, id: String, pref: String, value: String) -> Res {
    Ok(app.server()?.set_section_prefs(&id, &Params::new().set(pref, value)).await?)
}

#[tauri::command]
pub async fn butler_all(app: App<'_>, start: bool) -> Res {
    let server = app.server()?;
    if start { server.start_all_butler_tasks().await? } else { server.stop_all_butler_tasks().await? }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bucket {
    at: i64,
    lan: i64,
    wan: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    name: String,
    bytes: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    /// Bytes streamed per time bucket, oldest first.
    bandwidth: Vec<Bucket>,
    /// Bytes streamed per account, most first.
    accounts: Vec<Usage>,
    /// Bytes streamed per device, most first.
    devices: Vec<Usage>,
    plays: i64,
    play_seconds: i64,
}

/// Bandwidth and plays since `since` (unix seconds). `timespan` is Plex's
/// bucket size code: 6 for hours, 4 for days.
#[tauri::command]
pub async fn server_stats(app: App<'_>, timespan: u32, since: i64) -> Res<Stats> {
    let server = app.server()?;
    let (bandwidth, media) = tokio::join!(server.statistics_bandwidth(timespan), server.statistics_media(timespan));
    let bandwidth = bandwidth?;
    let names = |mc: &MediaContainer| -> (HashMap<i64, String>, HashMap<i64, String>) {
        let accounts = mc.account.iter().filter_map(|a| Some((a.id?, a.name.clone()?))).collect();
        let devices = mc.device.iter().filter_map(|d| Some((d.id?, d.name.clone()?))).collect();
        (accounts, devices)
    };
    let (account_names, device_names) = names(&bandwidth);
    let mut buckets: std::collections::BTreeMap<i64, Bucket> = Default::default();
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

/// Save the server's logs (`logs`) or database (`database`) to the downloads
/// folder and reveal it. Returns the file path.
#[tauri::command]
pub async fn save_diagnostics(app: App<'_>, what: String) -> Res<String> {
    let server = app.server()?;
    let bytes = match what.as_str() {
        "logs" => server.download_logs().await?,
        "database" => server.download_database().await?,
        other => return Err(anyhow::anyhow!("Unknown diagnostics {other}.").into()),
    };
    let dir = directories::UserDirs::new()
        .and_then(|d| d.download_dir().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| app.settings().player.downloads_dir());
    std::fs::create_dir_all(&dir)?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let path = dir.join(format!("plex-{what}-{stamp}.zip"));
    std::fs::write(&path, bytes)?;
    let _ = open::that(&dir);
    Ok(path.display().to_string())
}
