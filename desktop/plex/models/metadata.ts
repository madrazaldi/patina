// Metadata items and their nested blocks (port of crates/plex/src/models/metadata.rs).

import { boolean, extraOf, list, obj, oneOrMany, optF64, optI64, optString, optional } from "../de";
import { Media, normalizeMedia } from "./media";
import { Hub, normalizeHub } from "./container";
import { type Player, type SessionInfo, type SessionUser, type TranscodeSession, normalizePlayer, normalizeSessionInfo, normalizeSessionUser, normalizeTranscodeSession } from "./server";

/**
 * A metadata item: movie, show, season, episode, artist, album, track, photo,
 * clip, playlist, collection, history entry, or live session.
 */
export interface Metadata {
  ratingKey: string | null;
  key: string | null;
  guid: string | null;
  type: string | null;
  subtype: string | null;
  title: string | null;
  titleSort: string | null;
  originalTitle: string | null;
  editionTitle: string | null;
  studio: string | null;
  contentRating: string | null;
  summary: string | null;
  tagline: string | null;
  year: number | null;
  index: number | null;
  parentIndex: number | null;
  rating: number | null;
  audienceRating: number | null;
  userRating: number | null;
  ratingImage: string | null;
  audienceRatingImage: string | null;
  viewCount: number | null;
  skipCount: number | null;
  viewOffset: number | null;
  lastViewedAt: number | null;
  lastRatedAt: number | null;
  addedAt: number | null;
  updatedAt: number | null;
  originallyAvailableAt: string | null;
  duration: number | null;
  thumb: string | null;
  art: string | null;
  banner: string | null;
  theme: string | null;
  composite: string | null;
  chapterSource: string | null;
  primaryExtraKey: string | null;
  extraType: number | null;
  parentRatingKey: string | null;
  parentKey: string | null;
  parentGuid: string | null;
  parentTitle: string | null;
  parentThumb: string | null;
  parentArt: string | null;
  parentTheme: string | null;
  parentYear: number | null;
  parentStudio: string | null;
  grandparentRatingKey: string | null;
  grandparentKey: string | null;
  grandparentGuid: string | null;
  grandparentTitle: string | null;
  grandparentThumb: string | null;
  grandparentArt: string | null;
  grandparentTheme: string | null;
  leafCount: number | null;
  viewedLeafCount: number | null;
  childCount: number | null;
  seasonCount: number | null;
  albumCount: number | null;
  librarySectionID: number | null;
  librarySectionTitle: string | null;
  librarySectionKey: string | null;
  playlistType: string | null;
  smart: boolean;
  playlistItemID: number | null;
  playQueueItemID: number | null;
  minYear: string | null;
  maxYear: string | null;
  collectionMode: number | null;
  historyKey: string | null;
  viewedAt: number | null;
  accountID: number | null;
  deviceID: number | null;
  watchlistedAt: number | null;
  sessionKey: string | null;
  User: SessionUser | null;
  Player: Player | null;
  Session: SessionInfo | null;
  TranscodeSession: TranscodeSession | null;
  Media: Media[];
  Genre: Tag[];
  Director: Tag[];
  Writer: Tag[];
  Producer: Tag[];
  Country: Tag[];
  Collection: Tag[];
  Label: Tag[];
  Mood: Tag[];
  Style: Tag[];
  Similar: Tag[];
  Role: Tag[];
  Guid: GuidRef[];
  Marker: Marker[];
  Chapter: Chapter[];
  Image: Image[];
  Rating: RatingSource[];
  Review: Review[];
  UltraBlurColors: UltraBlurColors | null;
  Extras: Nested | null;
  Related: Nested | null;
  OnDeck: Nested | null;
  Children: Nested | null;
  /** Radio stations (`includeStations=1`); each is a playlist-like item whose key feeds a play queue. */
  Stations: Metadata[];
}

const METADATA_KEYS = new Set(["ratingKey", "key", "guid", "type", "subtype", "title", "titleSort", "originalTitle", "editionTitle", "studio", "contentRating", "summary", "tagline", "year", "index", "parentIndex", "rating", "audienceRating", "userRating", "ratingImage", "audienceRatingImage", "viewCount", "skipCount", "viewOffset", "lastViewedAt", "lastRatedAt", "addedAt", "updatedAt", "originallyAvailableAt", "duration", "thumb", "art", "banner", "theme", "composite", "chapterSource", "primaryExtraKey", "extraType", "parentRatingKey", "parentKey", "parentGuid", "parentTitle", "parentThumb", "parentArt", "parentTheme", "parentYear", "parentStudio", "grandparentRatingKey", "grandparentKey", "grandparentGuid", "grandparentTitle", "grandparentThumb", "grandparentArt", "grandparentTheme", "leafCount", "viewedLeafCount", "childCount", "seasonCount", "albumCount", "librarySectionID", "librarySectionTitle", "librarySectionKey", "playlistType", "smart", "playlistItemID", "playQueueItemID", "minYear", "maxYear", "collectionMode", "historyKey", "viewedAt", "accountID", "deviceID", "watchlistedAt", "sessionKey", "User", "Player", "Session", "TranscodeSession", "Media", "Genre", "Director", "Writer", "Producer", "Country", "Collection", "Label", "Mood", "Style", "Similar", "Role", "Guid", "Marker", "Chapter", "Image", "Rating", "Review", "UltraBlurColors", "Extras", "Related", "OnDeck", "Children", "Stations"]);

export function normalizeMetadata(v: unknown): Metadata {
  const o = obj(v, "Metadata");
  return {
    ratingKey: optString(o.ratingKey),
    key: optString(o.key),
    guid: optString(o.guid),
    type: optString(o.type),
    subtype: optString(o.subtype),
    title: optString(o.title),
    titleSort: optString(o.titleSort),
    originalTitle: optString(o.originalTitle),
    editionTitle: optString(o.editionTitle),
    studio: optString(o.studio),
    contentRating: optString(o.contentRating),
    summary: optString(o.summary),
    tagline: optString(o.tagline),
    year: optI64(o.year),
    index: optI64(o.index),
    parentIndex: optI64(o.parentIndex),
    rating: optF64(o.rating),
    audienceRating: optF64(o.audienceRating),
    userRating: optF64(o.userRating),
    ratingImage: optString(o.ratingImage),
    audienceRatingImage: optString(o.audienceRatingImage),
    viewCount: optI64(o.viewCount),
    skipCount: optI64(o.skipCount),
    viewOffset: optI64(o.viewOffset),
    lastViewedAt: optI64(o.lastViewedAt),
    lastRatedAt: optI64(o.lastRatedAt),
    addedAt: optI64(o.addedAt),
    updatedAt: optI64(o.updatedAt),
    originallyAvailableAt: optString(o.originallyAvailableAt),
    duration: optI64(o.duration),
    thumb: optString(o.thumb),
    art: optString(o.art),
    banner: optString(o.banner),
    theme: optString(o.theme),
    composite: optString(o.composite),
    chapterSource: optString(o.chapterSource),
    primaryExtraKey: optString(o.primaryExtraKey),
    extraType: optI64(o.extraType),
    parentRatingKey: optString(o.parentRatingKey),
    parentKey: optString(o.parentKey),
    parentGuid: optString(o.parentGuid),
    parentTitle: optString(o.parentTitle),
    parentThumb: optString(o.parentThumb),
    parentArt: optString(o.parentArt),
    parentTheme: optString(o.parentTheme),
    parentYear: optI64(o.parentYear),
    parentStudio: optString(o.parentStudio),
    grandparentRatingKey: optString(o.grandparentRatingKey),
    grandparentKey: optString(o.grandparentKey),
    grandparentGuid: optString(o.grandparentGuid),
    grandparentTitle: optString(o.grandparentTitle),
    grandparentThumb: optString(o.grandparentThumb),
    grandparentArt: optString(o.grandparentArt),
    grandparentTheme: optString(o.grandparentTheme),
    leafCount: optI64(o.leafCount),
    viewedLeafCount: optI64(o.viewedLeafCount),
    childCount: optI64(o.childCount),
    seasonCount: optI64(o.seasonCount),
    albumCount: optI64(o.albumCount),
    librarySectionID: optI64(o.librarySectionID),
    librarySectionTitle: optString(o.librarySectionTitle),
    librarySectionKey: optString(o.librarySectionKey),
    playlistType: optString(o.playlistType),
    smart: boolean(o.smart),
    playlistItemID: optI64(o.playlistItemID),
    playQueueItemID: optI64(o.playQueueItemID),
    minYear: optString(o.minYear),
    maxYear: optString(o.maxYear),
    collectionMode: optI64(o.collectionMode),
    historyKey: optString(o.historyKey),
    viewedAt: optI64(o.viewedAt),
    accountID: optI64(o.accountID),
    deviceID: optI64(o.deviceID),
    watchlistedAt: optI64(o.watchlistedAt),
    sessionKey: optString(o.sessionKey),
    User: optional(o.User, normalizeSessionUser, "User"),
    Player: optional(o.Player, normalizePlayer, "Player"),
    Session: optional(o.Session, normalizeSessionInfo, "Session"),
    TranscodeSession: optional(o.TranscodeSession, normalizeTranscodeSession, "TranscodeSession"),
    Media: list(o.Media, normalizeMedia, "Media"),
    Genre: list(o.Genre, normalizeTag, "Genre"),
    Director: list(o.Director, normalizeTag, "Director"),
    Writer: list(o.Writer, normalizeTag, "Writer"),
    Producer: list(o.Producer, normalizeTag, "Producer"),
    Country: list(o.Country, normalizeTag, "Country"),
    Collection: list(o.Collection, normalizeTag, "Collection"),
    Label: list(o.Label, normalizeTag, "Label"),
    Mood: list(o.Mood, normalizeTag, "Mood"),
    Style: list(o.Style, normalizeTag, "Style"),
    Similar: list(o.Similar, normalizeTag, "Similar"),
    Role: list(o.Role, normalizeTag, "Role"),
    Guid: list(o.Guid, normalizeGuidRef, "Guid"),
    Marker: list(o.Marker, normalizeMarker, "Marker"),
    Chapter: list(o.Chapter, normalizeChapter, "Chapter"),
    Image: list(o.Image, normalizeImage, "Image"),
    Rating: list(o.Rating, normalizeRatingSource, "Rating"),
    Review: list(o.Review, normalizeReview, "Review"),
    UltraBlurColors: optional(o.UltraBlurColors, normalizeUltraBlurColors, "UltraBlurColors"),
    Extras: optional(o.Extras, normalizeNested, "Extras"),
    Related: optional(o.Related, normalizeNested, "Related"),
    OnDeck: optional(o.OnDeck, normalizeNested, "OnDeck"),
    Children: optional(o.Children, normalizeNested, "Children"),
    Stations: oneOrMany(o.Stations, normalizeMetadata, "Stations"),
    ...extraOf(o, METADATA_KEYS),
  };
}

/** `Metadata::default()`, with fields overridden by `over`. */
export function defaultMetadata(over: Partial<Metadata> = {}): Metadata {
  return { ...normalizeMetadata({}), ...over };
}

/**
 * Nested `{ "Metadata": [...], "Hub": [...] }` blocks (Extras, Related, OnDeck).
 */
export interface Nested {
  size: number | null;
  Metadata: Metadata[];
  Hub: Hub[];
}

export function normalizeNested(v: unknown): Nested {
  const o = obj(v, "Nested");
  return {
    size: optI64(o.size),
    Metadata: oneOrMany(o.Metadata, normalizeMetadata, "Metadata"),
    Hub: oneOrMany(o.Hub, normalizeHub, "Hub"),
  };
}

/** `Nested::default()`, with fields overridden by `over`. */
export function defaultNested(over: Partial<Nested> = {}): Nested {
  return { ...normalizeNested({}), ...over };
}

/**
 * A tag reference: genre, director, actor (role), collection, label, ...
 */
export interface Tag {
  id: number | null;
  tag: string | null;
  tagKey: string | null;
  filter: string | null;
  role: string | null;
  thumb: string | null;
  count: number | null;
}

export function normalizeTag(v: unknown): Tag {
  const o = obj(v, "Tag");
  return {
    id: optI64(o.id),
    tag: optString(o.tag),
    tagKey: optString(o.tagKey),
    filter: optString(o.filter),
    role: optString(o.role),
    thumb: optString(o.thumb),
    count: optI64(o.count),
  };
}

/** `Tag::default()`, with fields overridden by `over`. */
export function defaultTag(over: Partial<Tag> = {}): Tag {
  return { ...normalizeTag({}), ...over };
}

export interface GuidRef {
  id: string | null;
}

export function normalizeGuidRef(v: unknown): GuidRef {
  const o = obj(v, "GuidRef");
  return {
    id: optString(o.id),
  };
}

/** `GuidRef::default()`, with fields overridden by `over`. */
export function defaultGuidRef(over: Partial<GuidRef> = {}): GuidRef {
  return { ...normalizeGuidRef({}), ...over };
}

/**
 * Intro / credits / commercial markers.
 */
export interface Marker {
  id: number | null;
  type: string | null;
  startTimeOffset: number | null;
  endTimeOffset: number | null;
  final: boolean;
}

export function normalizeMarker(v: unknown): Marker {
  const o = obj(v, "Marker");
  return {
    id: optI64(o.id),
    type: optString(o.type),
    startTimeOffset: optI64(o.startTimeOffset),
    endTimeOffset: optI64(o.endTimeOffset),
    final: boolean(o.final),
  };
}

/** `Marker::default()`, with fields overridden by `over`. */
export function defaultMarker(over: Partial<Marker> = {}): Marker {
  return { ...normalizeMarker({}), ...over };
}

export interface Chapter {
  id: number | null;
  index: number | null;
  tag: string | null;
  startTimeOffset: number | null;
  endTimeOffset: number | null;
  thumb: string | null;
}

export function normalizeChapter(v: unknown): Chapter {
  const o = obj(v, "Chapter");
  return {
    id: optI64(o.id),
    index: optI64(o.index),
    tag: optString(o.tag),
    startTimeOffset: optI64(o.startTimeOffset),
    endTimeOffset: optI64(o.endTimeOffset),
    thumb: optString(o.thumb),
  };
}

/** `Chapter::default()`, with fields overridden by `over`. */
export function defaultChapter(over: Partial<Chapter> = {}): Chapter {
  return { ...normalizeChapter({}), ...over };
}

export interface Image {
  alt: string | null;
  type: string | null;
  url: string | null;
}

export function normalizeImage(v: unknown): Image {
  const o = obj(v, "Image");
  return {
    alt: optString(o.alt),
    type: optString(o.type),
    url: optString(o.url),
  };
}

/** `Image::default()`, with fields overridden by `over`. */
export function defaultImage(over: Partial<Image> = {}): Image {
  return { ...normalizeImage({}), ...over };
}

export interface RatingSource {
  image: string | null;
  value: number | null;
  type: string | null;
}

export function normalizeRatingSource(v: unknown): RatingSource {
  const o = obj(v, "RatingSource");
  return {
    image: optString(o.image),
    value: optF64(o.value),
    type: optString(o.type),
  };
}

/** `RatingSource::default()`, with fields overridden by `over`. */
export function defaultRatingSource(over: Partial<RatingSource> = {}): RatingSource {
  return { ...normalizeRatingSource({}), ...over };
}

export interface Review {
  tag: string | null;
  text: string | null;
  image: string | null;
  source: string | null;
  link: string | null;
}

export function normalizeReview(v: unknown): Review {
  const o = obj(v, "Review");
  return {
    tag: optString(o.tag),
    text: optString(o.text),
    image: optString(o.image),
    source: optString(o.source),
    link: optString(o.link),
  };
}

/** `Review::default()`, with fields overridden by `over`. */
export function defaultReview(over: Partial<Review> = {}): Review {
  return { ...normalizeReview({}), ...over };
}

/**
 * Four corner colours (hex, no `#`) for Plex's "ultra blur" backgrounds.
 */
export interface UltraBlurColors {
  topLeft: string | null;
  topRight: string | null;
  bottomRight: string | null;
  bottomLeft: string | null;
}

export function normalizeUltraBlurColors(v: unknown): UltraBlurColors {
  const o = obj(v, "UltraBlurColors");
  return {
    topLeft: optString(o.topLeft),
    topRight: optString(o.topRight),
    bottomRight: optString(o.bottomRight),
    bottomLeft: optString(o.bottomLeft),
  };
}

/** `UltraBlurColors::default()`, with fields overridden by `over`. */
export function defaultUltraBlurColors(over: Partial<UltraBlurColors> = {}): UltraBlurColors {
  return { ...normalizeUltraBlurColors({}), ...over };
}

/**
 * Match candidate from `/library/metadata/{id}/matches`.
 */
export interface MatchResult {
  guid: string | null;
  name: string | null;
  score: number | null;
  year: number | null;
  thumb: string | null;
  summary: string | null;
  matched: boolean;
}

export function normalizeMatchResult(v: unknown): MatchResult {
  const o = obj(v, "MatchResult");
  return {
    guid: optString(o.guid),
    name: optString(o.name),
    score: optI64(o.score),
    year: optI64(o.year),
    thumb: optString(o.thumb),
    summary: optString(o.summary),
    matched: boolean(o.matched),
  };
}

/** `MatchResult::default()`, with fields overridden by `over`. */
export function defaultMatchResult(over: Partial<MatchResult> = {}): MatchResult {
  return { ...normalizeMatchResult({}), ...over };
}


/** Key under which Patina tags an item with the server it came from (see `Metadata.serverId`). */
export const SERVER_TAG = "patinaServer";

function pad2(n: number): string {
  return String(n).padStart(2, "0");
}

/** `impl Metadata`: pure helpers over the serialized item. */
// eslint-disable-next-line @typescript-eslint/no-namespace
export namespace Metadata {
  /** Attributes Plex sent that are not modelled (serde's flattened `extra` map). */
  export function extra(m: Metadata): Record<string, unknown> {
    return extraOf(m as unknown as Record<string, unknown>, METADATA_KEYS);
  }

  /**
   * The machine identifier of the server this item came from, when it is not the
   * session's own server. Patina sets it on items found on other servers so that
   * navigation, playback and reporting stay with the right one. Plex never sends it.
   */
  export function serverId(m: Metadata): string | null {
    const v = (m as unknown as Record<string, unknown>)[SERVER_TAG];
    return typeof v === "string" && v !== "" ? v : null;
  }

  /** Tag the item with the server it lives on (`null` clears the tag). */
  export function setServerId(m: Metadata, server: string | null): void {
    const rec = m as unknown as Record<string, unknown>;
    if (server !== null) rec[SERVER_TAG] = server;
    else delete rec[SERVER_TAG];
  }

  export function ratingKey(m: Metadata): string {
    return m.ratingKey ?? "";
  }

  export function title(m: Metadata): string {
    return m.title ?? "";
  }

  export function kind(m: Metadata): string {
    return m.type ?? "";
  }

  /** Whether this item can be handed to a player directly. */
  export function isPlayable(m: Metadata): boolean {
    const k = kind(m);
    return (
      ["movie", "episode", "track", "clip", "trailer", "photo"].includes(k) ||
      (m.Media.length > 0 && !["show", "season", "artist", "album"].includes(k))
    );
  }

  /** Whether this item is a container of other items (show, season, album, ...). */
  export function isContainer(m: Metadata): boolean {
    return ["show", "season", "artist", "album", "collection", "playlist", "photoalbum"].includes(kind(m));
  }

  export function isWatched(m: Metadata): boolean {
    if (m.leafCount !== null && m.viewedLeafCount !== null && m.leafCount > 0) {
      return m.viewedLeafCount >= m.leafCount;
    }
    return (m.viewCount ?? 0) > 0 && (m.viewOffset ?? 0) === 0;
  }

  export function unwatchedLeaves(m: Metadata): number | null {
    if (m.leafCount === null) return null;
    return m.leafCount - (m.viewedLeafCount ?? 0);
  }

  /** Resume progress in `0.0..=1.0`, if partially watched. */
  export function progress(m: Metadata): number | null {
    if (m.viewOffset === null || m.duration === null) return null;
    const offset = m.viewOffset;
    const duration = m.duration;
    return duration > 0 && offset > 0 ? Math.min(1, Math.max(0, offset / duration)) : null;
  }

  /** Best available image for a poster-shaped card. */
  export function poster(m: Metadata): string | null {
    switch (kind(m)) {
      case "episode":
        return m.grandparentThumb ?? m.parentThumb ?? m.thumb;
      case "season":
        return m.thumb ?? m.parentThumb;
      case "playlist":
        return m.composite ?? m.thumb;
      default:
        return m.thumb ?? m.composite ?? m.parentThumb;
    }
  }

  /** Best available background art. */
  export function backdrop(m: Metadata): string | null {
    return m.art ?? m.grandparentArt ?? m.parentArt ?? imageOfType(m, "background");
  }

  export function imageOfType(m: Metadata, kind: string): string | null {
    const found = m.Image.find((i) => i.type === kind);
    return found?.url ?? null;
  }

  /** Title including show/season context, e.g. "Show — S01E02 · Pilot". */
  export function fullTitle(m: Metadata): string {
    switch (kind(m)) {
      case "episode":
        return `${m.grandparentTitle ?? ""} — ${episodeCode(m)} · ${title(m)}`;
      case "season":
        return `${m.parentTitle ?? ""} — ${title(m)}`;
      case "track":
        return `${m.grandparentTitle ?? ""} — ${title(m)}`;
      default:
        return m.year !== null ? `${title(m)} (${m.year})` : title(m);
    }
  }

  /** "S01E02" style code for episodes. */
  export function episodeCode(m: Metadata): string {
    if (m.parentIndex !== null && m.index !== null) return `S${pad2(m.parentIndex)}E${pad2(m.index)}`;
    if (m.parentIndex === null && m.index !== null) return `E${pad2(m.index)}`;
    return "";
  }

  /** The plex.tv discover id derived from a `plex://type/<id>` guid. */
  export function discoverId(m: Metadata): string | null {
    const guid = m.guid;
    if (guid === null || !guid.startsWith("plex://")) return null;
    const parts = guid.slice("plex://".length).split("/");
    return parts[parts.length - 1] ?? null;
  }

  export function markersOf(m: Metadata, kind: string): Marker[] {
    return m.Marker.filter((x) => x.type === kind);
  }

  export function extras(m: Metadata): Metadata[] {
    return m.Extras?.Metadata ?? [];
  }

  export function relatedHubs(m: Metadata): Hub[] {
    return m.Related?.Hub ?? [];
  }

  export function onDeckItem(m: Metadata): Metadata | null {
    return m.OnDeck?.Metadata[0] ?? null;
  }
}

export namespace Tag {
  export function name(t: Tag): string {
    return t.tag ?? "";
  }
}
