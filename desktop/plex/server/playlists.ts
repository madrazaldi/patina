// Playlists (port of crates/plex/src/server/playlists.rs).

import { Params } from "../client";
import type { MediaContainer, Metadata } from "../models";
import { HubsApi } from "./hubs";

export class PlaylistsApi extends HubsApi {
  /** All playlists; `kind` is `video`, `audio` or `photo`. */
  async playlists(kind?: string | null, smart?: boolean | null): Promise<Metadata[]> {
    const p = new Params().opt("playlistType", kind).opt("smart", smart === null || smart === undefined ? null : smart ? 1 : 0);
    return (await this.get("/playlists", p)).Metadata;
  }

  async playlist(id: string): Promise<Metadata | null> {
    return (await this.get(`/playlists/${id}`, new Params())).Metadata[0] ?? null;
  }

  playlistItems(id: string, start: number, size: number): Promise<MediaContainer> {
    const p = new Params().set("X-Plex-Container-Start", start).set("X-Plex-Container-Size", size);
    return this.get(`/playlists/${id}/items`, p);
  }

  /**
   * Create a playlist from item URIs (see `itemUri`).
   * For smart playlists pass a library URI with filters and `smart = true`.
   */
  async createPlaylist(title: string, kind: string, uri: string, smart: boolean): Promise<Metadata | null> {
    const p = new Params().set("title", title).set("type", kind).set("uri", uri).set("smart", smart ? 1 : 0);
    return (await this.container("POST", "/playlists", p)).Metadata[0] ?? null;
  }

  addToPlaylist(id: string, uri: string): Promise<void> {
    return this.put(`/playlists/${id}/items`, new Params().set("uri", uri));
  }

  removeFromPlaylist(id: string, playlistItemId: number): Promise<void> {
    return this.delete(`/playlists/${id}/items/${playlistItemId}`, new Params());
  }

  /** Move an item after another one; `after = null` moves it to the top. */
  movePlaylistItem(id: string, playlistItemId: number, after?: number | null): Promise<void> {
    return this.put(`/playlists/${id}/items/${playlistItemId}/move`, new Params().opt("after", after));
  }

  clearPlaylist(id: string): Promise<void> {
    return this.delete(`/playlists/${id}/items`, new Params());
  }

  updatePlaylist(id: string, title?: string | null, summary?: string | null): Promise<void> {
    const p = new Params().opt("title", title).opt("summary", summary);
    return this.put(`/playlists/${id}`, p);
  }

  deletePlaylist(id: string): Promise<void> {
    return this.delete(`/playlists/${id}`, new Params());
  }

  /** Import an .m3u file that lives on the server's filesystem. */
  uploadPlaylist(sectionId: string, path: string, force: boolean): Promise<void> {
    const p = new Params().set("sectionID", sectionId).set("path", path).flag("force", force);
    return this.post("/playlists/upload", p);
  }
}
