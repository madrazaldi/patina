// Search across every server of the signed-in account.
//
// Each server is searched on its own connection, a few at a time and with a
// timeout, so a slow or unreachable server never holds the others back. Results
// arrive as a growing snapshot (`Results`): after every server that finishes,
// the same title found on several servers is shown once, listing each copy with
// its quality. Items found on a server other than the current one are tagged
// with it (see `Metadata.setServerId`), which keeps navigation, playback and
// reporting on the server that has the copy.

import { type Media, Metadata, Part, Resource, Stream } from "../../plex";
import { logger } from "../log";
import * as servers from "../servers";
import type { AppState } from "../state";
import { eachCompleted, errorText } from "./util";

const log = logger("xsearch");

/** Servers searched at once. */
export const CONCURRENCY = 4;
/** How long one server gets to connect and answer before it is given up on (milliseconds). */
export const PER_SERVER = 12_000;
const HUB_LIMIT = 24;
const PER_SERVER_ITEMS = 40;

/** One file of a copy: what a person compares when choosing where to watch. */
export interface Version {
  /** "1080p · H264 · MKV · 6.2 Mbps · 2.1 GB". */
  label: string;
  resolution: string | null;
  videoCodec: string | null;
  container: string | null;
  /** Kilobits per second. */
  bitrate: number | null;
  /** Bytes. */
  size: number | null;
  hdr: string | null;
}

/** A title as one server has it. */
export interface ServerCopy {
  serverId: string;
  serverName: string;
  /** On the server this session is connected to. */
  current: boolean;
  library: string | null;
  /** The item, tagged with its server unless it is the current one. */
  item: Metadata;
  versions: Version[];
}

/** One title and every server that has it. */
export interface Group {
  key: string;
  title: string;
  year: number | null;
  kind: string;
  /** The show and episode number for an episode, the artist for an album, ... */
  subtitle: string | null;
  copies: ServerCopy[];
}

export type ServerState = "pending" | "done" | "failed" | "timedOut" | "offline";

export interface ServerStatus {
  id: string;
  name: string;
  current: boolean;
  state: ServerState;
  /** Titles this server contributed. */
  count: number;
  error: string | null;
}

/** A server to search. */
export interface Target {
  id: string;
  name: string;
  current: boolean;
  online: boolean;
}

/** The current server first, then the account's other servers. */
export function targets(app: AppState): Target[] {
  const session = app.session();
  if (!session) return [];
  const out: Target[] = [{ id: session.serverId, name: session.serverName, current: true, online: true }];
  for (const r of session.servers) {
    if (Resource.id(r) !== session.serverId && Resource.isServer(r)) {
      out.push({ id: Resource.id(r), name: Resource.name(r), current: false, online: r.presence });
    }
  }
  return out;
}

/** Everything found so far. */
export class Results {
  query: string;
  groups: Group[] = [];
  servers: ServerStatus[];
  /** Every server has answered, failed or timed out. */
  done = false;

  constructor(query: string, targets: readonly Target[]) {
    this.query = query;
    this.servers = targets.map((t) => ({
      id: t.id,
      name: t.name,
      current: t.current,
      state: t.online ? "pending" : "offline",
      count: 0,
      error: null,
    }));
  }

  /** Fold in what one server found. */
  add(target: Target, items: Metadata[]): void {
    const count = items.length;
    for (const item of items) {
      Metadata.setServerId(item, target.current ? null : target.id);
      const copy: ServerCopy = {
        serverId: target.id,
        serverName: target.name,
        current: target.current,
        library: item.librarySectionTitle,
        versions: item.Media.map(version),
        item,
      };
      insert(this.groups, copy);
    }
    this.finish(target, "done", count, null);
  }

  /** Note that a server could not be searched. */
  fail(target: Target, state: ServerState, error: string): void {
    this.finish(target, state, 0, error);
  }

  private finish(target: Target, state: ServerState, count: number, error: string | null): void {
    const s = this.servers.find((s) => s.id === target.id);
    if (s) {
      s.state = state;
      s.count = count;
      s.error = error;
    }
    this.done = this.servers.every((s) => s.state !== "pending");
  }
}

/**
 * Put a copy into the group of its title, starting a new group when it is the first.
 * The current server's copy is listed first, then the others by server name.
 */
function insert(groups: Group[], copy: ServerCopy): void {
  const key = groupKey(copy.item);
  const group = groups.find((g) => g.key === key);
  if (!group) {
    groups.push({
      key,
      title: Metadata.title(copy.item),
      year: copy.item.year,
      kind: Metadata.kind(copy.item),
      subtitle: subtitle(copy.item),
      copies: [copy],
    });
    return;
  }
  // A server sometimes lists the same item under more than one hub.
  if (group.copies.some((c) => c.serverId === copy.serverId && Metadata.ratingKey(c.item) === Metadata.ratingKey(copy.item))) return;
  group.copies.push(copy);
  group.copies.sort((a, b) => {
    if (a.current !== b.current) return a.current ? -1 : 1;
    const an = a.serverName.toLowerCase();
    const bn = b.serverName.toLowerCase();
    return an < bn ? -1 : an > bn ? 1 : 0;
  });
}

const pad2 = (n: number) => String(n).padStart(2, "0");

/** @internal The show and episode number for an episode, the artist for an album. */
export function subtitle(m: Metadata): string | null {
  switch (Metadata.kind(m)) {
    case "episode": {
      const show = m.grandparentTitle ?? "";
      const number = m.parentIndex !== null && m.index !== null ? `S${pad2(m.parentIndex)}E${pad2(m.index)}` : "";
      const text = `${show} ${number}`.trim();
      return text !== "" ? text : null;
    }
    case "album":
    case "track":
      return m.parentTitle ?? m.grandparentTitle;
    default:
      return null;
  }
}

/**
 * What identifies a title across servers: its Plex guid, or without one (local or
 * legacy matches) the kind, title and year.
 */
export function groupKey(m: Metadata): string {
  const guid = m.guid;
  if (guid !== null && guid !== "" && !guid.startsWith("local://") && !guid.includes("agents.none")) return guid;
  const norm = (s: string) =>
    Array.from(s)
      .filter((c) => /[\p{Alphabetic}\p{N}]/u.test(c))
      .map((c) => c.toLowerCase())
      .join("");
  const kind = Metadata.kind(m);
  if (kind === "episode") {
    return `episode|${norm(m.grandparentTitle ?? "")}|${m.parentIndex ?? -1}|${m.index ?? -1}`;
  }
  return `${kind}|${norm(Metadata.title(m))}|${m.year ?? 0}`;
}

/** A short description of a file: resolution, codec, container, bitrate and size. */
export function version(media: Media): Version {
  const r = media.videoResolution;
  let resolution: string | null = null;
  if (r !== null) {
    if (r === "4k" || r === "4K") resolution = "4K";
    else if (r === "sd" || r === "SD") resolution = "SD";
    else if (/^[0-9]*$/.test(r)) resolution = `${r}p`;
    else resolution = r.toUpperCase();
  }
  const part = media.Part[0];
  const containerRaw = media.container ?? part?.container ?? null;
  const container = containerRaw !== null ? containerRaw.toUpperCase() : null;
  const videoCodec = media.videoCodec !== null ? media.videoCodec.toUpperCase() : null;
  const video = part ? Part.videoStream(part) : null;
  const hdr = video ? Stream.hdrLabel(video) : null;
  const size = part?.size ?? null;
  const bits: string[] = [];
  const edition = media.editionTitle ?? media.title;
  if (edition !== null) bits.push(edition);
  for (const b of [resolution, videoCodec, hdr, container]) if (b !== null) bits.push(b);
  if (media.bitrate !== null && media.bitrate > 0) {
    bits.push(media.bitrate >= 1000 ? `${fixed(media.bitrate / 1000, 1)} Mbps` : `${media.bitrate} kbps`);
  }
  if (size !== null && size > 0) bits.push(sizeLabel(size));
  const label = bits.length === 0 ? "Unknown quality" : bits.join(" · ");
  return { label, resolution, videoCodec, container, bitrate: media.bitrate, size, hdr };
}

/**
 * Rust's `{:.N}` for N of 0 or 1: exact decimal rounding with ties to even
 * (`toFixed` rounds ties up).
 */
function fixed(x: number, digits: 0 | 1): string {
  if (digits === 0) {
    if (Number.isInteger(x * 2) && !Number.isInteger(x)) {
      const lo = Math.floor(x);
      return String(lo % 2 === 0 ? lo : lo + 1);
    }
    return x.toFixed(0);
  }
  // A tie at one decimal is exactly a quarter (x = odd / 4); .25 goes down to .2, .75 up to .8.
  if (Number.isInteger(x * 4) && (x * 4) % 2 !== 0 && x % 1 === 0.25) return (Math.floor(x * 10) / 10).toFixed(1);
  return x.toFixed(1);
}

function sizeLabel(bytes: number): string {
  const GB = 1024 * 1024 * 1024;
  const MB = 1024 * 1024;
  return bytes >= GB ? `${fixed(bytes / GB, 1)} GB` : `${fixed(bytes / MB, 0)} MB`;
}

/** Search one server; the current one when `target.current`. */
async function searchOne(app: AppState, target: Target, query: string): Promise<Metadata[]> {
  const server = await servers.serverFor(app, target.current ? undefined : target.id);
  const hubs = await server.searchHubs(query, HUB_LIMIT, undefined);
  const seen = new Set<string>();
  const out: Metadata[] = [];
  for (const m of hubs.flatMap((h) => h.Metadata)) {
    if (out.length >= PER_SERVER_ITEMS) break;
    if (Metadata.kind(m) === "collection") continue;
    const id = `${Metadata.kind(m)}\0${Metadata.ratingKey(m)}`;
    if (seen.has(id)) continue;
    seen.add(id);
    out.push(m);
  }
  return out;
}

type Outcome = { kind: "items"; items: Metadata[] } | { kind: "error"; error: unknown } | { kind: "timeout" };

/**
 * Search every server of the account. `onUpdate` sees the results after each server
 * that finishes (and once at the start), so a front end can show them as they come.
 */
export async function searchAll(app: AppState, query: string, onUpdate: (r: Results) => void = () => {}): Promise<Results> {
  return searchAllWithin(app, query, PER_SERVER, onUpdate);
}

/** `searchAll` with its own limit (milliseconds) on how long one server may take. */
export async function searchAllWithin(
  app: AppState,
  query: string,
  perServer: number,
  onUpdate: (r: Results) => void = () => {},
): Promise<Results> {
  const all = targets(app);
  const results = new Results(query, all);
  onUpdate(results);
  if (results.servers.every((s) => s.state !== "pending")) {
    results.done = true;
    onUpdate(results);
    return results;
  }
  await eachCompleted(
    all.filter((t) => t.online),
    CONCURRENCY,
    async (target): Promise<Outcome> => {
      // `tokio::time::timeout`: the search itself is left to finish on its own.
      const search = searchOne(app, target, query).then(
        (items): Outcome => ({ kind: "items", items }),
        (error: unknown): Outcome => ({ kind: "error", error }),
      );
      let timer: ReturnType<typeof setTimeout> | undefined;
      const timeout = new Promise<Outcome>((resolve) => {
        timer = setTimeout(() => resolve({ kind: "timeout" }), perServer);
      });
      try {
        return await Promise.race([search, timeout]);
      } finally {
        clearTimeout(timer);
      }
    },
    (outcome, target) => {
      if (outcome.kind === "items") {
        results.add(target, outcome.items);
      } else if (outcome.kind === "error") {
        log.debug(`searching ${target.name} failed: ${errorText(outcome.error)}`);
        if (!target.current) servers.forget(app, target.id);
        results.fail(target, "failed", errorText(outcome.error));
      } else {
        if (!target.current) servers.forget(app, target.id);
        results.fail(target, "timedOut", "Took too long to answer.");
      }
      onUpdate(results);
    },
  );
  results.done = true;
  onUpdate(results);
  return results;
}
