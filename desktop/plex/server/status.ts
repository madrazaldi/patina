// Sessions and history (port of crates/plex/src/server/status.rs).

import { Params } from "../client";
import type { MediaContainer, Metadata } from "../models";
import { PlayQueueApi } from "./playqueue";

/** The fields of a `HistoryQuery`; every one is optional. */
export interface HistoryQueryInit {
  accountId?: number | null;
  sectionId?: string | null;
  ratingKey?: string | null;
  /** Only entries viewed at or after this unix timestamp. */
  since?: number | null;
  start?: number;
  size?: number;
}

/** Filters for watch history. */
export class HistoryQuery {
  accountId: number | null;
  sectionId: string | null;
  ratingKey: string | null;
  since: number | null;
  start: number;
  size: number;

  constructor(init: HistoryQueryInit = {}) {
    this.accountId = init.accountId ?? null;
    this.sectionId = init.sectionId ?? null;
    this.ratingKey = init.ratingKey ?? null;
    this.since = init.since ?? null;
    this.start = init.start ?? 0;
    this.size = init.size ?? 0;
  }

  static from(q: HistoryQueryInit): HistoryQuery {
    return q instanceof HistoryQuery ? q : new HistoryQuery(q);
  }
}

export class StatusApi extends PlayQueueApi {
  /** Everything currently playing on the server, with user, player and transcode info. */
  async sessions(): Promise<Metadata[]> {
    return (await this.get("/status/sessions", new Params())).Metadata;
  }

  /** Watch history (newest first). */
  history(query: HistoryQueryInit): Promise<MediaContainer> {
    const q = HistoryQuery.from(query);
    const p = new Params()
      .set("sort", "viewedAt:desc")
      .opt("accountID", q.accountId)
      .opt("librarySectionID", q.sectionId)
      .opt("metadataItemID", q.ratingKey)
      .opt("viewedAt>", q.since)
      .set("X-Plex-Container-Start", q.start)
      .set("X-Plex-Container-Size", q.size === 0 ? 100 : q.size);
    return this.get("/status/sessions/history/all", p);
  }

  deleteHistory(historyId: string): Promise<void> {
    return this.delete(`/status/sessions/history/${historyId}`, new Params());
  }

  /** Stop someone's stream with a message (Plex Pass). */
  terminateSession(sessionId: string, reason: string): Promise<void> {
    const p = new Params().set("sessionId", sessionId).set("reason", reason);
    return this.empty("GET", "/status/sessions/terminate", p);
  }
}
