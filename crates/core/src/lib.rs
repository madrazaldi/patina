//! Patina's backend, shared by its front ends: sign-in and sessions, mpv
//! playback, downloads, live server updates and the heavier server queries.
//!
//! A front end builds an [`AppState`] with an [`EventSink`] that receives
//! every [`CoreEvent`], and a tokio runtime handle that background work runs on.

use std::sync::OnceLock;

pub mod config;
pub mod controls;
pub mod downloads;
pub mod failure;
pub mod images;
pub mod live;
pub mod livetv;
pub mod lyrics;
pub mod ops;
pub mod player;
pub mod prefs;
pub mod presence;
pub mod recovery;
pub mod report;
pub mod session;
pub mod smart;
pub mod state;
pub mod storage;
pub mod offline;
pub mod tracks;

pub use state::{AppState, CoreEvent, EventSink};

static RUNTIME: OnceLock<tokio::runtime::Handle> = OnceLock::new();

/// The runtime background work runs on: the one given to [`AppState::new`],
/// else the current one.
pub fn runtime() -> tokio::runtime::Handle {
    RUNTIME.get().cloned().unwrap_or_else(tokio::runtime::Handle::current)
}
