//! Commands by name, with the same names, arguments and results as the Tauri
//! app's `crates/app/src/commands.rs`. Keep the two in step.

use anyhow::{Context, Result, anyhow, bail};
use patina_core::AppState;
use patina_core::config::{self, PlayerSettings, UiSettings};
use patina_core::ops::{edit, library, lists, playback, server, setup};
use patina_core::{downloads, livetv, player, session, tracks};
use patina_plex::{LibraryQuery, MetadataOptions, Params};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

/// A command's arguments: a JSON object with camelCase keys.
struct Args(Map<String, Value>);

impl Args {
    /// A required or `Option` argument; a missing key reads as null.
    fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T> {
        let v = self.0.get(key).cloned().unwrap_or(Value::Null);
        serde_json::from_value(v).with_context(|| format!("invalid argument `{key}`"))
    }

    fn s(&self, key: &str) -> Result<String> {
        self.get(key)
    }
}

fn to<T: Serialize>(v: T) -> Result<Value> {
    Ok(serde_json::to_value(v)?)
}

const NONE: Result<Value> = Ok(Value::Null);

#[derive(Serialize)]
struct PublicSettings {
    player: PlayerSettings,
    ui: UiSettings,
}

pub async fn call(app: &AppState, name: &str, args: Value) -> Result<Value> {
    let a = Args(match args {
        Value::Object(m) => m,
        Value::Null => Map::new(),
        _ => bail!("arguments must be an object"),
    });
    match name {
        // App
        "snapshot" => to(app.snapshot()),
        "info" => to(setup::info()),
        "settings" => {
            let s = app.settings();
            to(PublicSettings { player: s.player, ui: s.ui })
        }
        "set_player_settings" => {
            let patch: Value = a.get("player")?;
            app.try_update_settings(|s| config::patch(&mut s.player, patch))?;
            NONE
        }
        "set_ui_settings" => {
            let patch: Value = a.get("ui")?;
            app.try_update_settings(|s| config::patch(&mut s.ui, patch))?;
            NONE
        }
        "probe_mpv" => to(setup::probe_mpv(app).await?),
        "open_url" => {
            open::that(a.s("url")?)?;
            NONE
        }
        "open_mpv_config" => {
            let dir = config::mpv_config_dir();
            std::fs::create_dir_all(&dir)?;
            open::that(dir)?;
            NONE
        }

        // Session
        "retry" => {
            session::retry(app).await;
            NONE
        }
        "start_pin" => {
            let strong: bool = a.get("strong")?;
            let pin = session::create_pin(app, strong).await?;
            if strong {
                let _ = open::that(&pin.url);
            }
            Ok(json!({ "id": pin.id, "code": pin.code, "url": pin.url, "attempt": pin.attempt }))
        }
        "complete_pin" => to(session::complete_pin(app, a.get("id")?, a.get("attempt")?).await?),
        "cancel_pin" => {
            session::cancel_pin(app);
            NONE
        }
        "password_sign_in" => {
            let code: Option<String> = a.get("code")?;
            to(session::password_sign_in(app, &a.s("login")?, &a.s("password")?, code.as_deref()).await?)
        }
        "sign_in_manual" => to(session::sign_in_manual(app, &a.s("url")?, &a.s("token")?).await?),
        "pick_server" => {
            session::run(app, session::pick_server(app, &a.s("id")?)).await;
            NONE
        }
        "switch_server" => to(session::switch_server(app, &a.s("id")?).await?),
        "switch_user" => {
            let pin: Option<String> = a.get("pin")?;
            to(session::switch_user(app, &a.s("uuid")?, pin.as_deref()).await?)
        }
        "sign_out" => {
            session::sign_out(app);
            NONE
        }
        "reload_sections" => to(session::reload_sections(app).await?),
        "home_users" => match app.tv() {
            Some(tv) => to(tv.home_users().await?),
            None => Ok(json!([])),
        },

        // Browsing
        "home_hubs" => to(app.server()?.home_hubs(20).await?),
        "section_hubs" => to(app.server()?.section_hubs(&a.s("id")?, 18).await?),
        "section_sorts" => to(app.server()?.section_sorts(&a.s("id")?).await?),
        "section_filter_values" => {
            to(app.server()?.section_filter_values(&a.s("id")?, &a.s("filter")?, a.get("kind")?).await?)
        }
        "section_items" => {
            let filters: Vec<(String, String)> = a.get("filters")?;
            let q = LibraryQuery {
                kind: a.get("kind")?,
                sort: a.get("sort")?,
                unwatched: a.get("unwatched")?,
                filters: filters.into_iter().collect(),
                start: a.get("start")?,
                size: a.get("size")?,
            };
            to(app.server()?.section_items(&a.s("id")?, &q).await?)
        }
        "collections" => to(app.server()?.collections(&a.s("id")?).await?),
        "listing" => {
            let start: u32 = a.get("start")?;
            let size: u32 = a.get("size")?;
            let p = Params::new().set("X-Plex-Container-Start", start).set("X-Plex-Container-Size", size);
            to(app.server()?.fetch(&a.s("path")?, &p).await?)
        }
        "search" => to(app.server()?.search_hubs(&a.s("query")?, 24, None).await?),
        "metadata" => to(app.server()?.metadata(&a.s("ratingKey")?, MetadataOptions::full()).await?),
        "children" => {
            let server = app.server()?;
            let key = a.s("ratingKey")?;
            let collection: bool = a.get::<Option<bool>>("collection")?.unwrap_or(false);
            let mc = if collection { server.collection_items(&key).await? } else { server.children(&key).await? };
            to(mc.metadata)
        }
        "find_by_guid" => {
            let found = app.server()?.find_by_guid(&a.s("guid")?).await?;
            to(found.into_iter().next().and_then(|m| m.rating_key))
        }

        // Item actions
        "set_watched" => {
            let server = app.server()?;
            let key = a.s("ratingKey")?;
            if a.get("watched")? { server.scrobble(&key).await? } else { server.unscrobble(&key).await? }
            NONE
        }
        "rate" => to(app.server()?.rate(&a.s("ratingKey")?, a.get("rating")?).await?),
        "select_streams" => {
            to(app.server()?.select_streams(a.get("partId")?, a.get("audio")?, a.get("subtitle")?, true).await?)
        }
        "refresh_metadata" => to(app.server()?.refresh_metadata(&a.s("ratingKey")?, true).await?),
        "analyze_metadata" => to(app.server()?.analyze_metadata(&a.s("ratingKey")?).await?),
        "remove_from_continue_watching" => to(app.server()?.remove_from_continue_watching(&a.s("ratingKey")?).await?),
        "clear_progress" => to(app.server()?.set_progress(&a.s("ratingKey")?, 0, "stopped").await?),
        "delete_item" => to(app.server()?.delete_metadata(&a.s("ratingKey")?).await?),
        "edit_metadata" => {
            let fields: Vec<(String, String)> = a.get("fields")?;
            let tags: Vec<edit::TagEdit> = a.get::<Option<_>>("tags")?.unwrap_or_default();
            to(edit::edit_metadata(app, &a.s("ratingKey")?, &fields, &tags).await?)
        }
        "artwork" => {
            let server = app.server()?;
            let key = a.s("ratingKey")?;
            to(if a.s("kind")? == "art" { server.arts(&key).await? } else { server.posters(&key).await? })
        }
        "set_artwork" => {
            let server = app.server()?;
            let (key, url) = (a.s("ratingKey")?, a.s("url")?);
            if a.s("kind")? == "art" { server.set_art(&key, &url).await? } else { server.set_poster(&key, &url).await? }
            NONE
        }
        "matches" => {
            let title: Option<String> = a.get::<Option<String>>("title")?.filter(|t| !t.trim().is_empty());
            to(app.server()?.matches(&a.s("ratingKey")?, title.as_deref(), a.get("year")?, None).await?)
        }
        "apply_match" => {
            let name: Option<String> = a.get("name")?;
            to(app.server()?.apply_match(&a.s("ratingKey")?, &a.s("guid")?, name.as_deref()).await?)
        }
        "unmatch" => to(app.server()?.unmatch(&a.s("ratingKey")?).await?),
        "search_subtitles" => {
            let hi: bool = a.get::<Option<bool>>("hearingImpaired")?.unwrap_or(false);
            let forced: bool = a.get::<Option<bool>>("forced")?.unwrap_or(false);
            to(app.server()?.search_subtitles(&a.s("ratingKey")?, &a.s("language")?, hi, forced).await?)
        }
        "download_subtitle" => to(app.server()?.download_subtitle(&a.s("ratingKey")?, &a.s("key")?).await?),
        "delete_stream" => to(app.server()?.delete_stream(a.get("id")?).await?),

        // Collections
        "add_to_collection" => {
            let collection: Option<String> = a.get("collection")?;
            let title: Option<String> = a.get("title")?;
            to(edit::add_to_collection(app, &a.s("ratingKey")?, collection.as_deref(), title.as_deref()).await?)
        }
        "remove_from_collection" => to(app.server()?.remove_from_collection(&a.s("id")?, &a.s("ratingKey")?).await?),
        "move_collection_item" => {
            let after: Option<String> = a.get("after")?;
            to(app.server()?.move_collection_item(&a.s("id")?, &a.s("ratingKey")?, after.as_deref()).await?)
        }
        "delete_collection" => to(app.server()?.delete_collection(&a.s("id")?).await?),
        "collection_settings" => {
            let server = app.server()?;
            let id = a.s("id")?;
            if let Some(m) = a.get::<Option<i32>>("mode")? {
                server.set_collection_mode(&id, m).await?;
            }
            if let Some(s) = a.get::<Option<i32>>("sort")? {
                server.set_collection_sort(&id, s).await?;
            }
            NONE
        }

        // Watchlist
        "watchlist" => to(lists::watchlist(app).await?),
        "watchlisted" => to(lists::watchlisted(app, &a.s("discoverId")?).await?),
        "set_watchlisted" => {
            let tv = app.plex_tv()?;
            let id = a.s("discoverId")?;
            if a.get("on")? { tv.add_to_watchlist(&id).await? } else { tv.remove_from_watchlist(&id).await? }
            NONE
        }

        // Playlists
        "playlists" => {
            let kind: Option<String> = a.get("kind")?;
            let smart = kind.is_some().then_some(false);
            to(app.server()?.playlists(kind.as_deref(), smart).await?)
        }
        "playlist" => to(lists::playlist(app, &a.s("id")?).await?),
        "add_to_playlist" => {
            let playlist: Option<String> = a.get("playlist")?;
            let title: Option<String> = a.get("title")?;
            to(lists::add_to_playlist(app, &a.s("ratingKey")?, playlist.as_deref(), title.as_deref(), &a.s("kind")?).await?)
        }
        "remove_from_playlist" => to(app.server()?.remove_from_playlist(&a.s("id")?, a.get("itemId")?).await?),
        "delete_playlist" => to(app.server()?.delete_playlist(&a.s("id")?).await?),
        "move_playlist_item" => to(app.server()?.move_playlist_item(&a.s("id")?, a.get("itemId")?, a.get("after")?).await?),
        "update_playlist" => {
            let (title, summary) = (a.s("title")?, a.s("summary")?);
            to(app.server()?.update_playlist(&a.s("id")?, Some(title.trim()), Some(summary.trim())).await?)
        }
        "clear_playlist" => to(app.server()?.clear_playlist(&a.s("id")?).await?),

        // Playback
        "play" => {
            player::play(app.clone(), a.get("request")?);
            NONE
        }
        "control" => {
            player::control(a.get("command")?);
            NONE
        }
        "queue" => {
            player::queue(app.clone(), a.get("command")?);
            NONE
        }
        "queue_items" => to(player::queue_items(app.clone(), a.get("items")?)?),
        "lyrics" => to(patina_core::lyrics::load(&app.server()?, &a.s("ratingKey")?).await?),

        // Bulk audio & subtitles. Progress arrives as `tracks-progress` events: [done, total].
        "tracks_load" => to(tracks::load(app, &a.s("show")?).await?),
        "tracks_view" => to(tracks::view(app, &a.get("query")?)?),
        "tracks_apply" => {
            tracks::apply(app, a.get("changes")?, |done, total| crate::emit("tracks-progress", &json!([done, total]))).await?;
            NONE
        }

        // Activity
        "sessions" => to(app.server()?.sessions().await?),
        "terminate_session" => to(app.server()?.terminate_session(&a.s("id")?, "Stopped by the server owner.").await?),
        "history" => to(lists::history(app, a.get("page")?).await?),
        "delete_history" => to(lists::delete_history(app, &a.s("key")?).await?),
        "tasks" => to(server::tasks(app).await?),
        "cancel_activity" => to(app.server()?.cancel_activity(&a.s("uuid")?).await?),
        "kill_transcode" => to(server::kill_transcode(app, &a.s("key")?).await?),

        // Server administration
        "server_overview" => to(server::server_overview(app).await?),
        "check_for_updates" => to(app.server()?.check_for_updates(true).await?),
        "apply_update" => to(app.server()?.apply_update(a.get("tonight")?, false).await?),
        "section_action" => {
            let server = app.server()?;
            let id = a.s("id")?;
            match a.s("action")?.as_str() {
                "scan" => server.refresh_section(&id, false, None).await?,
                "refresh" => server.refresh_section(&id, true, None).await?,
                "analyze" => server.analyze_section(&id).await?,
                "emptyTrash" => server.empty_trash(&id).await?,
                other => bail!("Unknown library action {other}."),
            }
            NONE
        }
        "server_action" => {
            let server = app.server()?;
            match a.s("action")?.as_str() {
                "scanAll" => server.refresh_all_sections(false).await?,
                "cleanBundles" => server.clean_bundles().await?,
                "optimizeDatabase" => server.optimize_database().await?,
                other => bail!("Unknown server action {other}."),
            }
            NONE
        }
        "butler_tasks" => to(app.server()?.butler_tasks().await?),
        "butler_task" => {
            let server = app.server()?;
            let name = a.s("name")?;
            if a.get("start")? { server.start_butler_task(&name).await? } else { server.stop_butler_task(&name).await? }
            NONE
        }
        "butler_all" => {
            let server = app.server()?;
            if a.get("start")? { server.start_all_butler_tasks().await? } else { server.stop_all_butler_tasks().await? }
            NONE
        }
        "prefs" => to(server::prefs(app).await?),
        "set_pref" => to(app.server()?.set_pref(&a.s("id")?, &a.s("value")?).await?),
        "users_devices" => to(server::users_devices(app).await?),
        "library_choices" => to(server::library_choices(app, &a.s("kind")?).await?),
        "create_library" => to(server::create_library(app, &a.get("form")?).await?),
        "edit_library" => {
            let locations: Vec<String> = a.get("locations")?;
            to(server::edit_library(app, &a.s("id")?, &a.s("name")?, &locations).await?)
        }
        "delete_library" => to(server::delete_library(app, &a.s("id")?).await?),
        "cancel_scan" => to(app.server()?.cancel_refresh_section(&a.s("id")?).await?),
        "library_prefs" => to(server::library_prefs(app, &a.s("id")?).await?),
        "set_library_pref" => {
            to(app.server()?.set_section_prefs(&a.s("id")?, &Params::new().set(a.s("pref")?, a.s("value")?)).await?)
        }
        "server_stats" => to(server::server_stats(app, a.get("timespan")?, a.get("since")?).await?),
        "save_diagnostics" => {
            let path = server::save_diagnostics(app, &a.s("what")?).await?;
            if let Some(dir) = path.parent() {
                let _ = open::that(dir);
            }
            to(path.display().to_string())
        }

        // Library browsing
        "section_filters" => to(library::section_filters(app, &a.s("id")?, a.get("kind")?).await?),
        "first_characters" => to(app.server()?.section_first_characters(&a.s("id")?, a.get("kind")?).await?),
        "section_folders" => {
            let key: Option<String> = a.get("key")?;
            to(library::section_folders(app, &a.s("id")?, key.as_deref()).await?)
        }
        "item_hubs" => to(library::item_hubs(app, &a.s("ratingKey")?, &a.s("kind")?).await?),

        // Downloads
        "downloads" => to(downloads::list()),
        "download" => to(downloads::enqueue(app, &a.s("ratingKey")?).await?),
        "remove_download" => {
            downloads::remove(app, &a.s("id")?);
            NONE
        }
        "pause_download" => {
            downloads::pause(app, &a.s("id")?);
            NONE
        }
        "retry_download" => {
            downloads::retry(app, &a.s("id")?);
            NONE
        }
        "play_downloads" => {
            let ids: Vec<String> = a.get("ids")?;
            to(playback::play_downloads(app, &ids)?)
        }
        "open_downloads_folder" => {
            let dir = app.settings().player.downloads_dir();
            std::fs::create_dir_all(&dir)?;
            open::that(dir)?;
            NONE
        }

        // Live TV
        "livetv_available" => to(livetv::available(&app.server()?).await),
        "livetv_guide" => {
            let hours: i64 = a.get("hours")?;
            to(livetv::guide(&app.server()?, hours.clamp(1, 24)).await?)
        }
        "livetv_watch" => to(playback::livetv_watch(app, &a.s("dvr")?, &a.s("channel")?, &a.s("title")?).await?),
        "recordings" => to(livetv::recordings(&app.server()?).await?),
        "record" => to(livetv::record(&app.server()?, &a.s("guid")?).await?),
        "delete_recording_rule" => to(livetv::delete_rule(&app.server()?, &a.s("id")?).await?),

        other => Err(anyhow!("Unknown command {other}.")),
    }
}
