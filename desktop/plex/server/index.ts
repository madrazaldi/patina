// The Plex Media Server client (port of crates/plex/src/server/mod.rs and its submodules).

import { Http } from "../client";
import { NotificationsApi } from "./notifications";

export { LibraryQuery, type LibraryQueryInit } from "./library";
export { MetadataOptions, type MetadataOptionsInit } from "./metadata";
export type { Notification } from "./notifications";
export { TimelineState, TranscodeOptions, type TimelineUpdate, type TranscodeOptionsInit } from "./playback";
export { PlayQueueRequest, type PlayQueueRequestInit } from "./playqueue";
export { HistoryQuery, type HistoryQueryInit } from "./status";

/** Client bound to one Plex Media Server connection. */
export class PlexServer extends NotificationsApi {
  /** `PlexServer::with_http`: a client sharing the HTTP plumbing (and default headers) of another. */
  static withHttp(http: Http, baseUrl: string, token: string): PlexServer {
    return new PlexServer(http, baseUrl, token);
  }
}
