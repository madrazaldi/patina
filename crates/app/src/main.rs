//! Patina — a Plex client that plays through mpv.

// Keeps a console window from opening alongside the app on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;
mod images;
mod player;
mod session;
mod state;
mod tracks;

use tauri::Manager;

use crate::state::AppState;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("PATINA_LOG")
                .unwrap_or_else(|_| "warn,patina=info,patina_plex=info,mpv=warn".into()),
        )
        .init();

    std::thread::spawn(images::prune_cache);

    tauri::Builder::default()
        .register_asynchronous_uri_scheme_protocol(images::SCHEME, |_ctx, request, responder| {
            images::handle(request, responder)
        })
        .setup(|app| {
            let state = AppState::new(app.handle().clone());
            app.manage(state.clone());
            tauri::async_runtime::spawn(session::restore(state));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::snapshot,
            commands::info,
            commands::settings,
            commands::set_player_settings,
            commands::set_ui_settings,
            commands::probe_mpv,
            commands::open_url,
            commands::open_mpv_config,
            commands::retry,
            commands::start_pin,
            commands::complete_pin,
            commands::cancel_pin,
            commands::password_sign_in,
            commands::sign_in_manual,
            commands::pick_server,
            commands::switch_server,
            commands::switch_user,
            commands::sign_out,
            commands::reload_sections,
            commands::home_users,
            commands::home_hubs,
            commands::section_hubs,
            commands::section_sorts,
            commands::section_filter_values,
            commands::section_items,
            commands::collections,
            commands::listing,
            commands::search,
            commands::metadata,
            commands::children,
            commands::find_by_guid,
            commands::set_watched,
            commands::rate,
            commands::select_streams,
            commands::refresh_metadata,
            commands::analyze_metadata,
            commands::watchlist,
            commands::watchlisted,
            commands::set_watchlisted,
            commands::playlists,
            commands::playlist,
            commands::add_to_playlist,
            commands::remove_from_playlist,
            commands::delete_playlist,
            commands::play,
            commands::control,
            commands::tracks_load,
            commands::tracks_view,
            commands::tracks_apply,
            commands::sessions,
            commands::terminate_session,
            commands::history,
            commands::delete_history,
            commands::tasks,
            commands::cancel_activity,
            commands::kill_transcode,
            commands::server_overview,
            commands::check_for_updates,
            commands::apply_update,
            commands::section_action,
            commands::server_action,
            commands::butler_tasks,
            commands::butler_task,
            commands::prefs,
            commands::set_pref,
            commands::users_devices,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Patina")
        .run(|_, event| {
            if let tauri::RunEvent::Exit = event {
                player::shutdown();
            }
        });
}
