// MediaContainer, hubs and directories (port of crates/plex/src/models/container.rs).

import { DecodeFailure, boolean, extraOf, list, obj, optI64, optString, optional } from "../de";
import { type MatchResult, Metadata, normalizeMatchResult, normalizeMetadata } from "./metadata";
import { type Account, type Activity, type Device, type Release, type ServerDevice, Setting, type StatisticsBandwidth, type StatisticsMedia, type StatisticsResources, type TranscodeSession, normalizeAccount, normalizeActivity, normalizeDevice, normalizeRelease, normalizeServerDevice, normalizeSetting, normalizeStatisticsBandwidth, normalizeStatisticsMedia, normalizeStatisticsResources, normalizeTranscodeSession } from "./server";

/**
 * The generic container almost every server endpoint returns. Each endpoint
 * populates a different subset of the collections.
 */
export interface MediaContainer {
  size: number | null;
  totalSize: number | null;
  offset: number | null;
  title1: string | null;
  title2: string | null;
  identifier: string | null;
  key: string | null;
  art: string | null;
  thumb: string | null;
  viewGroup: string | null;
  librarySectionID: number | null;
  librarySectionTitle: string | null;
  librarySectionUUID: string | null;
  machineIdentifier: string | null;
  playQueueID: number | null;
  playQueueSelectedItemID: number | null;
  playQueueSelectedItemOffset: number | null;
  playQueueSelectedMetadataItemID: string | null;
  playQueueShuffled: boolean;
  playQueueSourceURI: string | null;
  playQueueTotalCount: number | null;
  playQueueVersion: number | null;
  canInstall: boolean;
  checkedAt: number | null;
  downloadURL: string | null;
  status: string | null;
  Metadata: Metadata[];
  Directory: Directory[];
  Hub: Hub[];
  Setting: Setting[];
  Activity: Activity[];
  Account: Account[];
  Device: Device[];
  Server: ServerDevice[];
  TranscodeSession: TranscodeSession[];
  StatisticsMedia: StatisticsMedia[];
  StatisticsResources: StatisticsResources[];
  StatisticsBandwidth: StatisticsBandwidth[];
  Release: Release[];
  SearchResult: MatchResult[];
  Meta: LibraryMeta | null;
}

const MEDIACONTAINER_KEYS = new Set(["size", "totalSize", "offset", "title1", "title2", "identifier", "key", "art", "thumb", "viewGroup", "librarySectionID", "librarySectionTitle", "librarySectionUUID", "machineIdentifier", "playQueueID", "playQueueSelectedItemID", "playQueueSelectedItemOffset", "playQueueSelectedMetadataItemID", "playQueueShuffled", "playQueueSourceURI", "playQueueTotalCount", "playQueueVersion", "canInstall", "checkedAt", "downloadURL", "status", "Metadata", "Directory", "Hub", "Setting", "Activity", "Account", "Device", "Server", "TranscodeSession", "StatisticsMedia", "StatisticsResources", "StatisticsBandwidth", "Release", "SearchResult", "Meta"]);

export function normalizeMediaContainer(v: unknown): MediaContainer {
  const o = obj(v, "MediaContainer");
  return {
    size: optI64(o.size),
    totalSize: optI64(o.totalSize),
    offset: optI64(o.offset),
    title1: optString(o.title1),
    title2: optString(o.title2),
    identifier: optString(o.identifier),
    key: optString(o.key),
    art: optString(o.art),
    thumb: optString(o.thumb),
    viewGroup: optString(o.viewGroup),
    librarySectionID: optI64(o.librarySectionID),
    librarySectionTitle: optString(o.librarySectionTitle),
    librarySectionUUID: optString(o.librarySectionUUID),
    machineIdentifier: optString(o.machineIdentifier),
    playQueueID: optI64(o.playQueueID),
    playQueueSelectedItemID: optI64(o.playQueueSelectedItemID),
    playQueueSelectedItemOffset: optI64(o.playQueueSelectedItemOffset),
    playQueueSelectedMetadataItemID: optString(o.playQueueSelectedMetadataItemID),
    playQueueShuffled: boolean(o.playQueueShuffled),
    playQueueSourceURI: optString(o.playQueueSourceURI),
    playQueueTotalCount: optI64(o.playQueueTotalCount),
    playQueueVersion: optI64(o.playQueueVersion),
    canInstall: boolean(o.canInstall),
    checkedAt: optI64(o.checkedAt),
    downloadURL: optString(o.downloadURL),
    status: optString(o.status),
    Metadata: list(o.Metadata, normalizeMetadata, "Metadata"),
    Directory: list(o.Directory, normalizeDirectory, "Directory"),
    Hub: list(o.Hub, normalizeHub, "Hub"),
    Setting: list(o.Setting, normalizeSetting, "Setting"),
    Activity: list(o.Activity, normalizeActivity, "Activity"),
    Account: list(o.Account, normalizeAccount, "Account"),
    Device: list(o.Device, normalizeDevice, "Device"),
    Server: list(o.Server, normalizeServerDevice, "Server"),
    TranscodeSession: list(o.TranscodeSession, normalizeTranscodeSession, "TranscodeSession"),
    StatisticsMedia: list(o.StatisticsMedia, normalizeStatisticsMedia, "StatisticsMedia"),
    StatisticsResources: list(o.StatisticsResources, normalizeStatisticsResources, "StatisticsResources"),
    StatisticsBandwidth: list(o.StatisticsBandwidth, normalizeStatisticsBandwidth, "StatisticsBandwidth"),
    Release: list(o.Release, normalizeRelease, "Release"),
    SearchResult: list(o.SearchResult, normalizeMatchResult, "SearchResult"),
    Meta: optional(o.Meta, normalizeLibraryMeta, "Meta"),
    ...extraOf(o, MEDIACONTAINER_KEYS),
  };
}

/** `MediaContainer::default()`, with fields overridden by `over`. */
export function defaultMediaContainer(over: Partial<MediaContainer> = {}): MediaContainer {
  return { ...normalizeMediaContainer({}), ...over };
}

/**
 * A hub (row) of related content: "Continue Watching", "Recently Added", search groups, ...
 */
export interface Hub {
  hubKey: string | null;
  key: string | null;
  title: string | null;
  type: string | null;
  hubIdentifier: string | null;
  context: string | null;
  size: number | null;
  more: boolean;
  style: string | null;
  promoted: boolean;
  Metadata: Metadata[];
  Directory: Directory[];
}

export function normalizeHub(v: unknown): Hub {
  const o = obj(v, "Hub");
  return {
    hubKey: optString(o.hubKey),
    key: optString(o.key),
    title: optString(o.title),
    type: optString(o.type),
    hubIdentifier: optString(o.hubIdentifier),
    context: optString(o.context),
    size: optI64(o.size),
    more: boolean(o.more),
    style: optString(o.style),
    promoted: boolean(o.promoted),
    Metadata: list(o.Metadata, normalizeMetadata, "Metadata"),
    Directory: list(o.Directory, normalizeDirectory, "Directory"),
  };
}

/** `Hub::default()`, with fields overridden by `over`. */
export function defaultHub(over: Partial<Hub> = {}): Hub {
  return { ...normalizeHub({}), ...over };
}

/**
 * A "directory" entry: library sections, filter values, sorts, tag lists, capabilities.
 */
export interface Directory {
  key: string | null;
  title: string | null;
  type: string | null;
  fastKey: string | null;
  thumb: string | null;
  art: string | null;
  composite: string | null;
  count: number | null;
  /** Item count of a `firstCharacter` entry. */
  size: number | null;
  tag: string | null;
  filter: string | null;
  filterType: string | null;
  default: string | null;
  defaultDirection: string | null;
  descKey: string | null;
  firstCharacterKey: string | null;
  secondary: boolean;
  search: boolean;
  prompt: string | null;
  agent: string | null;
  scanner: string | null;
  language: string | null;
  uuid: string | null;
  updatedAt: number | null;
  createdAt: number | null;
  scannedAt: number | null;
  contentChangedAt: number | null;
  refreshing: boolean;
  hidden: boolean;
  allowSync: boolean;
  Location: Location[];
}

export function normalizeDirectory(v: unknown): Directory {
  const o = obj(v, "Directory");
  return {
    key: optString(o.key),
    title: optString(o.title),
    type: optString(o.type),
    fastKey: optString(o.fastKey),
    thumb: optString(o.thumb),
    art: optString(o.art),
    composite: optString(o.composite),
    count: optI64(o.count),
    size: optI64(o.size),
    tag: optString(o.tag),
    filter: optString(o.filter),
    filterType: optString(o.filterType),
    default: optString(o.default),
    defaultDirection: optString(o.defaultDirection),
    descKey: optString(o.descKey),
    firstCharacterKey: optString(o.firstCharacterKey),
    secondary: boolean(o.secondary),
    search: boolean(o.search),
    prompt: optString(o.prompt),
    agent: optString(o.agent),
    scanner: optString(o.scanner),
    language: optString(o.language),
    uuid: optString(o.uuid),
    updatedAt: optI64(o.updatedAt),
    createdAt: optI64(o.createdAt),
    scannedAt: optI64(o.scannedAt),
    contentChangedAt: optI64(o.contentChangedAt),
    refreshing: boolean(o.refreshing),
    hidden: boolean(o.hidden),
    allowSync: boolean(o.allowSync),
    Location: list(o.Location, normalizeLocation, "Location"),
  };
}

/** `Directory::default()`, with fields overridden by `over`. */
export function defaultDirectory(over: Partial<Directory> = {}): Directory {
  return { ...normalizeDirectory({}), ...over };
}

export interface Location {
  id: number | null;
  path: string | null;
}

export function normalizeLocation(v: unknown): Location {
  const o = obj(v, "Location");
  return {
    id: optI64(o.id),
    path: optString(o.path),
  };
}

/** `Location::default()`, with fields overridden by `over`. */
export function defaultLocation(over: Partial<Location> = {}): Location {
  return { ...normalizeLocation({}), ...over };
}

/**
 * `includeMeta=1` payload describing a library's types, filters, sorts and fields.
 */
export interface LibraryMeta {
  Type: LibraryMetaType[];
  FieldType: FieldType[];
}

export function normalizeLibraryMeta(v: unknown): LibraryMeta {
  const o = obj(v, "LibraryMeta");
  return {
    Type: list(o.Type, normalizeLibraryMetaType, "Type"),
    FieldType: list(o.FieldType, normalizeFieldType, "FieldType"),
  };
}

/** `LibraryMeta::default()`, with fields overridden by `over`. */
export function defaultLibraryMeta(over: Partial<LibraryMeta> = {}): LibraryMeta {
  return { ...normalizeLibraryMeta({}), ...over };
}

export interface LibraryMetaType {
  key: string | null;
  type: string | null;
  title: string | null;
  active: boolean;
  Filter: Directory[];
  Sort: Directory[];
  Field: Field[];
}

export function normalizeLibraryMetaType(v: unknown): LibraryMetaType {
  const o = obj(v, "LibraryMetaType");
  return {
    key: optString(o.key),
    type: optString(o.type),
    title: optString(o.title),
    active: boolean(o.active),
    Filter: list(o.Filter, normalizeDirectory, "Filter"),
    Sort: list(o.Sort, normalizeDirectory, "Sort"),
    Field: list(o.Field, normalizeField, "Field"),
  };
}

/** `LibraryMetaType::default()`, with fields overridden by `over`. */
export function defaultLibraryMetaType(over: Partial<LibraryMetaType> = {}): LibraryMetaType {
  return { ...normalizeLibraryMetaType({}), ...over };
}

export interface Field {
  key: string | null;
  title: string | null;
  type: string | null;
  subType: string | null;
}

export function normalizeField(v: unknown): Field {
  const o = obj(v, "Field");
  return {
    key: optString(o.key),
    title: optString(o.title),
    type: optString(o.type),
    subType: optString(o.subType),
  };
}

/** `Field::default()`, with fields overridden by `over`. */
export function defaultField(over: Partial<Field> = {}): Field {
  return { ...normalizeField({}), ...over };
}

export interface FieldType {
  type: string | null;
  Operator: Operator[];
}

export function normalizeFieldType(v: unknown): FieldType {
  const o = obj(v, "FieldType");
  return {
    type: optString(o.type),
    Operator: list(o.Operator, normalizeOperator, "Operator"),
  };
}

/** `FieldType::default()`, with fields overridden by `over`. */
export function defaultFieldType(over: Partial<FieldType> = {}): FieldType {
  return { ...normalizeFieldType({}), ...over };
}

export interface Operator {
  key: string | null;
  title: string | null;
}

export function normalizeOperator(v: unknown): Operator {
  const o = obj(v, "Operator");
  return {
    key: optString(o.key),
    title: optString(o.title),
  };
}

/** `Operator::default()`, with fields overridden by `over`. */
export function defaultOperator(over: Partial<Operator> = {}): Operator {
  return { ...normalizeOperator({}), ...over };
}


/** Top-level response wrapper: `{"MediaContainer": {...}}`. */
export interface Envelope {
  MediaContainer: MediaContainer;
}

export function normalizeEnvelope(v: unknown): Envelope {
  const o = obj(v, "Envelope");
  const mc = o.MediaContainer;
  try {
    return { MediaContainer: mc === undefined ? normalizeMediaContainer({}) : normalizeMediaContainer(mc) };
  } catch (e) {
    if (e instanceof DecodeFailure) throw e.under("MediaContainer");
    throw e;
  }
}

export namespace MediaContainer {
  /** Attributes Plex sent that are not modelled (serde's flattened `extra` map). */
  export function extra(mc: MediaContainer): Record<string, unknown> {
    return extraOf(mc as unknown as Record<string, unknown>, MEDIACONTAINER_KEYS);
  }

  /** Look up an attribute from `extra` as a string. */
  export function attr(mc: MediaContainer, key: string): string | null {
    if (MEDIACONTAINER_KEYS.has(key)) return null;
    const rec = mc as unknown as Record<string, unknown>;
    if (!Object.hasOwn(rec, key)) return null;
    const v = rec[key];
    if (v === null || v === undefined) return null;
    return typeof v === "string" ? v : JSON.stringify(v);
  }

  /** Whether there are more items after this page. */
  export function hasMore(mc: MediaContainer): boolean {
    if (mc.offset !== null && mc.size !== null && mc.totalSize !== null) {
      return mc.offset + mc.size < mc.totalSize;
    }
    return false;
  }
}

export namespace Hub {
  export function isEmpty(h: Hub): boolean {
    return h.Metadata.length === 0 && h.Directory.length === 0;
  }
}

export namespace Directory {
  export function title(d: Directory): string {
    return d.title ?? d.tag ?? "";
  }
}
