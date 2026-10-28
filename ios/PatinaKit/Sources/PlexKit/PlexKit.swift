// Port of desktop/plex/index.ts.
// Async client for the Plex Media Server and plex.tv APIs (port of crates/plex/src/lib.rs).
//
// The package is split in two halves:
//
// * `PlexTv` talks to plex.tv / discover.provider.plex.tv: PIN and password sign-in, account
//   info, resource (server) discovery, Plex Home user switching, friends and the watchlist.
// * `PlexServer` talks to a single Plex Media Server: libraries, metadata, hubs, search,
//   playlists, collections, play queues, timeline reporting, transcoding, sessions, history,
//   butler tasks, preferences, statistics, updates, live TV & DVR and the live notification
//   stream.
//
// Every endpoint has a typed wrapper; `PlexServer.raw` and `PlexTv.raw` remain available for
// anything exotic. Everything public in the module is what index.ts exports; Swift needs no
// export list.
