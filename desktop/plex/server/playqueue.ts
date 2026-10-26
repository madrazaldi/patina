// Play queues (port of crates/plex/src/server/playqueue.rs).

import { Params } from "../client";
import type { MediaContainer } from "../models";
import { PlaybackApi } from "./playback";

/** The fields of a `PlayQueueRequest`; every one is optional (`kind` defaults to the empty string, as in Rust). */
export interface PlayQueueRequestInit {
  /** Library URI of the source (item, show, season, album, section filter...). */
  uri?: string | null;
  /** Build from a playlist instead of a URI. */
  playlistId?: string | null;
  /** `video`, `audio` or `photo`. */
  kind?: string;
  /** Item to start at (a `/library/metadata/<id>` key). */
  key?: string | null;
  shuffle?: boolean;
  repeat?: boolean;
  /** Continue with the following items (next episodes) after `key`. */
  continuous?: boolean;
  includeChapters?: boolean;
  includeMarkers?: boolean;
}

/** Parameters for creating a play queue. */
export class PlayQueueRequest {
  uri: string | null;
  playlistId: string | null;
  kind: string;
  key: string | null;
  shuffle: boolean;
  repeat: boolean;
  continuous: boolean;
  includeChapters: boolean;
  includeMarkers: boolean;

  constructor(init: PlayQueueRequestInit = {}) {
    this.uri = init.uri ?? null;
    this.playlistId = init.playlistId ?? null;
    this.kind = init.kind ?? "";
    this.key = init.key ?? null;
    this.shuffle = init.shuffle ?? false;
    this.repeat = init.repeat ?? false;
    this.continuous = init.continuous ?? false;
    this.includeChapters = init.includeChapters ?? false;
    this.includeMarkers = init.includeMarkers ?? false;
  }

  static from(r: PlayQueueRequestInit): PlayQueueRequest {
    return r instanceof PlayQueueRequest ? r : new PlayQueueRequest(r);
  }

  static video(uri: string): PlayQueueRequest {
    return new PlayQueueRequest({ uri, kind: "video" });
  }
}

export class PlayQueueApi extends PlaybackApi {
  createPlayQueue(request: PlayQueueRequestInit): Promise<MediaContainer> {
    const req = PlayQueueRequest.from(request);
    const p = new Params()
      .set("type", req.kind)
      .opt("uri", req.uri)
      .opt("playlistID", req.playlistId)
      .opt("key", req.key)
      .set("shuffle", req.shuffle ? 1 : 0)
      .set("repeat", req.repeat ? 1 : 0)
      .set("continuous", req.continuous ? 1 : 0)
      .flag("includeChapters", req.includeChapters)
      .flag("includeMarkers", req.includeMarkers)
      .set("own", 1);
    return this.container("POST", "/playQueues", p);
  }

  /** Fetch a play queue; `window` limits items around the selected one. */
  playQueue(id: number, window?: number | null): Promise<MediaContainer> {
    const p = new Params().opt("window", window).set("includeMarkers", 1).set("includeChapters", 1);
    return this.get(`/playQueues/${id}`, p);
  }

  /** Add items; `next` inserts after the current item ("Play Next"). */
  addToPlayQueue(id: number, uri: string, next: boolean): Promise<MediaContainer> {
    const p = new Params().set("uri", uri).flag("next", next);
    return this.container("PUT", `/playQueues/${id}`, p);
  }

  removeFromPlayQueue(id: number, itemId: number): Promise<MediaContainer> {
    return this.container("DELETE", `/playQueues/${id}/items/${itemId}`, new Params());
  }

  movePlayQueueItem(id: number, itemId: number, after?: number | null): Promise<MediaContainer> {
    return this.container("PUT", `/playQueues/${id}/items/${itemId}/move`, new Params().opt("after", after));
  }

  shufflePlayQueue(id: number): Promise<MediaContainer> {
    return this.container("PUT", `/playQueues/${id}/shuffle`, new Params());
  }

  unshufflePlayQueue(id: number): Promise<MediaContainer> {
    return this.container("PUT", `/playQueues/${id}/unshuffle`, new Params());
  }

  clearPlayQueue(id: number): Promise<MediaContainer> {
    return this.container("DELETE", `/playQueues/${id}/items`, new Params());
  }

  /** Reset the queue position to the first item. */
  resetPlayQueue(id: number): Promise<MediaContainer> {
    return this.container("PUT", `/playQueues/${id}/reset`, new Params());
  }
}
