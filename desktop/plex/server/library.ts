// Library sections, listings and maintenance (port of crates/plex/src/server/library.rs).

import { Params } from "../client";
import type { Directory, LibraryMeta, MediaContainer, Metadata, Setting } from "../models";
import { ServerBase } from "./mod";

/** The fields of a `LibraryQuery`; every one is optional (Rust `..Default::default()`). */
export interface LibraryQueryInit {
  /** Plex media type id (see `MediaType`); e.g. episodes of a show library. */
  kind?: number | null;
  /** Sort key, e.g. `titleSort`, `addedAt:desc`, `year:desc`, `rating:desc`. */
  sort?: string | null;
  unwatched?: boolean;
  /** Free-form filters: `genre=12`, `year=1999`, `title=alien`, `resolution=4k`... */
  filters?: Params;
  start?: number;
  size?: number;
}

/** Query for browsing a library section. */
export class LibraryQuery {
  kind: number | null;
  sort: string | null;
  unwatched: boolean;
  filters: Params;
  start: number;
  size: number;

  constructor(init: LibraryQueryInit = {}) {
    this.kind = init.kind ?? null;
    this.sort = init.sort ?? null;
    this.unwatched = init.unwatched ?? false;
    this.filters = init.filters ?? new Params();
    this.start = init.start ?? 0;
    this.size = init.size ?? 0;
  }

  static page(start: number, size: number): LibraryQuery {
    return new LibraryQuery({ start, size });
  }

  static from(q: LibraryQueryInit): LibraryQuery {
    return q instanceof LibraryQuery ? q : new LibraryQuery(q);
  }

  params(): Params {
    const p = new Params()
      .opt("type", this.kind)
      .opt("sort", this.sort)
      .flag("unwatched", this.unwatched)
      .extend(this.filters)
      .set("includeGuids", 1)
      .set("X-Plex-Container-Start", this.start);
    if (this.size > 0) p.push("X-Plex-Container-Size", this.size);
    return p;
  }
}

export class LibraryApi extends ServerBase {
  /** All library sections. */
  async sections(): Promise<Directory[]> {
    return (await this.get("/library/sections", new Params())).Directory;
  }

  /** Section details, including the browse sub-directories (unwatched, genre, year, ...). */
  section(id: string): Promise<MediaContainer> {
    return this.get(`/library/sections/${id}`, new Params().set("includeDetails", 1));
  }

  /** Browse a section's items. */
  sectionItems(id: string, query: LibraryQueryInit): Promise<MediaContainer> {
    return this.get(`/library/sections/${id}/all`, LibraryQuery.from(query).params());
  }

  /**
   * Any section sub-listing: `unwatched`, `newest`, `recentlyAdded`, `onDeck`,
   * `recentlyViewed`, `genre`, `year`, `decade`, `director`, `actor`,
   * `contentRating`, `resolution`, `firstCharacter`, `collection`, `folder`, ...
   */
  sectionListing(id: string, listing: string, params: Params = new Params()): Promise<MediaContainer> {
    return this.get(`/library/sections/${id}/${listing}`, params);
  }

  async sectionFilters(id: string): Promise<Directory[]> {
    return (await this.sectionListing(id, "filters", new Params())).Directory;
  }

  async sectionSorts(id: string): Promise<Directory[]> {
    return (await this.sectionListing(id, "sorts", new Params())).Directory;
  }

  /** Values of a filter (e.g. all genres) as `Directory` entries whose `key` is the filter value. */
  async sectionFilterValues(id: string, filter: string, kind?: number | null): Promise<Directory[]> {
    return (await this.sectionListing(id, filter, new Params().opt("type", kind))).Directory;
  }

  /** Types, filters, sorts and editable fields for a section (`includeMeta=1`). */
  async sectionMeta(id: string): Promise<LibraryMeta | null> {
    const params = new Params().set("includeMeta", 1).set("X-Plex-Container-Size", 0);
    return (await this.get(`/library/sections/${id}/all`, params)).Meta;
  }

  async sectionFirstCharacters(id: string, kind?: number | null): Promise<Directory[]> {
    return (await this.sectionListing(id, "firstCharacter", new Params().opt("type", kind))).Directory;
  }

  async sectionOnDeck(id: string): Promise<Metadata[]> {
    return (await this.sectionListing(id, "onDeck", new Params())).Metadata;
  }

  async sectionRecentlyAdded(id: string, size: number): Promise<Metadata[]> {
    const p = new Params().set("X-Plex-Container-Start", 0).set("X-Plex-Container-Size", size);
    return (await this.sectionListing(id, "recentlyAdded", p)).Metadata;
  }

  /** Browse a section by folder. `parent` is a key from a previous call. */
  sectionFolders(id: string, parent?: string | null): Promise<MediaContainer> {
    if (parent !== null && parent !== undefined) return this.get(parent, new Params());
    return this.sectionListing(id, "folder", new Params());
  }

  // ----- Library-wide ---------------------------------------------------

  async onDeck(): Promise<Metadata[]> {
    return (await this.get("/library/onDeck", new Params())).Metadata;
  }

  async recentlyAdded(size: number): Promise<Metadata[]> {
    const p = new Params().set("X-Plex-Container-Start", 0).set("X-Plex-Container-Size", size);
    return (await this.get("/library/recentlyAdded", p)).Metadata;
  }

  /** Search all libraries with arbitrary filters (`/library/all`). */
  libraryAll(params: Params): Promise<MediaContainer> {
    return this.get("/library/all", params);
  }

  /** Find the local copy of a plex:// (or imdb://, tmdb://, tvdb://) guid. */
  async findByGuid(guid: string): Promise<Metadata[]> {
    return (await this.libraryAll(new Params().set("guid", guid))).Metadata;
  }

  /** Every tag of a given kind (`genre`, `actor`, ...), optionally for one type. */
  async tags(tagType: number): Promise<Directory[]> {
    return (await this.get("/library/tags", new Params().set("type", tagType))).Directory;
  }

  // ----- Maintenance ----------------------------------------------------

  /** Scan a section for new files. `force` re-reads metadata; `path` limits to a subfolder. */
  refreshSection(id: string, force: boolean, path?: string | null): Promise<void> {
    const p = new Params().flag("force", force).opt("path", path);
    return this.empty("GET", `/library/sections/${id}/refresh`, p);
  }

  cancelRefreshSection(id: string): Promise<void> {
    return this.delete(`/library/sections/${id}/refresh`, new Params());
  }

  refreshAllSections(force: boolean): Promise<void> {
    return this.empty("GET", "/library/sections/all/refresh", new Params().flag("force", force));
  }

  emptyTrash(id: string): Promise<void> {
    return this.put(`/library/sections/${id}/emptyTrash`, new Params());
  }

  analyzeSection(id: string): Promise<void> {
    return this.put(`/library/sections/${id}/analyze`, new Params());
  }

  cleanBundles(): Promise<void> {
    return this.put("/library/clean/bundles", new Params());
  }

  optimizeDatabase(): Promise<void> {
    return this.put("/library/optimize", new Params().set("async", 1));
  }

  /**
   * Create a library. Params: `name`, `type` (movie/show/artist/photo), `agent`,
   * `scanner`, `language`, `location` (repeatable), plus `prefs[...]`.
   */
  createSection(params: Params): Promise<MediaContainer> {
    return this.container("POST", "/library/sections", params);
  }

  editSection(id: string, params: Params): Promise<void> {
    return this.put(`/library/sections/${id}`, params);
  }

  deleteSection(id: string): Promise<void> {
    return this.delete(`/library/sections/${id}`, new Params());
  }

  /** Per-section preferences. */
  async sectionPrefs(id: string): Promise<Setting[]> {
    return (await this.get(`/library/sections/${id}/prefs`, new Params())).Setting;
  }

  setSectionPrefs(id: string, params: Params): Promise<void> {
    return this.put(`/library/sections/${id}/prefs`, params);
  }

  /** Available metadata agents for a media type. */
  agents(mediaType?: number | null): Promise<MediaContainer> {
    return this.get("/system/agents", new Params().opt("mediaType", mediaType));
  }

  /** Available scanners for a media type. */
  scanners(mediaType: number): Promise<MediaContainer> {
    return this.get("/system/scanners", new Params().set("type", mediaType));
  }
}
