//! Tauri commands for automatic playback preferences and smart downloads.
//! Both are scoped by account and server inside the core.

use patina_core::state::AppState;
use patina_core::{prefs, smart, tracks};
use patina_plex::Metadata;
use tauri::State;

use crate::commands::{Error, Res};

type App<'a> = State<'a, AppState>;

// Playback preferences ---------------------------------------------------------

/// The profile's rules, and a show's when `show` is given, with what applies.
#[tauri::command]
pub fn playback_prefs(app: App, show: Option<String>) -> Res<prefs::View> {
    Ok(prefs::view(&app, show.as_deref())?)
}

/// Replace the rules of the profile, or of a show; empty rules clear them.
#[tauri::command]
pub fn set_playback_prefs(app: App, show: Option<String>, title: Option<String>, rules: prefs::Rules) -> Res {
    Ok(prefs::save(&app, show.as_deref(), title.as_deref(), rules)?)
}

/// Shows that have rules of their own.
#[tauri::command]
pub fn playback_prefs_shows(app: App) -> Res<Vec<prefs::ShowRules>> {
    Ok(prefs::shows(&app)?)
}

/// Index of the version of an item the rules prefer.
#[tauri::command]
pub fn preferred_version(app: App, item: Metadata) -> usize {
    prefs::preferred_version(&app, &item)
}

/// Remember the track dialog's choices as the show's rules.
#[tauri::command]
pub fn tracks_remember(app: App, query: tracks::Query) -> Res {
    Ok(prefs::remember_tracks(&app, &query)?)
}

// Smart downloads --------------------------------------------------------------

#[tauri::command]
pub fn smart_downloads(app: App) -> smart::Overview {
    smart::overview(&app)
}

/// Keep the next `keep` unwatched items of a show, season or playlist downloaded.
#[tauri::command]
pub async fn set_smart_download(app: App<'_>, rating_key: String, keep: u32, delete_watched: bool) -> Res<smart::Policy> {
    Ok(smart::set_policy(&app, &rating_key, keep, delete_watched).await?)
}

#[tauri::command]
pub fn update_smart_download(app: App, id: String, patch: smart::Patch) -> Res {
    Ok(smart::update_policy(&app, &id, patch)?)
}

#[tauri::command]
pub fn remove_smart_download(app: App, id: String, delete_downloads: bool) -> Res {
    Ok(smart::remove_policy(&app, &id, delete_downloads)?)
}

/// Limit all downloads together to this many bytes; none removes the limit.
#[tauri::command]
pub fn set_download_budget(app: App, bytes: Option<u64>) -> Res {
    Ok(smart::set_budget(&app, bytes)?)
}

/// Bring downloads in line with the policies now.
#[tauri::command]
pub async fn run_smart_downloads(app: App<'_>) -> Res<smart::Report> {
    smart::reconcile(&app).await.map_err(Error::from)
}
