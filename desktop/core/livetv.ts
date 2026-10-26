// Live TV & DVR: a channel guide, tuning, and recordings. The server's
// responses are loosely structured, so they are read defensively from JSON.

import { type Metadata, Params, type PlexServer } from "../plex";
import { metadataFromJson } from "./ops/util";

export interface Program {
  title: string;
  /** Episode title or year. */
  subtitle: string | null;
  guid: string | null;
  summary: string | null;
  thumb: string | null;
  beginsAt: number;
  endsAt: number;
}

export interface Channel {
  dvr: string;
  id: string;
  number: string | null;
  title: string;
  thumb: string | null;
  programs: Program[];
}

export interface Recording {
  id: string;
  title: string;
  subtitle: string | null;
  beginsAt: number | null;
  channel: string | null;
  status: string | null;
}

export interface Rule {
  id: string;
  title: string;
  kind: string | null;
}

export interface Recordings {
  upcoming: Recording[];
  rules: Rule[];
}

type Json = unknown;
type Obj = Record<string, unknown>;

function isObj(v: unknown): v is Obj {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function get(v: Json, key: string): unknown {
  return isObj(v) ? v[key] : undefined;
}

/** `{Key: [...]}` or `{Key: {...}}` as a list. */
function list(v: Json, key: string): Json[] {
  const x = get(v, key);
  if (Array.isArray(x)) return [...x];
  if (isObj(x)) return [x];
  return [];
}

function mc(v: Json): Json {
  return get(v, "MediaContainer") ?? v;
}

function text(v: Json, key: string): string | null {
  const x = get(v, key);
  if (typeof x === "string" && x !== "") return x;
  if (typeof x === "number") return String(x);
  return null;
}

/** `Value::Number::as_i64` / `str::parse::<i64>`. */
function int(v: Json, key: string): number | null {
  const x = get(v, key);
  if (typeof x === "number") return Number.isInteger(x) ? x : null;
  if (typeof x === "string") return /^[+-]?\d+$/.test(x) ? Number(x) : null;
  return null;
}

/** `str::parse::<f64>` for plain decimal numbers. */
function parseF64(s: string): number | null {
  return /^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$/.test(s) ? Number(s) : null;
}

interface Dvr {
  id: string;
  epg: string;
  lineup: string;
  /** Channel keys enabled on the tuners; empty means all. */
  enabled: string[];
}

async function dvrs(server: PlexServer): Promise<Dvr[]> {
  const v = await server.dvrs();
  const out: Dvr[] = [];
  for (const d of list(mc(v), "Dvr")) {
    const enabled = list(d, "Device")
      .flatMap((dev) => list(dev, "ChannelMapping"))
      .filter((m) => {
        const e = get(m, "enabled");
        return e === undefined || e === "1" || e === 1 || e === true;
      })
      .map((m) => text(m, "channelKey"))
      .filter((k): k is string => k !== null);
    const id = text(d, "key");
    const epg = text(d, "epgIdentifier");
    const lineup = text(d, "lineup");
    if (id === null || epg === null || lineup === null) continue;
    out.push({ id, epg, lineup, enabled });
  }
  return out;
}

export async function available(server: PlexServer): Promise<boolean> {
  try {
    return (await dvrs(server)).length > 0;
  } catch {
    return false;
  }
}

/** Channels with what is on from now for `hours`. */
export async function guide(server: PlexServer, hours: number): Promise<Channel[]> {
  const now = Math.floor(Date.now() / 1000);
  const out: Channel[] = [];
  for (const dvr of await dvrs(server)) {
    const epg = `/${dvr.epg.replace(/^\/+/, "")}`;
    const channels = await server.lineupChannels(epg, dvr.lineup);
    const grid: Json = await server.epgGrid(epg, now, now + hours * 3600).catch(() => null);

    const programs = new Map<string, Program[]>();
    for (const p of list(mc(grid), "Metadata")) {
      for (const media of list(p, "Media")) {
        const channel = text(media, "channelIdentifier");
        if (channel === null) continue;
        const beginsAt = int(media, "beginsAt");
        const endsAt = int(media, "endsAt");
        if (beginsAt === null || endsAt === null) continue;
        const episode = get(p, "type") === "episode";
        const entry: Program = {
          title: (episode ? (text(p, "grandparentTitle") ?? text(p, "title")) : text(p, "title")) ?? "",
          subtitle: episode ? text(p, "title") : text(p, "year"),
          guid: text(p, "guid"),
          summary: text(p, "summary"),
          thumb: text(p, "thumb") ?? text(p, "grandparentThumb"),
          beginsAt,
          endsAt,
        };
        const existing = programs.get(channel);
        if (existing) existing.push(entry);
        else programs.set(channel, [entry]);
      }
    }
    for (const c of list(mc(channels), "Channel")) {
      const id = text(c, "identifier");
      if (id === null) continue;
      const key = text(c, "key");
      if (dvr.enabled.length > 0 && key !== null && !dvr.enabled.includes(key)) continue;
      let progs = programs.get(id) ?? [];
      programs.delete(id);
      // Stable sort by start, then drop consecutive duplicates of a start time.
      progs = [...progs].sort((a, b) => a.beginsAt - b.beginsAt);
      progs = progs.filter((p, i) => i === 0 || p.beginsAt !== progs[i - 1]!.beginsAt);
      progs = progs.filter((p) => p.endsAt > now);
      out.push({
        dvr: dvr.id,
        number: text(c, "channelVcn") ?? text(c, "vcn"),
        title: text(c, "title") ?? text(c, "callSign") ?? id,
        thumb: text(c, "thumb"),
        id,
        programs: progs,
      });
    }
  }
  const number = (c: Channel) => {
    const n = c.number === null ? null : parseF64(c.number);
    return n ?? Number.MAX_VALUE;
  };
  out.sort((a, b) => {
    const d = number(a) - number(b);
    if (d !== 0 && !Number.isNaN(d)) return d;
    return a.title < b.title ? -1 : a.title > b.title ? 1 : 0;
  });
  return out;
}

/** Find the first `Metadata` object anywhere in a response. */
function firstMetadata(v: Json): Json | null {
  if (Array.isArray(v)) {
    for (const x of v) {
      const r = firstMetadata(x);
      if (r !== null) return r;
    }
    return null;
  }
  if (isObj(v)) {
    if ("Metadata" in v) {
      const m = v.Metadata;
      return Array.isArray(m) ? (m[0] ?? null) : m;
    }
    for (const x of Object.values(v)) {
      const r = firstMetadata(x);
      if (r !== null) return r;
    }
  }
  return null;
}

/** Tune a channel; returns the live item to hand to the player. */
export async function tune(server: PlexServer, dvr: string, channel: string): Promise<Metadata> {
  const v = await server.tune(dvr, channel);
  const m = firstMetadata(v);
  if (m === null) throw new Error("The tuner did not start a session.");
  const item = metadataFromJson(m);
  if (item.key == null) throw new Error("The tuner did not start a session.");
  return item;
}

export async function recordings(server: PlexServer): Promise<Recordings> {
  const [scheduledResult, subscriptions] = await Promise.all([
    server.scheduledRecordings().then(
      (v) => ({ ok: true as const, v }),
      () => ({ ok: false as const, v: null }),
    ),
    server.subscriptions(),
  ]);
  const scheduled: Json = scheduledResult.ok ? scheduledResult.v : null;
  const upcoming: Recording[] = [];
  for (const g of list(mc(scheduled), "MediaGrabOperation")) {
    const m = get(g, "Metadata");
    if (m === undefined || m === null) continue;
    const media = list(m, "Media")[0];
    const episode = get(m, "type") === "episode";
    const id = text(g, "id") ?? text(g, "mediaSubscriptionID");
    if (id === null) continue;
    upcoming.push({
      id,
      title: (episode ? text(m, "grandparentTitle") : text(m, "title")) ?? "",
      subtitle: episode ? text(m, "title") : null,
      beginsAt: media !== undefined ? int(media, "beginsAt") : null,
      channel: media !== undefined ? (text(media, "channelTitle") ?? text(media, "channelCallSign")) : null,
      status: text(g, "status"),
    });
  }
  const rules: Rule[] = [];
  for (const s of list(mc(subscriptions), "MediaSubscription")) {
    let target: Json | undefined;
    for (const k of ["Directory", "Video", "Metadata"]) {
      target = list(s, k)[0];
      if (target !== undefined) break;
    }
    const id = text(s, "key");
    if (id === null) continue;
    rules.push({
      id,
      title: text(s, "title") ?? (target !== undefined ? text(target, "title") : null) ?? "",
      kind: text(s, "type"),
    });
  }
  return { upcoming, rules };
}

/** Record a guide item with the server's default rule for it. */
export async function record(server: PlexServer, guid: string): Promise<void> {
  const v = await server.subscriptionTemplate(guid);
  const template = list(mc(v), "SubscriptionTemplate").flatMap((t) => list(t, "MediaSubscription"))[0];
  if (template === undefined) throw new Error("This program cannot be recorded.");
  const p = new Params().set("includeGrabs", 1);
  const params = text(template, "parameters");
  if (params !== null) {
    for (const [k, val] of new URLSearchParams(params)) p.push(k, val);
  }
  for (const key of ["type", "targetLibrarySectionID", "targetSectionLocationID"]) {
    const val = text(template, key);
    if (val !== null) p.push(key, val);
  }
  for (const s of list(template, "Setting")) {
    const id = text(s, "id");
    const value = text(s, "value") ?? text(s, "default");
    if (id !== null && value !== null) p.push(`prefs[${id}]`, value);
  }
  await server.createSubscription(p);
}

/** Delete a recording rule by its id or key. */
export async function deleteRule(server: PlexServer, id: string): Promise<void> {
  const last = id.slice(id.lastIndexOf("/") + 1);
  await server.deleteSubscription(last);
}
