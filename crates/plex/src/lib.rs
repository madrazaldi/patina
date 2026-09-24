//! Async client for the Plex Media Server and plex.tv APIs.
//!
//! The crate is split in two halves:
//!
//! * [`PlexTv`] talks to plex.tv / discover.provider.plex.tv: PIN and password
//!   sign-in, account info, resource (server) discovery, Plex Home user
//!   switching, friends and the watchlist.
//! * [`PlexServer`] talks to a single Plex Media Server: libraries, metadata,
//!   hubs, search, playlists, collections, play queues, timeline reporting,
//!   transcoding, sessions, history, butler tasks, preferences, statistics,
//!   updates, live TV & DVR and the live notification stream.
//!
//! Every endpoint has a typed wrapper; [`PlexServer::raw`] and
//! [`PlexTv::raw`] remain available for anything exotic.

mod client;
mod connect;
mod de;
mod error;
pub mod models;
mod server;
mod tv;

pub use client::{ClientInfo, Params};
pub use connect::{ConnectionCandidate, connect_resource};
pub use error::{Error, Result};
pub use models::*;
pub use server::{
    HistoryQuery, LibraryQuery, MetadataOptions, Notification, PlayQueueRequest, PlexServer,
    TimelineState, TimelineUpdate, TranscodeOptions,
};
pub use tv::PlexTv;

/// Plex's numeric media type identifiers, used for `type=` query parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaType {
    Movie = 1,
    Show = 2,
    Season = 3,
    Episode = 4,
    Trailer = 5,
    Person = 7,
    Artist = 8,
    Album = 9,
    Track = 10,
    Clip = 12,
    Photo = 13,
    PhotoAlbum = 14,
    Playlist = 15,
    PlaylistFolder = 16,
    Collection = 18,
}

impl MediaType {
    pub fn id(self) -> u32 {
        self as u32
    }

    /// Map a metadata `type` string ("movie", "episode", ...) to its id.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "movie" => Self::Movie,
            "show" => Self::Show,
            "season" => Self::Season,
            "episode" => Self::Episode,
            "trailer" => Self::Trailer,
            "person" => Self::Person,
            "artist" => Self::Artist,
            "album" => Self::Album,
            "track" => Self::Track,
            "clip" => Self::Clip,
            "photo" => Self::Photo,
            "photoalbum" => Self::PhotoAlbum,
            "playlist" => Self::Playlist,
            "playlistFolder" => Self::PlaylistFolder,
            "collection" => Self::Collection,
            _ => return None,
        })
    }
}

/// Stream types found on [`Stream::stream_type`].
pub mod stream_type {
    pub const VIDEO: i64 = 1;
    pub const AUDIO: i64 = 2;
    pub const SUBTITLE: i64 = 3;
    pub const LYRICS: i64 = 4;
}
