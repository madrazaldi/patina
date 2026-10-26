// Watchlist, playlists and watch history.

import type { Metadata, PlexServer } from "../../plex";
import type { AppState } from "../state";
import { errorText } from "./util";

/** The plex.tv watchlist, up to 1000 items. */
export async function watchlist(app: AppState): Promise<Metadata[]> {
  // Discover caps the page size at an undocumented value that has shrunk
  // before (even 50 was refused at times), so page and step down when refused.
  const tv = app.plexTv();
  const sizes = [50, 20, 10];
  let at = 0;
  let size = sizes[at]!;
  const items: Metadata[] = [];
  while (true) {
    let page;
    try {
      page = await tv.watchlist(items.length, size);
    } catch (e) {
      if (errorText(e).includes("container-size") && at + 1 < sizes.length) {
        at += 1;
        size = sizes[at]!;
        continue;
      }
      throw e;
    }
    const total = page.totalSize != null ? page.totalSize : null;
    const got = page.Metadata.length;
    items.push(...page.Metadata);
    if (got < size || (total !== null && items.length >= total) || items.length >= 1000) break;
  }
  return items;
}

export async function watchlisted(app: AppState, discoverId: string): Promise<boolean> {
  const state = await app.plexTv().userState(discoverId);
  return state != null && state.watchlistedAt != null;
}

export interface PlaylistView {
  playlist: Metadata;
  items: Metadata[];
  /**
   * How many entries the server says the playlist has; more than
   * `items.length` only when the safety limit cut loading short.
   */
  total: number;
}

/** Entries requested per page of a playlist. */
export const PLAYLIST_PAGE = 1000;
/** A runaway guard: no front end loads more entries than this. */
export const PLAYLIST_LIMIT = 100_000;

export async function playlist(app: AppState, id: string): Promise<PlaylistView | null> {
  const server = app.server();
  const found = await server.playlist(id);
  if (!found) return null;
  const [items, total] = await playlistEntries(server, id);
  return { playlist: found, items, total };
}

/**
 * Every entry of a playlist (paging through the server's limit) and the
 * total the server reports.
 */
export async function playlistEntries(server: PlexServer, id: string): Promise<[Metadata[], number]> {
  return collectPages(PLAYLIST_PAGE, PLAYLIST_LIMIT, async (start, size) => {
    const mc = await server.playlistItems(id, start, size);
    const t = mc.totalSize ?? mc.size;
    const total = t != null ? Math.max(t, 0) : null;
    return [mc.Metadata, total];
  });
}

/**
 * Fetch consecutive pages until the reported total, a short page, or
 * `limit` entries. `fetch(start, size)` returns a page and the total the
 * server reports (when it does). The returned total is what the server
 * said, or what was loaded when it said nothing.
 */
export async function collectPages<T>(
  size: number,
  limit: number,
  fetch: (start: number, size: number) => Promise<[T[], number | null]>,
): Promise<[T[], number]> {
  const items: T[] = [];
  let total: number | null = null;
  while (true) {
    const [page, reported] = await fetch(items.length, size);
    total = reported ?? total;
    const got = page.length;
    items.push(...page);
    // Without a total a short page means the end; with one keep going
    // until it is reached, since servers may cap pages below `size`.
    const done = got === 0 || (total === null ? got < size : items.length >= total) || items.length >= limit;
    if (done) {
      items.length = Math.min(items.length, limit);
      return [items, total ?? items.length];
    }
  }
}

/** Add an item to an existing playlist, or to a new one called `title` of `kind`. */
export async function addToPlaylist(
  app: AppState,
  ratingKey: string,
  playlist?: string | null,
  title?: string | null,
  kind = "video",
): Promise<void> {
  const server = app.server();
  const uri = server.itemUri([ratingKey]);
  if (playlist != null) {
    await server.addToPlaylist(playlist, uri);
  } else if (title != null) {
    await server.createPlaylist(title, kind, uri, false);
  } else {
    throw new Error("No playlist given.");
  }
}

export interface HistoryPage {
  items: Metadata[];
  hasMore: boolean;
  /** Account names by id (owners only). */
  accounts: Record<number, string>;
}

export const HISTORY_PAGE = 60;

/** Page `page` of the server's watch history, newest first. */
export async function history(app: AppState, page: number): Promise<HistoryPage> {
  const server = app.server();
  const owned = app.session()?.owned ?? false;
  const mc = await server.history({ start: page * HISTORY_PAGE, size: HISTORY_PAGE });
  const accounts: Record<number, string> = {};
  if (owned) {
    const list = await server.accounts().catch(() => []);
    for (const a of list) {
      if (a.id != null && a.name != null) accounts[a.id] = a.name;
    }
  }
  const hasMore = containerHasMore(mc) || mc.Metadata.length === HISTORY_PAGE;
  return { items: mc.Metadata, hasMore, accounts };
}

/** `MediaContainer::has_more`: whether there are more items after this page. */
function containerHasMore(mc: { offset?: number | null; size?: number | null; totalSize?: number | null }): boolean {
  if (mc.offset != null && mc.size != null && mc.totalSize != null) return mc.offset + mc.size < mc.totalSize;
  return false;
}

/** Delete a history entry by its `historyKey`. */
export async function deleteHistory(app: AppState, key: string): Promise<void> {
  await app.server().deleteHistory(lastSegment(key));
}

/** The id at the end of a key like `/status/sessions/history/12`. */
export function lastSegment(key: string): string {
  return key.slice(key.lastIndexOf("/") + 1);
}
