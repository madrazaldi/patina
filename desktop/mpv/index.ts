// Spawn and drive an external mpv process over its JSON IPC socket.
//
// mpv is started without `--no-config` or `--config-dir`, so it loads the
// user's normal configuration directory (`~/.config/mpv` on macOS/Linux,
// `%APPDATA%\mpv` on Windows), including scripts, input.conf and mpv.conf.

import { existsSync, statSync, unlinkSync } from "node:fs";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { logger } from "../core/log";
import { Channel } from "./channel";

export { Channel } from "./channel";

const log = logger("mpv");

/** A JSON value as mpv exchanges it. */
export type Value = unknown;

export type MpvErrorKind = "spawn" | "noSocket" | "closed" | "command" | "io";

/** Errors of this module; `message` is the text Rust's `thiserror` Display gave. */
export class MpvError extends Error {
  constructor(
    readonly kind: MpvErrorKind,
    message: string,
  ) {
    super(message);
    this.name = "MpvError";
  }

  static spawn(program: string, source: string): MpvError {
    return new MpvError("spawn", `could not start mpv (${program}): ${source}`);
  }
  static noSocket(): MpvError {
    return new MpvError("noSocket", "mpv IPC socket did not come up");
  }
  static closed(): MpvError {
    return new MpvError("closed", "mpv exited");
  }
  static command(message: string): MpvError {
    return new MpvError("command", `mpv error: ${message}`);
  }
  static io(message: string): MpvError {
    return new MpvError("io", `io: ${message}`);
  }
}

/** How to launch mpv. */
export interface SpawnOptions {
  /** mpv executable (name on PATH or absolute path). */
  program: string;
  /** Extra command-line arguments appended after ours. */
  extraArgs: string[];
  /** HTTP headers sent with every request mpv makes. */
  httpHeaders: [string, string][];
}

export function defaultSpawnOptions(): SpawnOptions {
  return { program: defaultProgram(), extraArgs: [], httpHeaders: [] };
}

/** Best guess at the mpv binary: PATH, then common install locations. */
export function defaultProgram(): string {
  const candidates =
    process.platform === "darwin"
      ? ["/opt/homebrew/bin/mpv", "/usr/local/bin/mpv", "/Applications/mpv.app/Contents/MacOS/mpv"]
      : process.platform === "win32"
        ? []
        : ["/usr/bin/mpv", "/usr/local/bin/mpv"];
  const found = which("mpv");
  if (found !== null) return found;
  return candidates.find((p) => existsSync(p)) ?? "mpv";
}

function which(name: string): string | null {
  const dirs = process.env.PATH;
  if (dirs === undefined) return null;
  const exe = process.platform === "win32" ? `${name}.exe` : name;
  for (const dir of dirs.split(path.delimiter)) {
    const candidate = path.join(dir, exe);
    try {
      if (statSync(candidate).isFile()) return candidate;
    } catch {
      // not there
    }
  }
  return null;
}

/** Check that mpv can be executed and return its version line. */
export async function probe(program: string): Promise<string> {
  let proc: ReturnType<typeof Bun.spawn>;
  try {
    proc = Bun.spawn([program, "--version"], { stdin: "ignore", stdout: "pipe", stderr: "ignore" });
  } catch (e) {
    throw MpvError.spawn(program, errorText(e));
  }
  const out = await new Response(proc.stdout as ReadableStream<Uint8Array>).text();
  await proc.exited;
  return out.split(/\r?\n/)[0] || "mpv";
}

function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/** Events from mpv. */
export type Event =
  | { kind: "propertyChange"; id: number; name: string; data: Value }
  | { kind: "startFile"; playlistEntryId: number | null }
  | { kind: "fileLoaded" }
  | { kind: "endFile"; reason: string; playlistEntryId: number | null; error: string | null }
  | { kind: "seek" }
  | { kind: "playbackRestart" }
  | { kind: "idle" }
  /** `script-message` broadcasts (including from our own key bindings). */
  | { kind: "clientMessage"; args: string[] }
  | { kind: "shutdown" }
  | { kind: "other"; name: string; raw: Record<string, unknown> };

interface Waiting {
  resolve: (v: Value) => void;
  reject: (e: MpvError) => void;
}

type Child = ReturnType<typeof Bun.spawn>;

// mpv children still alive, so they are not left behind if the process exits without cleaning up
// (the Rust version relied on `kill_on_drop`).
const children = new Set<Child>();
let exitHook = false;
function track(child: Child): void {
  children.add(child);
  void child.exited.then(() => children.delete(child));
  if (!exitHook) {
    exitHook = true;
    process.on("exit", () => {
      for (const c of children) {
        try {
          c.kill();
        } catch {
          // already gone
        }
      }
    });
  }
}

/** A running mpv instance. */
export class Mpv {
  private nextId = 1;
  private readonly pending = new Map<number, Waiting>();

  private constructor(
    private readonly sock: net.Socket,
    private readonly child: Child,
    /** Only unix leaves a socket file to remove; Windows named pipes vanish with mpv. */
    private readonly socket: string,
  ) {}

  /** Launch mpv idle, connect to its IPC socket, and return it with its event stream. */
  static async spawn(opts: SpawnOptions): Promise<[Mpv, Channel<Event>]> {
    const socket = socketPath();
    const args = ["--idle=yes", `--input-ipc-server=${socket}`, "--msg-level=all=warn"];
    if (opts.httpHeaders.length > 0) {
      const headers = opts.httpHeaders.map(([k, v]) => `${k}: ${v}`.replaceAll(",", "\\,"));
      args.push(`--http-header-fields=${headers.join(",")}`);
    }
    args.push(...opts.extraArgs);
    let child: Child;
    try {
      child = Bun.spawn([opts.program, ...args], { stdin: "ignore", stdout: "ignore", stderr: "pipe" });
    } catch (e) {
      throw MpvError.spawn(opts.program, errorText(e));
    }
    track(child);

    void pumpStderr(child).catch(() => {});

    let sock: net.Socket;
    try {
      sock = await connect(socket, child);
    } catch (e) {
      try {
        child.kill();
      } catch {
        // already gone
      }
      throw e;
    }
    const events = new Channel<Event>();
    const mpv = new Mpv(sock, child, socket);
    mpv.read(events);
    return [mpv, events];
  }

  private read(events: Channel<Event>): void {
    let buf: Buffer = Buffer.alloc(0);
    const onLine = (line: string) => {
      let v: unknown;
      try {
        v = JSON.parse(line);
      } catch {
        return;
      }
      if (typeof v !== "object" || v === null) return;
      const obj = v as Record<string, unknown>;
      const id = obj.request_id;
      if (typeof id === "number" && Number.isInteger(id) && id >= 0) {
        const waiting = this.pending.get(id);
        if (waiting) {
          this.pending.delete(id);
          const err = obj.error;
          if (err === undefined || err === "success" || typeof err !== "string") {
            waiting.resolve("data" in obj ? obj.data : null);
          } else {
            waiting.reject(MpvError.command(err));
          }
        }
        return;
      }
      const event = parseEvent(obj);
      if (event) events.send(event);
    };
    let ended = false;
    const end = () => {
      if (ended) return;
      ended = true;
      events.send({ kind: "shutdown" });
      events.close();
      // Fail anything still waiting.
      const waiting = [...this.pending.values()];
      this.pending.clear();
      for (const w of waiting) w.reject(MpvError.closed());
      this.removeSocket();
    };
    this.sock.on("data", (chunk: Buffer) => {
      buf = buf.length === 0 ? chunk : Buffer.concat([buf, chunk]);
      for (;;) {
        const nl = buf.indexOf(0x0a);
        if (nl < 0) break;
        const line = buf.subarray(0, nl).toString("utf8");
        buf = buf.subarray(nl + 1);
        onLine(line);
      }
    });
    this.sock.on("end", end);
    this.sock.on("close", end);
    this.sock.on("error", end);
  }

  private removeSocket(): void {
    if (process.platform === "win32") return;
    try {
      unlinkSync(this.socket);
    } catch {
      // already removed
    }
  }

  /** Run an mpv command (`["loadfile", url, "replace"]`, ...). */
  command(args: Value[]): Promise<Value> {
    const id = this.nextId++;
    return new Promise<Value>((resolve, reject) => {
      if (this.sock.destroyed || !this.sock.writable) {
        reject(MpvError.closed());
        return;
      }
      const timer = setTimeout(() => {
        if (this.pending.delete(id)) reject(MpvError.command("timeout"));
      }, 15_000);
      this.pending.set(id, {
        resolve: (v) => {
          clearTimeout(timer);
          resolve(v);
        },
        reject: (e) => {
          clearTimeout(timer);
          reject(e);
        },
      });
      const line = `${JSON.stringify({ command: args, request_id: id })}\n`;
      try {
        this.sock.write(line, (err) => {
          if (err && this.pending.delete(id)) {
            clearTimeout(timer);
            reject(MpvError.closed());
          }
        });
      } catch {
        this.pending.delete(id);
        clearTimeout(timer);
        reject(MpvError.closed());
      }
    });
  }

  getProperty(name: string): Promise<Value> {
    return this.command(["get_property", name]);
  }

  async setProperty(name: string, value: Value): Promise<void> {
    await this.command(["set_property", name, value]);
  }

  async observeProperty(id: number, name: string): Promise<void> {
    await this.command(["observe_property", id, name]);
  }

  /**
   * `loadfile` with per-file options (`start`, `force-media-title`, ...).
   * `mode` is `replace`, `append`, `append-play`, `insert-next`, ...
   */
  loadfile(url: string, mode: string, options: [string, string][] = []): Promise<Value> {
    return this.command(["loadfile", url, mode, -1, optionList(options)]);
  }

  /** Insert a file into the playlist before entry `index` (mpv 0.38+). */
  loadfileAt(url: string, index: number, options: [string, string][] = []): Promise<Value> {
    return this.command(["loadfile", url, "insert-at", index, optionList(options)]);
  }

  /** Move playlist entry `from` so it sits before entry `to` (`to` may be the playlist length). */
  async playlistMove(from: number, to: number): Promise<void> {
    await this.command(["playlist-move", from, to]);
  }

  async playlistRemove(index: number): Promise<void> {
    await this.command(["playlist-remove", index]);
  }

  /** Add an external subtitle track. `flags`: `select`, `auto` or `cached`. */
  async subAdd(url: string, flags: string, title: string, lang: string): Promise<void> {
    await this.command(["sub-add", url, flags, title, lang]);
  }

  async seekAbsolute(secs: number): Promise<void> {
    await this.command(["seek", secs, "absolute"]);
  }

  async showText(text: string, durationMs: number): Promise<void> {
    await this.command(["show-text", text, durationMs]);
  }

  /** Bind a key to broadcast `script-message <message>`; arrives as a `clientMessage` event. */
  async bindMessage(key: string, message: string): Promise<void> {
    await this.command(["keybind", key, `script-message ${message}`]);
  }

  async playlistNext(): Promise<void> {
    await this.command(["playlist-next", "force"]);
  }

  async playlistPrev(): Promise<void> {
    await this.command(["playlist-prev"]);
  }

  setPause(paused: boolean): Promise<void> {
    return this.setProperty("pause", paused);
  }

  async quit(): Promise<void> {
    try {
      await this.command(["quit"]);
    } catch (e) {
      if (e instanceof MpvError && e.kind === "closed") return;
      throw e;
    }
  }

  /** Wait for the process to exit (killing it after `graceMs`). */
  async wait(graceMs: number): Promise<void> {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const timeout = new Promise<"timeout">((resolve) => {
      timer = setTimeout(() => resolve("timeout"), graceMs);
    });
    const result = await Promise.race([this.child.exited.then(() => "exited" as const), timeout]);
    clearTimeout(timer);
    if (result === "timeout") {
      try {
        this.child.kill();
      } catch {
        // already gone
      }
      await this.child.exited;
    }
    this.removeSocket();
  }

  /** Whether `this` and `other` are the same instance (Rust: `Arc::ptr_eq`). */
  is(other: Mpv | null | undefined): boolean {
    return this === other;
  }
}

/** mpv's `%len%value` quoting for option lists, so commas and quotes are safe. */
export function quoteOption(v: string): string {
  return `%${Buffer.byteLength(v, "utf8")}%${v}`;
}

function optionList(options: [string, string][]): string {
  return options.map(([k, v]) => `${k}=${quoteOption(v)}`).join(",");
}

function str(v: unknown): string | undefined {
  return typeof v === "string" ? v : undefined;
}

function int(v: unknown): number | null {
  return typeof v === "number" && Number.isInteger(v) ? v : null;
}

export function parseEvent(v: Record<string, unknown>): Event | null {
  const name = str(v.event);
  if (name === undefined) return null;
  const entry = () => int(v.playlist_entry_id);
  switch (name) {
    case "property-change": {
      const id = v.id;
      return {
        kind: "propertyChange",
        id: typeof id === "number" && Number.isInteger(id) && id >= 0 ? id : 0,
        name: str(v.name) ?? "",
        data: "data" in v ? v.data : null,
      };
    }
    case "start-file":
      return { kind: "startFile", playlistEntryId: entry() };
    case "file-loaded":
      return { kind: "fileLoaded" };
    case "end-file":
      return { kind: "endFile", reason: str(v.reason) ?? "", playlistEntryId: entry(), error: str(v.file_error) ?? null };
    case "seek":
      return { kind: "seek" };
    case "playback-restart":
      return { kind: "playbackRestart" };
    case "idle":
      return { kind: "idle" };
    case "client-message":
      return {
        kind: "clientMessage",
        args: Array.isArray(v.args) ? v.args.filter((x): x is string => typeof x === "string") : [],
      };
    case "shutdown":
      return { kind: "shutdown" };
    default:
      return { kind: "other", name, raw: v };
  }
}

function socketPath(): string {
  const nonce = `${Date.now()}${String(process.hrtime.bigint() % 1_000_000n).padStart(6, "0")}`;
  const name = `patina-mpv-${process.pid}-${nonce}`;
  return process.platform === "win32" ? `\\\\.\\pipe\\${name}` : path.join(os.tmpdir(), `${name}.sock`);
}

async function pumpStderr(child: Child): Promise<void> {
  const stream = child.stderr as ReadableStream<Uint8Array> | undefined;
  if (!stream) return;
  const decoder = new TextDecoder();
  let rest = "";
  for await (const chunk of stream) {
    rest += decoder.decode(chunk, { stream: true });
    const lines = rest.split("\n");
    rest = lines.pop() ?? "";
    for (const line of lines) log.info(line.replace(/\r$/, ""));
  }
  if (rest) log.info(rest);
}

function tryConnect(socket: string): Promise<net.Socket | null> {
  return new Promise((resolve) => {
    const s = net.createConnection({ path: socket });
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

async function connect(socket: string, child: Child): Promise<net.Socket> {
  for (let i = 0; i < 100; i++) {
    if (child.exitCode !== null) throw MpvError.closed();
    const s = await tryConnect(socket);
    if (s) return s;
    await Bun.sleep(100);
  }
  throw MpvError.noSocket();
}
