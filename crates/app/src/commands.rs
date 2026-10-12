//! Tauri commands: the webview's whole API. Plex models are returned as they
//! serialize (Plex's own JSON field names); tokens never cross the boundary.

use patina_core::config::{self, PlayerSettings, UiSettings};
use patina_core::ops::{edit, library, lists, pick, playback, server, setup, xsearch};
use patina_core::controls::{PlaybackState, Tune};
use patina_core::failure::{self, PlaybackFailure, Recover};
use patina_core::player::{self, Control, PlayRequest, QueueCommand};
use patina_core::recovery::SavedQueueView;
use patina_core::state::{AppState, Snapshot};
use patina_core::{downloads, livetv, servers, session, tracks, undo};
use patina_plex::{Directory, Hub, LibraryQuery, MatchResult, MediaContainer, Metadata, MetadataOptions, Params, Stream};
use serde::{Serialize, Serializer};
use tauri::State;
use tauri::ipc::Channel;

/// Any failure, reported to the webview as its message.
pub struct Error(pub String);

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

pub type Res<T = ()> = Result<T, Error>;
type App<'a> = State<'a, AppState>;

// App ------------------------------------------------------------------------

#[tauri::command]
pub fn snapshot(app: App) -> Snapshot {
    app.snapshot()
}

#[tauri::command]
pub fn info() -> setup::Info {
    setup::info()
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
pub fn set_player_settings(app: App, player: serde_json::Value) -> Res {
    Ok(app.try_update_settings(|s| config::patch(&mut s.player, player))?)
}

#[tauri::command]
pub fn set_ui_settings(app: App, ui: serde_json::Value) -> Res {
    Ok(app.try_update_settings(|s| config::patch(&mut s.ui, ui))?)
}

#[tauri::command]
pub async fn probe_mpv(app: App<'_>) -> Res<String> {
    Ok(setup::probe_mpv(&app).await?)
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
    session::retry(&app).await;
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
    Ok(session::password_sign_in(&app, &login, &password, code.as_deref()).await?)
}

#[tauri::command]
pub async fn sign_in_manual(app: App<'_>, url: String, token: String) -> Res {
    Ok(session::sign_in_manual(&app, &url, &token).await?)
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

/// `server` is the machine id of another server of the account (see `search_all_servers`).
#[tauri::command]
pub async fn metadata(app: App<'_>, rating_key: String, server: Option<String>) -> Res<Option<Metadata>> {
    let found = servers::server_for(&app, server.as_deref()).await?.metadata(&rating_key, MetadataOptions::full()).await?;
    Ok(found.map(|mut m| {
        servers::tag(&app, std::slice::from_mut(&mut m), server.as_deref());
        m
    }))
}

/// Seasons, episodes, albums, tracks or collection items of an item.
#[tauri::command]
pub async fn children(app: App<'_>, rating_key: String, collection: bool, server: Option<String>) -> Res<Vec<Metadata>> {
    let on = servers::server_for(&app, server.as_deref()).await?;
    let mc = if collection { on.collection_items(&rating_key).await? } else { on.children(&rating_key).await? };
    let mut items = mc.metadata;
    servers::tag(&app, &mut items, server.as_deref());
    Ok(items)
}

/// Rating key of the local copy of a plex.tv item, if the server has one.
#[tauri::command]
pub async fn find_by_guid(app: App<'_>, guid: String) -> Res<Option<String>> {
    let found = app.server()?.find_by_guid(&guid).await?;
    Ok(found.into_iter().next().and_then(|m| m.rating_key))
}

// Item actions ---------------------------------------------------------------

/// Mark an item (or a whole show, season or album) watched or unwatched. Announces itself
/// with a toast that offers Undo (see `undo_action`).
#[tauri::command]
pub async fn set_watched(app: App<'_>, rating_key: String, watched: bool, server: Option<String>) -> Res {
    Ok(undo::set_watched(&app, server.as_deref(), &[rating_key], watched).await?)
}

/// [`set_watched`] for a selection; undone together.
#[tauri::command]
pub async fn set_watched_many(app: App<'_>, rating_keys: Vec<String>, watched: bool) -> Res {
    Ok(undo::set_watched(&app, None, &rating_keys, watched).await?)
}

#[tauri::command]
pub async fn rate(app: App<'_>, rating_key: String, rating: Option<f64>) -> Res {
    Ok(app.server()?.rate(&rating_key, rating).await?)
}

/// Choose the audio and/or subtitle stream of a part (0 turns subtitles off).
#[tauri::command]
/// `server` is the machine id of the server the item was found on (`None` for the connected one).
pub async fn select_streams(app: App<'_>, part_id: i64, audio: Option<i64>, subtitle: Option<i64>, server: Option<String>) -> Res {
    Ok(servers::server_for(&app, server.as_deref()).await?.select_streams(part_id, audio, subtitle, true).await?)
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

#[tauri::command]
pub async fn watchlist(app: App<'_>) -> Res<Vec<Metadata>> {
    Ok(lists::watchlist(&app).await?)
}

#[tauri::command]
pub async fn watchlisted(app: App<'_>, discover_id: String) -> Res<bool> {
    Ok(lists::watchlisted(&app, &discover_id).await?)
}

#[tauri::command]
pub async fn set_watchlisted(app: App<'_>, discover_id: String, on: bool) -> Res {
    let tv = app.plex_tv()?;
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

#[tauri::command]
pub async fn playlist(app: App<'_>, id: String) -> Res<Option<lists::PlaylistView>> {
    Ok(lists::playlist(&app, &id).await?)
}

/// Add an item to an existing playlist, or to a new one called `title`.
#[tauri::command]
pub async fn add_to_playlist(app: App<'_>, rating_key: String, playlist: Option<String>, title: Option<String>, kind: String) -> Res {
    Ok(lists::add_to_playlist(&app, &rating_key, playlist.as_deref(), title.as_deref(), &kind).await?)
}

#[tauri::command]
pub async fn remove_from_playlist(app: App<'_>, id: String, item_id: i64) -> Res {
    Ok(undo::remove_from_playlist(&app, &id, item_id).await?)
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

/// Volume, speed, chapters, repeat and the sleep timer.
#[tauri::command]
pub fn tune(app: App, command: Tune) {
    player::tune(&app, command);
}

/// The playing item's controls and diagnostics; also pushed as `playback-state`.
#[tauri::command]
pub fn playback_state() -> Option<PlaybackState> {
    patina_core::controls::playback_state()
}

/// The playback failure waiting for a decision; also pushed as `playback-failure`.
#[tauri::command]
pub fn playback_failure() -> Option<PlaybackFailure> {
    failure::current()
}

#[tauri::command]
pub fn recover_playback(app: App, action: Recover) -> Res {
    Ok(failure::recover(&app, action)?)
}

#[tauri::command]
pub fn dismiss_failure(app: App) {
    failure::clear(&app);
}

/// The queue saved by the last playback, to offer a Resume.
#[tauri::command]
pub fn saved_queue(app: App) -> Option<SavedQueueView> {
    playback::saved_queue(&app)
}

#[tauri::command]
pub fn resume_saved(app: App) -> Res {
    Ok(playback::resume_saved(&app)?)
}

#[tauri::command]
pub fn forget_saved(app: App) {
    playback::forget_saved(&app);
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

#[tauri::command]
pub async fn history(app: App<'_>, page: u32) -> Res<lists::HistoryPage> {
    Ok(lists::history(&app, page).await?)
}

/// Delete a history entry by its `historyKey`.
#[tauri::command]
pub async fn delete_history(app: App<'_>, key: String) -> Res {
    Ok(lists::delete_history(&app, &key).await?)
}

#[tauri::command]
pub async fn tasks(app: App<'_>) -> Res<server::Tasks> {
    Ok(server::tasks(&app).await?)
}

#[tauri::command]
pub async fn cancel_activity(app: App<'_>, uuid: String) -> Res {
    Ok(app.server()?.cancel_activity(&uuid).await?)
}

#[tauri::command]
pub async fn kill_transcode(app: App<'_>, key: String) -> Res {
    Ok(server::kill_transcode(&app, &key).await?)
}

// Server administration ------------------------------------------------------

#[tauri::command]
pub async fn server_overview(app: App<'_>) -> Res<server::Overview> {
    Ok(server::server_overview(&app).await?)
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
pub async fn maintenance(app: App<'_>) -> Res<server::Maintenance> {
    Ok(server::maintenance(&app).await?)
}

#[tauri::command]
pub async fn butler_task(app: App<'_>, name: String, start: bool) -> Res {
    let server = app.server()?;
    if start { server.start_butler_task(&name).await? } else { server.stop_butler_task(&name).await? }
    Ok(())
}

#[tauri::command]
pub async fn prefs(app: App<'_>) -> Res<Vec<server::PrefGroup>> {
    Ok(server::settings(&app).await?)
}

#[tauri::command]
pub async fn set_pref(app: App<'_>, id: String, value: String) -> Res {
    Ok(app.server()?.set_pref(&id, &value).await?)
}

#[tauri::command]
pub async fn users_devices(app: App<'_>) -> Res<server::UsersDevices> {
    Ok(server::users_devices(&app).await?)
}

// Live updates, queue, lyrics ----------------------------------------------

#[tauri::command]
pub fn queue(app: App, command: QueueCommand) {
    player::queue(app.inner().clone(), command);
}

#[tauri::command]
pub async fn lyrics(app: App<'_>, rating_key: String) -> Res<Option<patina_core::lyrics::Lyrics>> {
    Ok(patina_core::lyrics::load(&app.server()?, &rating_key).await?)
}

// Library browsing: filters, jump bar, folders, related ---------------------

/// Filters for a section and media type (genre, year, resolution, unwatched, ...).
#[tauri::command]
pub async fn section_filters(app: App<'_>, id: String, kind: Option<u32>) -> Res<Vec<library::FilterView>> {
    Ok(library::section_filters(&app, &id, kind).await?)
}

#[tauri::command]
pub async fn first_characters(app: App<'_>, id: String, kind: Option<u32>) -> Res<Vec<Directory>> {
    Ok(app.server()?.section_first_characters(&id, kind).await?)
}

#[tauri::command]
pub async fn section_folders(app: App<'_>, id: String, key: Option<String>) -> Res<library::FolderView> {
    Ok(library::section_folders(&app, &id, key.as_deref()).await?)
}

/// Extra hubs for a detail page, plus sonically similar music.
#[tauri::command]
pub async fn item_hubs(app: App<'_>, rating_key: String, kind: String, server: Option<String>) -> Res<Vec<Hub>> {
    if servers::is_current(&app, server.as_deref()) {
        return Ok(library::item_hubs(&app, &rating_key, &kind).await?);
    }
    let mut hubs = library::item_hubs_on(&servers::server_for(&app, server.as_deref()).await?, &rating_key, &kind).await?;
    for hub in &mut hubs {
        servers::tag(&app, &mut hub.metadata, server.as_deref());
    }
    Ok(hubs)
}

// Item actions ---------------------------------------------------------------

#[tauri::command]
pub async fn remove_from_continue_watching(app: App<'_>, rating_key: String) -> Res {
    Ok(undo::remove_from_continue_watching(&app, &rating_key).await?)
}

/// Forget the resume position without changing the watched state.
#[tauri::command]
pub async fn clear_progress(app: App<'_>, rating_key: String) -> Res {
    Ok(undo::clear_progress(&app, &rating_key).await?)
}

// Undo -----------------------------------------------------------------------

#[derive(Serialize)]
pub struct UndoView {
    id: u64,
    label: String,
}

/// What Cmd/Ctrl+Z would undo, if anything.
#[tauri::command]
pub fn undo_last(app: App) -> Option<UndoView> {
    undo::last(&app).map(|(id, label)| UndoView { id, label })
}

/// Undo entry `id` (a toast's Undo button), or the most recent one.
#[tauri::command]
pub async fn undo_action(app: App<'_>, id: Option<u64>) -> Res<String> {
    Ok(undo::undo(&app, id).await?)
}

// Pick something to watch ----------------------------------------------------

#[tauri::command]
pub async fn pick_options(app: App<'_>) -> Res<pick::PickOptions> {
    Ok(pick::options(&app).await?)
}

#[tauri::command]
pub async fn pick_something(app: App<'_>, query: pick::PickQuery) -> Res<pick::PickResult> {
    Ok(pick::pick(&app, &query).await?)
}

// Search across servers ------------------------------------------------------

/// Search every server of the account; `progress` gets the results so far after each server answers.
#[tauri::command]
pub async fn search_all_servers(app: App<'_>, query: String, progress: Channel<xsearch::Results>) -> Res<xsearch::Results> {
    Ok(xsearch::search_all(&app, &query, |r| {
        let _ = progress.send(r.clone());
    })
    .await)
}

#[tauri::command]
pub async fn delete_item(app: App<'_>, rating_key: String) -> Res {
    Ok(app.server()?.delete_metadata(&rating_key).await?)
}

/// Edit fields (locking them so refreshes keep the change) and tags.
#[tauri::command]
pub async fn edit_metadata(app: App<'_>, rating_key: String, fields: Vec<(String, String)>, tags: Vec<edit::TagEdit>) -> Res {
    Ok(edit::edit_metadata(&app, &rating_key, &fields, &tags).await?)
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
    Ok(edit::add_to_collection(&app, &rating_key, collection.as_deref(), title.as_deref()).await?)
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
pub fn downloads() -> Vec<downloads::Download> {
    downloads::list()
}

#[tauri::command]
pub async fn download(app: App<'_>, rating_key: String) -> Res<usize> {
    Ok(downloads::enqueue(&app, &rating_key).await?)
}

#[tauri::command]
pub fn remove_download(app: App, id: String) {
    downloads::remove(&app, &id);
}

#[tauri::command]
pub fn pause_download(app: App, id: String) { downloads::pause(&app, &id); }

#[tauri::command]
pub fn retry_download(app: App, id: String) {
    downloads::retry(&app, &id);
}

#[tauri::command]
pub fn play_downloads(app: App, ids: Vec<String>) -> Res {
    Ok(playback::play_downloads(&app, &ids)?)
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
    Ok(livetv::available(&app.server()?).await)
}

#[tauri::command]
pub async fn livetv_guide(app: App<'_>, hours: i64) -> Res<Vec<livetv::Channel>> {
    Ok(livetv::guide(&app.server()?, hours.clamp(1, 24)).await?)
}

#[tauri::command]
pub async fn livetv_watch(app: App<'_>, dvr: String, channel: String, title: String) -> Res {
    Ok(playback::livetv_watch(&app, &dvr, &channel, &title).await?)
}

#[tauri::command]
pub async fn recordings(app: App<'_>) -> Res<livetv::Recordings> {
    Ok(livetv::recordings(&app.server()?).await?)
}

#[tauri::command]
pub async fn record(app: App<'_>, guid: String) -> Res {
    Ok(livetv::record(&app.server()?, &guid).await?)
}

#[tauri::command]
pub async fn delete_recording_rule(app: App<'_>, id: String) -> Res {
    Ok(livetv::delete_rule(&app.server()?, &id).await?)
}

// Server administration: libraries, stats, diagnostics -------------------------

/// Agents and scanners for a new library of `kind` (`movie`, `show`, `artist`, `photo`).
#[tauri::command]
pub async fn library_choices(app: App<'_>, kind: String) -> Res<server::Choices> {
    Ok(server::library_choices(&app, &kind).await?)
}

#[tauri::command]
pub async fn create_library(app: App<'_>, form: server::LibraryForm) -> Res {
    Ok(server::create_library(&app, &form).await?)
}

/// Rename a library and/or change its folders.
#[tauri::command]
pub async fn edit_library(app: App<'_>, id: String, name: String, locations: Vec<String>) -> Res {
    Ok(server::edit_library(&app, &id, &name, &locations).await?)
}

#[tauri::command]
pub async fn delete_library(app: App<'_>, id: String) -> Res {
    Ok(server::delete_library(&app, &id).await?)
}

#[tauri::command]
pub async fn cancel_scan(app: App<'_>, id: String) -> Res {
    Ok(app.server()?.cancel_refresh_section(&id).await?)
}

#[tauri::command]
pub async fn library_prefs(app: App<'_>, id: String) -> Res<Vec<server::Pref>> {
    Ok(server::library_prefs(&app, &id).await?)
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

/// Bandwidth and plays since `since` (unix seconds). `timespan` is Plex's
/// bucket size code: 6 for hours, 4 for days.
#[tauri::command]
pub async fn server_stats(app: App<'_>, timespan: u32, since: i64) -> Res<server::Stats> {
    Ok(server::server_stats(&app, timespan, since).await?)
}

/// Save the server's logs (`logs`) or database (`database`) to the downloads
/// folder and reveal it. Returns the file path.
#[tauri::command]
pub async fn save_diagnostics(app: App<'_>, what: String) -> Res<String> {
    let path = server::save_diagnostics(&app, &what).await?;
    if let Some(dir) = path.parent() {
        let _ = open::that(dir);
    }
    Ok(path.display().to_string())
}

#[tauri::command]
pub fn queue_items(app: App, items: Vec<Metadata>) -> Res {
    Ok(player::queue_items(app.inner().clone(), items)?)
}
