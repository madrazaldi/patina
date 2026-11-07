//! The pages, one per [`Route`].

pub mod activity;
pub mod browse;
pub mod downloads;
pub mod home;
pub mod item;
pub mod library;
pub mod livetv;
pub mod pick;
pub mod playlist;
pub mod playlists;
pub mod requests;
pub mod search;
pub mod seerr_title;
pub mod server;
pub mod settings;
pub mod watchlist;

use crate::prelude::*;

use crate::nav::Route;

/// Build the view for a page.
pub fn build(route: &Route, window: &mut Window, cx: &mut App) -> AnyView {
    match route {
        Route::Home => cx.new(|cx| home::Home::new(window, cx)).into(),
        Route::Library { id } => cx.new(|cx| library::Library::new(id.clone(), window, cx)).into(),
        Route::Item { rating_key, server } => cx.new(|cx| item::Item::new(rating_key.clone(), server.clone(), window, cx)).into(),
        Route::Browse { path, title } => cx.new(|cx| browse::Browse::new(path.clone(), title.clone(), window, cx)).into(),
        Route::Search => cx.new(|cx| search::Search::new(window, cx)).into(),
        Route::Pick => cx.new(|cx| pick::Pick::new(window, cx)).into(),
        Route::Playlists => cx.new(|cx| playlists::Playlists::new(window, cx)).into(),
        Route::Playlist { id } => cx.new(|cx| playlist::Playlist::new(id.clone(), window, cx)).into(),
        Route::Watchlist => cx.new(|cx| watchlist::Watchlist::new(window, cx)).into(),
        Route::Downloads => cx.new(|cx| downloads::Downloads::new(false, window, cx)).into(),
        Route::LiveTv => cx.new(|cx| livetv::LiveTv::new(window, cx)).into(),
        Route::Activity => cx.new(|cx| activity::Activity::new(window, cx)).into(),
        Route::Server => cx.new(|cx| server::Server::new(window, cx)).into(),
        Route::Settings => cx.new(|cx| settings::Settings::new(window, cx)).into(),
        Route::Requests => cx.new(|cx| requests::Requests::new(window, cx)).into(),
        Route::Seerr { media_type, tmdb_id } => cx.new(|cx| seerr_title::SeerrTitle::new(media_type.clone(), *tmdb_id, window, cx)).into(),
    }
}

/// The downloads page as the offline screen shows it (before any server is reached).
pub fn offline_downloads(window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| downloads::Downloads::new(true, window, cx)).into()
}
