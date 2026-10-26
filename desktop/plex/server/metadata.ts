// Item metadata, watch state, editing, matching, artwork and streams
// (port of crates/plex/src/server/metadata.rs).

import { Params } from "../client";
import { pointer } from "../de";
import {
  Metadata,
  normalizeStream,
  type Hub,
  type MatchResult,
  type MediaContainer,
  type Stream,
} from "../models";
import { LibraryApi } from "./library";

/** The fields of a `MetadataOptions`; every one is optional. */
export interface MetadataOptionsInit {
  children?: boolean;
  extras?: boolean;
  related?: boolean;
  markers?: boolean;
  chapters?: boolean;
  onDeck?: boolean;
  reviews?: boolean;
  concerts?: boolean;
  /** Radio stations of an artist (or album / track). */
  stations?: boolean;
}

/** Optional sections to include when fetching an item. */
export class MetadataOptions {
  children: boolean;
  extras: boolean;
  related: boolean;
  markers: boolean;
  chapters: boolean;
  onDeck: boolean;
  reviews: boolean;
  concerts: boolean;
  stations: boolean;

  constructor(init: MetadataOptionsInit = {}) {
    this.children = init.children ?? false;
    this.extras = init.extras ?? false;
    this.related = init.related ?? false;
    this.markers = init.markers ?? false;
    this.chapters = init.chapters ?? false;
    this.onDeck = init.onDeck ?? false;
    this.reviews = init.reviews ?? false;
    this.concerts = init.concerts ?? false;
    this.stations = init.stations ?? false;
  }

  static from(o: MetadataOptionsInit): MetadataOptions {
    return o instanceof MetadataOptions ? o : new MetadataOptions(o);
  }

  /** Everything a detail page needs. */
  static full(): MetadataOptions {
    return new MetadataOptions({
      children: false,
      extras: true,
      related: true,
      markers: true,
      chapters: true,
      onDeck: true,
      reviews: true,
      concerts: false,
      stations: false,
    });
  }

  /** What the player needs: markers and chapters. */
  static playback(): MetadataOptions {
    return new MetadataOptions({ markers: true, chapters: true });
  }

  params(): Params {
    return new Params()
      .flag("includeChildren", this.children)
      .flag("includeExtras", this.extras)
      .flag("includeRelated", this.related)
      .flag("includeMarkers", this.markers)
      .flag("includeChapters", this.chapters)
      .flag("includeOnDeck", this.onDeck)
      .flag("includeReviews", this.reviews)
      .flag("includeConcerts", this.concerts)
      .flag("includeStations", this.stations)
      .set("includeGuids", 1)
      .set("asyncAugmentMetadata", 0);
  }
}

/**
 * `futures::stream::iter(items).map(f).buffered(limit).try_collect()`: at most `limit`
 * calls in flight, results in input order, the first error (in order) wins.
 */
async function buffered<T, R>(items: T[], limit: number, f: (item: T) => Promise<R>): Promise<R[]> {
  const out: R[] = [];
  const inflight: Promise<R>[] = [];
  let next = 0;
  while (out.length < items.length) {
    while (inflight.length < limit && next < items.length) {
      const p = f(items[next++]!);
      p.catch(() => undefined); // errors surface when awaited in order; avoid unhandled rejections
      inflight.push(p);
    }
    out.push(await inflight.shift()!);
  }
  return out;
}

function libKey(ratingKey: string): Params {
  return new Params().set("key", ratingKey).set("identifier", "com.plexapp.plugins.library");
}

export class MetadataApi extends LibraryApi {
  async metadata(ratingKey: string, opts: MetadataOptionsInit = {}): Promise<Metadata | null> {
    const mc = await this.get(`/library/metadata/${ratingKey}`, MetadataOptions.from(opts).params());
    return mc.Metadata[0] ?? null;
  }

  /** Several items in one request. */
  async metadataMany(ratingKeys: readonly string[]): Promise<Metadata[]> {
    if (ratingKeys.length === 0) return [];
    return (await this.get(`/library/metadata/${ratingKeys.join(",")}`, new Params())).Metadata;
  }

  /** Direct children: seasons of a show, episodes of a season, tracks of an album. */
  children(ratingKey: string): Promise<MediaContainer> {
    return this.get(`/library/metadata/${ratingKey}/children`, new Params().set("includeGuids", 1));
  }

  /** All leaves: every episode of a show, every track of an artist. */
  allLeaves(ratingKey: string): Promise<MediaContainer> {
    return this.get(`/library/metadata/${ratingKey}/allLeaves`, new Params());
  }

  /**
   * Every leaf under an item, with full stream details. `allLeaves` leaves
   * streams out, so the items are fetched again in batches; any the batch
   * endpoint still returns without streams are fetched one by one.
   */
  async allLeavesWithStreams(ratingKey: string): Promise<Metadata[]> {
    const keys = (await this.allLeaves(ratingKey)).Metadata.map((m) => Metadata.ratingKey(m));
    const chunks: string[][] = [];
    for (let i = 0; i < keys.length; i += 40) chunks.push(keys.slice(i, i + 40));
    const batches = await buffered(chunks, 4, (chunk) => this.metadataMany(chunk));
    const items = await buffered(batches.flat(), 8, async (item) => {
      const missing = item.Media.flatMap((m) => m.Part).some((p) => p.Stream.length === 0);
      if (!missing) return item;
      return (await this.metadata(Metadata.ratingKey(item))) ?? item;
    });
    // Keep allLeaves order even if the batch endpoint reorders.
    const position = (m: Metadata): number => keys.indexOf(Metadata.ratingKey(m)); // -1 (None) sorts first, like Option
    return items.sort((a, b) => position(a) - position(b));
  }

  grandchildren(ratingKey: string): Promise<MediaContainer> {
    return this.get(`/library/metadata/${ratingKey}/grandchildren`, new Params());
  }

  /** "More like this" and related hubs. */
  async related(ratingKey: string): Promise<Hub[]> {
    return (await this.get(`/library/metadata/${ratingKey}/related`, new Params())).Hub;
  }

  async similar(ratingKey: string): Promise<Metadata[]> {
    return (await this.get(`/library/metadata/${ratingKey}/similar`, new Params())).Metadata;
  }

  /** Trailers, featurettes, deleted scenes, ... */
  async extras(ratingKey: string): Promise<Metadata[]> {
    return (await this.get(`/library/metadata/${ratingKey}/extras`, new Params())).Metadata;
  }

  /** Items surrounding an item in its parent (e.g. neighbouring episodes). */
  async nearest(ratingKey: string): Promise<Metadata[]> {
    return (await this.get(`/library/metadata/${ratingKey}/nearest`, new Params())).Metadata;
  }

  // ----- Watch state & ratings -----------------------------------------

  scrobble(ratingKey: string): Promise<void> {
    return this.empty("GET", "/:/scrobble", libKey(ratingKey));
  }

  unscrobble(ratingKey: string): Promise<void> {
    return this.empty("GET", "/:/unscrobble", libKey(ratingKey));
  }

  /** Rate on a 0-10 scale; `null` clears the rating. */
  rate(ratingKey: string, rating?: number | null): Promise<void> {
    const p = libKey(ratingKey).set("rating", rating ?? -1);
    return this.put("/:/rate", p);
  }

  /** Hide an item from Continue Watching / On Deck until it is played again. */
  removeFromContinueWatching(ratingKey: string): Promise<void> {
    return this.put("/actions/removeFromContinueWatching", new Params().set("ratingKey", ratingKey));
  }

  /** The first radio station of an artist, album or track (sonic analysis required). */
  async station(ratingKey: string): Promise<Metadata | null> {
    const m = await this.metadata(ratingKey, { stations: true });
    return m?.Stations[0] ?? null;
  }

  /** Store a resume position without a playback session. */
  setProgress(ratingKey: string, timeMs: number, state: string): Promise<void> {
    const p = libKey(ratingKey).set("time", timeMs).set("state", state);
    return this.empty("GET", "/:/progress", p);
  }

  // ----- Maintenance & editing -----------------------------------------

  refreshMetadata(ratingKey: string, force: boolean): Promise<void> {
    return this.put(`/library/metadata/${ratingKey}/refresh`, new Params().flag("force", force));
  }

  analyzeMetadata(ratingKey: string): Promise<void> {
    return this.put(`/library/metadata/${ratingKey}/analyze`, new Params());
  }

  /** Delete an item and its files from disk (requires the server setting to allow it). */
  deleteMetadata(ratingKey: string): Promise<void> {
    return this.delete(`/library/metadata/${ratingKey}`, new Params());
  }

  /**
   * Edit fields: pairs like `["title", "New"]`, `["summary", "..."]`.
   * Each field is written as `<field>.value` and optionally locked.
   * Uses the section endpoint when `sectionId` is known (works on all server versions).
   */
  editMetadata(
    ratingKey: string,
    sectionId: number | null | undefined,
    kind: number,
    fields: readonly (readonly [string, string])[],
    lock: boolean,
  ): Promise<void> {
    const p = new Params().set("type", kind).set("id", ratingKey);
    for (const [field, value] of fields) {
      p.push(`${field}.value`, value);
      if (lock) p.push(`${field}.locked`, 1);
    }
    if (sectionId !== null && sectionId !== undefined) return this.put(`/library/sections/${sectionId}/all`, p);
    return this.put(`/library/metadata/${ratingKey}`, p);
  }

  /**
   * Lock or unlock fields (`title`, `summary`, `label`, ...) without changing
   * their values; locked fields are kept when agents refresh the item.
   */
  lockFields(
    ratingKey: string,
    sectionId: number | null | undefined,
    kind: number,
    fields: readonly (readonly [string, boolean])[],
  ): Promise<void> {
    const p = new Params().set("type", kind).set("id", ratingKey);
    for (const [field, locked] of fields) p.push(`${field}.locked`, locked ? 1 : 0);
    if (sectionId !== null && sectionId !== undefined) return this.put(`/library/sections/${sectionId}/all`, p);
    return this.put(`/library/metadata/${ratingKey}`, p);
  }

  /** Add or remove tags (genre, collection, label, ...). */
  editTags(ratingKey: string, kind: number, tag: string, add: readonly string[], remove: readonly string[]): Promise<void> {
    const p = new Params().set("type", kind).set("id", ratingKey);
    add.forEach((v, i) => p.push(`${tag}[${i}].tag.tag`, v));
    if (remove.length > 0) p.push(`${tag}[].tag.tag-`, remove.join(","));
    return this.put(`/library/metadata/${ratingKey}`, p);
  }

  /** Item-level preferences (e.g. show episode ordering, collection mode). */
  setMetadataPrefs(ratingKey: string, params: Params): Promise<void> {
    return this.put(`/library/metadata/${ratingKey}/prefs`, params);
  }

  // ----- Matching --------------------------------------------------------

  async matches(
    ratingKey: string,
    title?: string | null,
    year?: number | null,
    agent?: string | null,
  ): Promise<MatchResult[]> {
    const p = new Params().set("manual", 1).opt("title", title).opt("year", year).opt("agent", agent);
    return (await this.get(`/library/metadata/${ratingKey}/matches`, p)).SearchResult;
  }

  applyMatch(ratingKey: string, guid: string, name?: string | null): Promise<void> {
    const p = new Params().set("guid", guid).opt("name", name);
    return this.put(`/library/metadata/${ratingKey}/match`, p);
  }

  unmatch(ratingKey: string): Promise<void> {
    return this.put(`/library/metadata/${ratingKey}/unmatch`, new Params());
  }

  // ----- Artwork ---------------------------------------------------------

  async posters(ratingKey: string): Promise<Metadata[]> {
    return (await this.get(`/library/metadata/${ratingKey}/posters`, new Params())).Metadata;
  }

  async arts(ratingKey: string): Promise<Metadata[]> {
    return (await this.get(`/library/metadata/${ratingKey}/arts`, new Params())).Metadata;
  }

  /** Select a poster by its URL/key from `posters`. */
  setPoster(ratingKey: string, url: string): Promise<void> {
    return this.put(`/library/metadata/${ratingKey}/poster`, new Params().set("url", url));
  }

  setArt(ratingKey: string, url: string): Promise<void> {
    return this.put(`/library/metadata/${ratingKey}/art`, new Params().set("url", url));
  }

  // ----- Streams & subtitles ---------------------------------------------

  /**
   * Persist the preferred audio / subtitle stream for a part.
   * `subtitle = 0` disables subtitles.
   */
  selectStreams(
    partId: number,
    audio: number | null | undefined,
    subtitle: number | null | undefined,
    allParts: boolean,
  ): Promise<void> {
    const p = new Params()
      .opt("audioStreamID", audio)
      .opt("subtitleStreamID", subtitle)
      .flag("allParts", allParts);
    return this.put(`/library/parts/${partId}`, p);
  }

  /** Search online subtitle providers for an item. */
  async searchSubtitles(ratingKey: string, language: string, hearingImpaired: boolean, forced: boolean): Promise<Stream[]> {
    const p = new Params()
      .set("language", language)
      .flag("hearingImpaired", hearingImpaired)
      .flag("forced", forced);
    const v = await this.raw("GET", `/library/metadata/${ratingKey}/subtitles`, p);
    const list = pointer(v, "/MediaContainer/Stream");
    // `from_value(list).unwrap_or_default()`: anything that does not decode gives an empty list.
    if (!Array.isArray(list)) return [];
    try {
      return list.map(normalizeStream);
    } catch {
      return [];
    }
  }

  /** Download an online subtitle found with `searchSubtitles`. */
  downloadSubtitle(ratingKey: string, key: string): Promise<void> {
    return this.put(`/library/metadata/${ratingKey}/subtitles`, new Params().set("key", key));
  }

  deleteStream(streamId: number): Promise<void> {
    return this.delete(`/library/streams/${streamId}`, new Params());
  }

  /** Direct URL for a part (the original file), with the token embedded. */
  partUrl(partKey: string, download: boolean): URL {
    return this.authedUrl(partKey, new Params().flag("download", download));
  }
}
