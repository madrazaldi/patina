// Hubs and search (port of crates/plex/src/server/hubs.rs).

import { Params } from "../client";
import type { Hub, MediaContainer, Metadata } from "../models";
import { MetadataApi } from "./metadata";

export class HubsApi extends MetadataApi {
  /** Home screen hubs across all libraries. */
  async homeHubs(count: number): Promise<Hub[]> {
    const p = new Params().set("count", count).set("includeLibraryPlaylists", 1).set("includeStations", 0);
    return (await this.get("/hubs", p)).Hub;
  }

  /** Promoted hubs (what Plex apps show on the home screen), optionally per section. */
  async promotedHubs(sectionIds: readonly string[], count: number): Promise<Hub[]> {
    const ids = sectionIds.length > 0 ? sectionIds.join(",") : null;
    const p = new Params().set("count", count).opt("contentDirectoryID", ids);
    return (await this.get("/hubs/promoted", p)).Hub;
  }

  /** "Continue Watching" (partially watched items plus next episodes). */
  async continueWatching(count: number): Promise<Metadata[]> {
    const hubs = (await this.get("/hubs/home/continueWatching", new Params().set("count", count))).Hub;
    return hubs.flatMap((h) => h.Metadata);
  }

  async homeOnDeck(count: number): Promise<Metadata[]> {
    const hubs = (await this.get("/hubs/home/onDeck", new Params().set("count", count))).Hub;
    return hubs.flatMap((h) => h.Metadata);
  }

  /** Recently added across libraries; `kind` filters by media type. */
  async homeRecentlyAdded(kind: number | null | undefined, count: number): Promise<Hub[]> {
    const p = new Params().opt("type", kind).set("count", count);
    return (await this.get("/hubs/home/recentlyAdded", p)).Hub;
  }

  /** Hubs for one library section (recommended view). */
  async sectionHubs(sectionId: string, count: number): Promise<Hub[]> {
    const p = new Params().set("count", count).set("includeLibraryPlaylists", 1);
    return (await this.get(`/hubs/sections/${sectionId}`, p)).Hub;
  }

  /** Related hubs for an item. */
  async metadataHubs(ratingKey: string, count: number): Promise<Hub[]> {
    return (await this.get(`/hubs/metadata/${ratingKey}`, new Params().set("count", count))).Hub;
  }

  /** Items of a hub (via its `key`), paginated. */
  hubItems(hubKey: string, start: number, size: number): Promise<MediaContainer> {
    const p = new Params().set("X-Plex-Container-Start", start).set("X-Plex-Container-Size", size);
    return this.get(hubKey, p);
  }

  /** Search grouped into hubs (movies, shows, episodes, actors, genres, ...). */
  async searchHubs(query: string, limit: number, sectionId?: string | null): Promise<Hub[]> {
    const p = new Params()
      .set("query", query)
      .set("limit", limit)
      .set("includeCollections", 1)
      .set("includeExternalMedia", 0)
      .opt("sectionId", sectionId);
    return (await this.get("/hubs/search", p)).Hub;
  }

  /** Fuzzier search tuned for spoken queries. */
  async searchVoice(query: string, limit: number, kind?: number | null): Promise<Hub[]> {
    const p = new Params().set("query", query).set("limit", limit).opt("type", kind);
    return (await this.get("/hubs/search/voice", p)).Hub;
  }

  /** Legacy flat search. */
  async search(query: string, kind?: number | null): Promise<Metadata[]> {
    const p = new Params().set("query", query).opt("type", kind);
    return (await this.get("/search", p)).Metadata;
  }
}
