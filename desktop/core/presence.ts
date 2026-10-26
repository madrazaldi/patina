// What's playing as Discord Rich Presence. Front ends feed it every
// now-playing change; it skips updates that change nothing visible.
//
// There is no Discord library for this runtime, so the client speaks Discord's local IPC protocol
// directly (the same one the Rust `discord-rich-presence` crate uses): a unix socket
// `discord-ipc-N` (named pipe `\\.\pipe\discord-ipc-N` on Windows); every frame is an int32 LE
// opcode, an int32 LE payload length and a JSON payload.

import { randomUUID } from "node:crypto";
import { existsSync, statSync } from "node:fs";
import net from "node:net";
import path from "node:path";
import { Metadata } from "../plex";
import type { PlayerSettings } from "./config";
import { logger } from "./log";
import type { NowPlaying } from "./state";

const log = logger("presence");

/** What was last shown, to skip redundant updates. */
export interface Shown {
  key: string;
  paused: boolean;
  /** Where playback should be now if it kept going (unix ms minus position). */
  anchorMs: number;
}

export function shownOf(np: NowPlaying): Shown {
  return { key: `${Metadata.ratingKey(np.item)}:${np.uid}`, paused: np.paused, anchorMs: nowMs() - np.positionMs };
}

/**
 * Whether going from `a` to `b` changes nothing visible. A drift of
 * more than a few seconds means a seek.
 */
export function shownSame(a: Shown | null, b: Shown | null): boolean {
  if (a === null && b === null) return true;
  if (a !== null && b !== null) return a.key === b.key && a.paused === b.paused && Math.abs(a.anchorMs - b.anchorMs) < 3000;
  return false;
}

export function nowMs(): number {
  return Date.now();
}

/** Title, artist-like and album-like lines for an item. */
export function lines(item: Metadata): [string, string | null, string | null] {
  switch (Metadata.kind(item)) {
    case "episode":
      return [Metadata.title(item), item.grandparentTitle, `${item.parentTitle ?? ""} · ${Metadata.episodeCode(item)}`];
    case "track":
      return [Metadata.title(item), item.grandparentTitle, item.parentTitle];
    default:
      return [Metadata.title(item), item.year !== null ? String(item.year) : null, null];
  }
}

let LAST: Shown | null = null;

/** Rich presence contents. */
export interface Presence {
  appId: string;
  details: string;
  state: string | null;
  listening: boolean;
  /** Playing: when it started and will end (unix ms). Paused: `null`. */
  span: [number, number] | null;
}

/** What `update` would send for `np`, or `null` when nothing should be shown. */
export function presenceOf(np: NowPlaying | null, settings: PlayerSettings): Presence | null {
  const appId = settings.discord_app_id.trim();
  if (np === null || !settings.discord || appId === "") return null;
  const [title, artist, album] = lines(np.item);
  let details: string;
  let state: string | null;
  switch (Metadata.kind(np.item)) {
    case "episode":
      details = artist ?? "";
      state = `${Metadata.episodeCode(np.item)} · ${title}`;
      break;
    case "track":
      details = title;
      state = artist === null ? null : album !== null ? `${artist} — ${album}` : artist;
      break;
    default:
      details = title;
      state = artist;
  }
  const start = nowMs() - np.positionMs;
  return {
    appId,
    details,
    state: np.paused ? "Paused" : state,
    listening: Metadata.kind(np.item) === "track",
    span: !np.paused && np.durationMs > 0 ? [start, start + np.durationMs] : null,
  };
}

/** Follow the now-playing state. Cheap when nothing changed. */
export function update(np: NowPlaying | null, settings: PlayerSettings): void {
  const shown = np ? shownOf(np) : null;
  if (shownSame(LAST, shown)) return;
  LAST = shown;
  const presence = presenceOf(np, settings);
  if (!worker.started) {
    if (presence === null) return;
    worker.started = true;
  }
  worker.post(presence);
}

// The Discord connection ---------------------------------------------------------

// Environment keys to search for the Discord pipe.
const ENV_KEYS = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"];

const APP_SUBPATHS = [
  "",
  "app/com.discordapp.Discord/",
  "app/dev.vencord.Vesktop/",
  ".flatpak/com.discordapp.Discord/xdg-run/",
  ".flatpak/dev.vencord.Vesktop/xdg-run/",
  "snap.discord-canary/",
  "snap.discord/",
];

function isDir(p: string): boolean {
  try {
    return statSync(p).isDirectory();
  } catch {
    return false;
  }
}

/** The first `discord-ipc-N` socket found (unix). */
export function findPipe(env: Record<string, string | undefined> = process.env): string | null {
  const snap = env.SNAP !== undefined;
  for (const key of ENV_KEYS) {
    const val = env[key];
    if (val === undefined) continue;
    const base = snap && key === ENV_KEYS[0] ? val.slice(0, Math.max(0, val.lastIndexOf("/"))) : val;
    if (!isDir(base)) continue;
    for (let i = 0; i < 10; i++) {
      for (const sub of APP_SUBPATHS) {
        const candidate = path.join(base, sub, `discord-ipc-${i}`);
        if (existsSync(candidate)) return candidate;
      }
    }
  }
  return null;
}

/** An IPC frame header followed by the payload. */
export function pack(opcode: number, payload: string): Buffer {
  const body = Buffer.from(payload, "utf8");
  const header = Buffer.alloc(8);
  header.writeUInt32LE(opcode, 0);
  header.writeUInt32LE(body.length, 4);
  return Buffer.concat([header, body]);
}

const OP_HANDSHAKE = 0;
const OP_FRAME = 1;
const OP_CLOSE = 2;
const OP_PING = 3;
const OP_PONG = 4;

function connectPath(p: string): Promise<net.Socket | null> {
  return new Promise((resolve) => {
    const s = net.createConnection({ path: p });
    const fail = () => {
      s.destroy();
      resolve(null);
    };
    s.once("error", fail);
    s.once("connect", () => {
      s.removeListener("error", fail);
      resolve(s);
    });
  });
}

/** A connection to the Discord client (`DiscordIpcClient`). */
export class DiscordIpc {
  private socket: net.Socket | null = null;
  private buf: Buffer = Buffer.alloc(0);
  private frames: { op: number; data: unknown }[] = [];
  private waiting: (() => void) | null = null;
  dead = false;

  constructor(
    readonly clientId: string,
    /** Where to look for the socket; tests point it at their own. */
    private readonly find: () => string[] = defaultCandidates,
  ) {}

  /** Connect to the socket and send the handshake. Throws when Discord is not there. */
  async connect(): Promise<void> {
    let socket: net.Socket | null = null;
    for (const candidate of this.find()) {
      socket = await connectPath(candidate);
      if (socket) break;
    }
    if (!socket) throw new Error("Couldn't find the Discord IPC socket");
    this.socket = socket;
    // A lingering connection never keeps the process alive.
    socket.unref();
    socket.on("data", (chunk: Buffer) => this.onData(chunk));
    const gone = () => {
      this.dead = true;
      this.waiting?.();
    };
    socket.on("close", gone);
    socket.on("error", gone);
    this.send({ v: 1, client_id: this.clientId }, OP_HANDSHAKE);
    // The crate blocks until Discord answers the handshake; wait at most a few seconds here.
    await this.recv(5000);
  }

  private onData(chunk: Buffer): void {
    this.buf = this.buf.length === 0 ? chunk : Buffer.concat([this.buf, chunk]);
    while (this.buf.length >= 8) {
      const op = this.buf.readUInt32LE(0);
      const len = this.buf.readUInt32LE(4);
      if (this.buf.length < 8 + len) break;
      const text = this.buf.subarray(8, 8 + len).toString("utf8");
      this.buf = this.buf.subarray(8 + len);
      let data: unknown = null;
      try {
        data = JSON.parse(text);
      } catch {
        // an unparsable frame is ignored
      }
      if (op === OP_PING) {
        this.send(data, OP_PONG);
        continue;
      }
      this.frames.push({ op, data });
    }
    // Responses to SET_ACTIVITY are not needed; keep only what a handshake waits for.
    if (this.frames.length > 16) this.frames.splice(0, this.frames.length - 16);
    this.waiting?.();
  }

  private async recv(timeoutMs: number): Promise<{ op: number; data: unknown }> {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      const frame = this.frames.shift();
      if (frame) return frame;
      if (this.dead) throw new Error("Discord closed the connection");
      const left = deadline - Date.now();
      if (left <= 0) throw new Error("Discord did not answer");
      await new Promise<void>((resolve) => {
        const timer = setTimeout(() => {
          this.waiting = null;
          resolve();
        }, left);
        this.waiting = () => {
          clearTimeout(timer);
          this.waiting = null;
          resolve();
        };
      });
    }
  }

  private send(data: unknown, opcode: number): void {
    const s = this.socket;
    if (!s || s.destroyed || !s.writable) throw new Error("Not connected");
    s.write(pack(opcode, JSON.stringify(data)));
  }

  /** `SET_ACTIVITY` with `activity` (`null` clears it). */
  setActivity(activity: Record<string, unknown> | null): void {
    this.send({ cmd: "SET_ACTIVITY", args: { pid: process.pid, activity }, nonce: randomUUID() }, OP_FRAME);
  }

  clearActivity(): void {
    this.setActivity(null);
  }

  close(): void {
    const s = this.socket;
    if (!s) return;
    try {
      this.send({}, OP_CLOSE);
    } catch {
      // already gone
    }
    this.socket = null;
    s.end();
    s.destroy();
  }
}

function defaultCandidates(): string[] {
  if (process.platform === "win32") return Array.from({ length: 10 }, (_, i) => `\\\\.\\pipe\\discord-ipc-${i}`);
  const found = findPipe();
  return found ? [found] : [];
}

/** The activity Discord shows for `p`. */
export function activityOf(p: Presence): Record<string, unknown> {
  const activity: Record<string, unknown> = {
    details: p.details,
    // Listening = 2, Watching = 3; status display type 2 shows the details line.
    type: p.listening ? 2 : 3,
    status_display_type: 2,
  };
  if (p.state !== null) activity.state = p.state;
  if (p.span) activity.timestamps = { start: p.span[0], end: p.span[1] };
  return activity;
}

/**
 * Owns the Discord IPC connection; reconnects as needed. Discord may not be running.
 * Updates are handled one at a time, and only the latest one waiting counts.
 */
class Worker {
  started = false;
  private next: { value: Presence | null } | null = null;
  private running = false;
  private client: { appId: string; ipc: DiscordIpc } | null = null;

  post(presence: Presence | null): void {
    // Only the latest state matters.
    this.next = { value: presence };
    if (!this.running) {
      this.running = true;
      void this.run().catch((e) => log.warn(`discord presence stopped: ${e instanceof Error ? e.message : String(e)}`));
    }
  }

  private async run(): Promise<void> {
    try {
      while (this.next) {
        const { value } = this.next;
        this.next = null;
        await this.handle(value);
      }
    } finally {
      this.running = false;
    }
  }

  private dropClient(): void {
    this.client?.ipc.close();
    this.client = null;
  }

  private async handle(p: Presence | null): Promise<void> {
    if (this.client?.ipc.dead) this.client = null;
    if (p === null) {
      try {
        this.client?.ipc.clearActivity();
      } catch {
        // ignore
      }
      return;
    }
    if (this.client === null || this.client.appId !== p.appId) {
      this.dropClient();
      const ipc = new DiscordIpc(p.appId);
      try {
        await ipc.connect();
      } catch (e) {
        log.debug(`discord: ${e instanceof Error ? e.message : String(e)}`);
        ipc.close();
        return;
      }
      this.client = { appId: p.appId, ipc };
    }
    try {
      this.client.ipc.setActivity(activityOf(p));
    } catch {
      // Discord restarted or closed; try again on the next update.
      this.dropClient();
    }
  }
}

const worker = new Worker();

/** Forget what was shown and the Discord connection (tests). */
export function reset(): void {
  LAST = null;
}
