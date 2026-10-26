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
// anything exotic.

export { ClientInfo, Params, type ParamValue } from "./client";
export { ConnectionCandidate, connectResource } from "./connect";
export { PlexError, type PlexErrorKind } from "./error";
export { MediaType, streamType } from "./media-type";
export * from "./models";
export {
  HistoryQuery,
  LibraryQuery,
  MetadataOptions,
  PlayQueueRequest,
  PlexServer,
  TimelineState,
  TranscodeOptions,
  type HistoryQueryInit,
  type LibraryQueryInit,
  type MetadataOptionsInit,
  type Notification,
  type PlayQueueRequestInit,
  type TimelineUpdate,
  type TranscodeOptionsInit,
} from "./server";
export { PlexTv } from "./tv";
