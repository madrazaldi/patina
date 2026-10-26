// Server administration: activity, overview, settings, accounts, libraries,
// statistics and diagnostics.

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { isAbsolute, join } from "node:path";

import {
  type Account,
  type Activity,
  type ButlerTask,
  type Device,
  type Friend,
  MediaContainer,
  Params,
  type Release,
  type ServerDevice,
  Setting,
  type TranscodeSession,
} from "../../plex";
import * as config from "../config";
import * as session from "../session";
import type { AppState } from "../state";
import { lastSegment } from "./lists";
import { mediaTypeId } from "./util";

export interface Tasks {
  activities: Activity[];
  transcodes: TranscodeSession[];
}

/** Background activities and transcoder sessions. */
export async function tasks(app: AppState): Promise<Tasks> {
  const server = app.server();
  const [activities, transcodes] = await Promise.all([
    server.activities().catch((): Activity[] => []),
    server.transcodeSessions().catch((): TranscodeSession[] => []),
  ]);
  return { activities, transcodes };
}

/** Stop a transcoder session by its key. */
export async function killTranscode(app: AppState, key: string): Promise<void> {
  await app.server().killTranscodeSession(lastSegment(key));
}

export interface Overview {
  version: string | null;
  platform: string | null;
  owner: string | null;
  plexPass: boolean;
  machineIdentifier: string | null;
  transcoder: boolean;
  connectedVia: string;
  hostCpu: number;
  plexCpu: number;
  hostMemory: number;
  plexMemory: number;
  release: Release | null;
  canInstall: boolean;
}

export async function serverOverview(app: AppState): Promise<Overview> {
  const server = app.server();
  const [capsResult, update, resources] = await Promise.all([
    server.capabilities().then(
      (v) => ({ ok: true as const, v }),
      (error: unknown) => ({ ok: false as const, error }),
    ),
    server.updateStatus().catch(() => null),
    server.statisticsResources(6).catch(() => []),
  ]);
  if (!capsResult.ok) throw capsResult.error;
  const caps = capsResult.v;
  const latest = resources[resources.length - 1] ?? null;
  const attr = (k: string) => MediaContainer.attr(caps, k);
  const flag = (k: string) => {
    const v = attr(k);
    return v === "true" || v === "1";
  };
  const p = attr("platform");
  const v = attr("platformVersion");
  const platform = p !== null && v !== null ? `${p} ${v}` : p;
  return {
    version: attr("version"),
    platform,
    owner: attr("myPlexUsername"),
    plexPass: flag("myPlexSubscription"),
    machineIdentifier: attr("machineIdentifier"),
    transcoder: flag("transcoderVideo"),
    connectedVia: String(server.baseUrl()),
    hostCpu: latest?.hostCpuUtilization ?? 0,
    plexCpu: latest?.processCpuUtilization ?? 0,
    hostMemory: latest?.hostMemoryUtilization ?? 0,
    plexMemory: latest?.processMemoryUtilization ?? 0,
    canInstall: update?.canInstall ?? false,
    release: update?.Release[0] ?? null,
  };
}

/** A server or library setting. */
export interface Pref {
  id: string;
  label: string;
  summary: string | null;
  kind: string;
  value: string;
  options: [string, string][];
  advanced: boolean;
  /** Plex's group id (`general`, `network`, `butler`, ...); empty for library settings. */
  group: string;
}

/** A scheduled-task setting: the maintenance window or a task's switch. */
export function isButler(p: Pref): boolean {
  return p.group === "butler" || p.id.startsWith("Butler");
}

/** `Pref::from(Setting)`. */
export function prefFrom(s: Setting): Pref {
  return {
    value: Setting.valueString(s),
    options: Setting.enumOptions(s),
    id: s.id ?? "",
    label: s.label ?? s.id ?? "",
    summary: s.summary,
    kind: s.type ?? "",
    advanced: s.advanced,
    group: s.group ?? "",
  };
}

function visible(list: Setting[]): Pref[] {
  return list.filter((s) => !s.hidden).map(prefFrom);
}

/** Server settings that belong together, as Plex groups them. */
export interface PrefGroup {
  id: string;
  label: string;
  prefs: Pref[];
}

/** How a Plex settings group is titled. */
export function groupLabel(id: string): string {
  switch (id) {
    case "":
      return "Other";
    case "dlna":
      return "DLNA";
    case "butler":
      return "Scheduled tasks";
    default: {
      const words = Array.from(id.replace(/[_-]/g, " "));
      const first = words.shift();
      return first === undefined ? "" : first.toUpperCase() + words.join("");
    }
  }
}

/** Group settings in the order Plex lists them, ungrouped ones last. */
export function grouped(prefs: Pref[]): PrefGroup[] {
  const groups: PrefGroup[] = [];
  for (const p of prefs) {
    const g = groups.find((g) => g.id === p.group);
    if (g) g.prefs.push(p);
    else groups.push({ id: p.group, label: groupLabel(p.group), prefs: [p] });
  }
  // Stable: ungrouped (empty id) ones move last.
  return [...groups.filter((g) => g.id !== ""), ...groups.filter((g) => g.id === "")];
}

/**
 * The server's settings by group, without hidden ones. Scheduled-task
 * settings are left out; `maintenance` has them next to their tasks.
 */
export async function settings(app: AppState): Promise<PrefGroup[]> {
  const prefs = visible(await app.server().prefs());
  return grouped(prefs.filter((p) => !isButler(p)));
}

/** A butler task with the setting that turns its scheduled runs on and off. */
export type ScheduledTask = ButlerTask & { pref: Pref | null };

export interface Maintenance {
  /** Butler settings that aren't one task's switch, like the maintenance window. */
  schedule: Pref[];
  tasks: ScheduledTask[];
}

/** The butler's tasks joined with their settings. */
export async function maintenance(app: AppState): Promise<Maintenance> {
  const server = app.server();
  const [tasks, prefs] = await Promise.all([
    server.butlerTasks().then(
      (v) => ({ ok: true as const, v }),
      (error: unknown) => ({ ok: false as const, error }),
    ),
    server.prefs().catch((): Setting[] => []),
  ]);
  if (!tasks.ok) throw tasks.error;
  return joinButler(tasks.v, visible(prefs));
}

/** Plex names a task's switch `ButlerTask<name>`. */
export function joinButler(tasks: ButlerTask[], prefs: Pref[]): Maintenance {
  const schedule = prefs.filter(isButler);
  const out = tasks.map((task): ScheduledTask => {
    const id = `ButlerTask${task.name ?? ""}`;
    const at = schedule.findIndex((p) => p.id === id && p.kind === "bool");
    const pref = at >= 0 ? schedule.splice(at, 1)[0]! : null;
    return { ...task, pref };
  });
  return { schedule, tasks: out };
}

/** A library's settings, without hidden ones. */
export async function libraryPrefs(app: AppState, id: string): Promise<Pref[]> {
  return visible(await app.server().sectionPrefs(id));
}

export interface UsersDevices {
  accounts: Account[];
  devices: Device[];
  clients: ServerDevice[];
  friends: Friend[];
}

export async function usersDevices(app: AppState): Promise<UsersDevices> {
  const server = app.server();
  const tv = app.tv();
  const [accounts, devices, clients, friends] = await Promise.all([
    server.accounts().catch((): Account[] => []),
    server.devices().catch((): Device[] => []),
    server.clients().catch((): ServerDevice[] => []),
    tv ? tv.friends().catch((): Friend[] => []) : Promise.resolve([] as Friend[]),
  ]);
  return { accounts, devices, clients, friends };
}

// Libraries --------------------------------------------------------------------

export interface Choices {
  /** `[identifier, title]`. */
  agents: [string, string][];
  scanners: string[];
}

/** Agents and scanners for a new library of `kind` (`movie`, `show`, `artist`, `photo`). */
export async function libraryChoices(app: AppState, kind: string): Promise<Choices> {
  const server = app.server();
  const id = mediaTypeId(kind) ?? 1;
  const [agents, scanners] = await Promise.all([
    server.agents(id).then(
      (v): MediaContainer | null => v,
      () => null,
    ),
    server.scanners(id).then(
      (v): MediaContainer | null => v,
      () => null,
    ),
  ]);
  const pick = (mc: MediaContainer | null, key: string): unknown[] => {
    const v = mc ? MediaContainer.extra(mc)[key] : undefined;
    if (Array.isArray(v)) return v;
    if (typeof v === "object" && v !== null) return [v];
    return [];
  };
  const s = (v: unknown, k: string): string | null => {
    const x = typeof v === "object" && v !== null ? (v as Record<string, unknown>)[k] : undefined;
    return typeof x === "string" ? x : null;
  };
  const out: Choices = { agents: [], scanners: [] };
  for (const a of pick(agents, "Agent")) {
    const identifier = s(a, "identifier");
    if (identifier !== null) out.agents.push([identifier, s(a, "title") ?? ""]);
  }
  for (const a of pick(scanners, "Scanner")) {
    const name = s(a, "name");
    if (name !== null) out.scanners.push(name);
  }
  return out;
}

export interface LibraryForm {
  name: string;
  kind: string;
  agent: string;
  scanner: string;
  language: string;
  locations: string[];
}

function pushLocations(p: Params, locations: readonly string[]): void {
  for (const l of locations.filter((l) => l.trim() !== "")) p.push("location", l.trim());
}

export async function createLibrary(app: AppState, form: LibraryForm): Promise<void> {
  const p = new Params()
    .set("name", form.name.trim())
    .set("type", form.kind)
    .set("agent", form.agent)
    .set("scanner", form.scanner)
    .set("language", form.language);
  pushLocations(p, form.locations);
  await app.server().createSection(p);
  await session.reloadSections(app);
}

/** Rename a library and/or change its folders. */
export async function editLibrary(app: AppState, id: string, name: string, locations: readonly string[]): Promise<void> {
  const section = app.sections().find((s) => s.key === id);
  const p = new Params()
    .set("name", name.trim())
    .opt("agent", section?.agent ?? undefined)
    .opt("scanner", section?.scanner ?? undefined)
    .opt("language", section?.language ?? undefined);
  pushLocations(p, locations);
  await app.server().editSection(id, p);
  await session.reloadSections(app);
}

export async function deleteLibrary(app: AppState, id: string): Promise<void> {
  await app.server().deleteSection(id);
  await session.reloadSections(app);
}

// Statistics -------------------------------------------------------------------

export interface Bucket {
  at: number;
  lan: number;
  wan: number;
}

export interface Usage {
  name: string;
  bytes: number;
}

export interface Stats {
  /** Bytes streamed per time bucket, oldest first. */
  bandwidth: Bucket[];
  /** Bytes streamed per account, most first. */
  accounts: Usage[];
  /** Bytes streamed per device, most first. */
  devices: Usage[];
  plays: number;
  playSeconds: number;
}

/**
 * Bandwidth and plays since `since` (unix seconds). `timespan` is Plex's
 * bucket size code: 6 for hours, 4 for days.
 */
export async function serverStats(app: AppState, timespan: number, since: number): Promise<Stats> {
  const server = app.server();
  const [bandwidthResult, media] = await Promise.all([
    server.statisticsBandwidth(timespan).then(
      (v) => ({ ok: true as const, v }),
      (error: unknown) => ({ ok: false as const, error }),
    ),
    server.statisticsMedia(timespan).then(
      (m) => m.StatisticsMedia,
      () => [],
    ),
  ]);
  if (!bandwidthResult.ok) throw bandwidthResult.error;
  const bandwidth = bandwidthResult.v;
  const accountNames = new Map<number, string>();
  for (const a of bandwidth.Account) if (a.id !== null && a.name !== null) accountNames.set(a.id, a.name);
  const deviceNames = new Map<number, string>();
  for (const d of bandwidth.Device) if (d.id !== null && d.name !== null) deviceNames.set(d.id, d.name);
  const buckets = new Map<number, Bucket>();
  const byAccount = new Map<number, number>();
  const byDevice = new Map<number, number>();
  for (const b of bandwidth.StatisticsBandwidth.filter((b) => (b.at ?? 0) >= since)) {
    const at = b.at ?? 0;
    const bytes = b.bytes ?? 0;
    let e = buckets.get(at);
    if (!e) {
      e = { at, lan: 0, wan: 0 };
      buckets.set(at, e);
    }
    if (b.lan) e.lan += bytes;
    else e.wan += bytes;
    byAccount.set(b.accountID ?? 0, (byAccount.get(b.accountID ?? 0) ?? 0) + bytes);
    byDevice.set(b.deviceID ?? 0, (byDevice.get(b.deviceID ?? 0) ?? 0) + bytes);
  }
  const rank = (map: Map<number, number>, names: Map<number, string>, unknown: string): Usage[] =>
    [...map]
      .map(([id, bytes]): Usage => ({ name: names.get(id) ?? unknown, bytes }))
      .sort((a, b) => b.bytes - a.bytes)
      .slice(0, 8);
  let plays = 0;
  let playSeconds = 0;
  for (const m of media.filter((m) => (m.at ?? 0) >= since)) {
    plays += m.count ?? 0;
    playSeconds += m.duration ?? 0;
  }
  return {
    bandwidth: [...buckets.values()].sort((a, b) => a.at - b.at),
    accounts: rank(byAccount, accountNames, "Unknown user"),
    devices: rank(byDevice, deviceNames, "Unknown device"),
    plays,
    playSeconds,
  };
}

/** The user's Downloads folder (`directories::UserDirs::download_dir`), when the platform has one. */
function downloadDir(): string | null {
  const home = homedir();
  switch (process.platform) {
    case "darwin":
      return join(home, "Downloads");
    case "win32":
      return join(process.env.USERPROFILE || home, "Downloads");
    default: {
      const xdg = process.env.XDG_CONFIG_HOME;
      const base = xdg && isAbsolute(xdg) ? xdg : join(home, ".config");
      try {
        const text = readFileSync(join(base, "user-dirs.dirs"), "utf8");
        const m = /^\s*XDG_DOWNLOAD_DIR\s*=\s*"?([^"\n]*)"?/m.exec(text);
        if (!m || !m[1]) return null;
        const dir = m[1].replace(/^\$HOME/, home);
        return isAbsolute(dir) ? dir : null;
      } catch {
        return null;
      }
    }
  }
}

/**
 * Save the server's logs (`logs`) or database (`database`) as a zip in the
 * user's Downloads folder (else the downloads folder). Returns the file.
 */
export async function saveDiagnostics(app: AppState, what: string): Promise<string> {
  const server = app.server();
  let bytes: Uint8Array;
  if (what === "logs") bytes = await server.downloadLogs();
  else if (what === "database") bytes = await server.downloadDatabase();
  else throw new Error(`Unknown diagnostics ${what}.`);
  const dir = downloadDir() ?? config.downloadsDir(app.settings().player);
  mkdirSync(dir, { recursive: true });
  const stamp = Math.floor(Date.now() / 1000);
  const path = join(dir, `plex-${what}-${stamp}.zip`);
  writeFileSync(path, bytes);
  return path;
}
