// Shared HTTP plumbing for plex.tv and server clients (port of crates/plex/src/client.rs)
// on top of the global `fetch`.

import { spawnSync } from "node:child_process";

import { DecodeFailure } from "./de";
import { PlexError } from "./error";
import { logger } from "../core/log";

const log = logger("plex");

/** Identifies this application to Plex. Every request carries these values as `X-Plex-*` headers. */
export class ClientInfo {
  /** Must be stable per installation. */
  identifier: string;
  product: string;
  version: string;
  platform: string;
  platformVersion: string;
  device: string;
  deviceName: string;

  constructor(identifier: string, product: string, version: string) {
    let platform: string;
    switch (process.platform) {
      case "darwin":
        platform = "macOS";
        break;
      case "win32":
        platform = "Windows";
        break;
      case "linux":
        platform = "Linux";
        break;
      default:
        platform = process.platform;
    }
    this.identifier = identifier;
    this.product = product;
    this.version = version;
    this.platformVersion = "";
    this.device = platform;
    this.platform = platform;
    this.deviceName = hostname() ?? platform;
  }

  headers(): [string, string][] {
    return [
      ["X-Plex-Client-Identifier", this.identifier],
      ["X-Plex-Product", this.product],
      ["X-Plex-Version", this.version],
      ["X-Plex-Platform", this.platform],
      ["X-Plex-Platform-Version", this.platformVersion],
      ["X-Plex-Device", this.device],
      ["X-Plex-Device-Name", this.deviceName],
      ["X-Plex-Model", "desktop"],
      ["X-Plex-Provides", "player"],
    ];
  }

  /** The default header map: `Accept` plus the `X-Plex-*` values reqwest could encode. */
  headerMap(): Record<string, string> {
    const map: Record<string, string> = { Accept: "application/json" };
    for (const [k, v] of this.headers()) {
      // HeaderValue::from_str only accepts visible ASCII (and tab); other values are skipped.
      if (/^[\t\x20-\x7e]*$/.test(v)) map[k] = v;
    }
    return map;
  }
}

function hostname(): string | null {
  let raw: string | undefined = process.env.HOSTNAME;
  if (raw === undefined) {
    try {
      const out = spawnSync("hostname", { encoding: "utf8" });
      raw = out.status === 0 && typeof out.stdout === "string" ? out.stdout : undefined;
    } catch {
      raw = undefined;
    }
  }
  if (raw === undefined) return null;
  let h = raw.trim();
  while (h.endsWith(".local")) h = h.slice(0, -".local".length);
  return h !== "" ? h : null;
}

export type ParamValue = string | number | boolean | bigint;

/**
 * Ordered query parameters. Plex endpoints take almost everything as query
 * parameters, including on PUT/POST. Builders mutate and return `this`
 * (the Rust ones consume `self`); use `clone()` to branch.
 */
export class Params {
  entries: [string, string][];

  constructor(entries: Iterable<[string, ParamValue]> = []) {
    this.entries = [];
    for (const [k, v] of entries) this.push(k, v);
  }

  /** `Params::from_iter`. */
  static from(entries: Iterable<[string, ParamValue]>): Params {
    return new Params(entries);
  }

  set(key: string, value: ParamValue): this {
    this.push(key, value);
    return this;
  }

  opt(key: string, value: ParamValue | null | undefined): this {
    if (value !== null && value !== undefined) this.push(key, value);
    return this;
  }

  /** Adds `key=1` when `flag` is true. */
  flag(key: string, flag: boolean): this {
    return flag ? this.set(key, 1) : this;
  }

  push(key: string, value: ParamValue): void {
    this.entries.push([key, String(value)]);
  }

  extend(other: Params): this {
    this.entries.push(...other.entries.map(([k, v]): [string, string] => [k, v]));
    return this;
  }

  isEmpty(): boolean {
    return this.entries.length === 0;
  }

  clone(): Params {
    return new Params(this.entries);
  }

  /** `application/x-www-form-urlencoded`, as `url::form_urlencoded::Serializer` writes it. */
  toQueryString(): string {
    return new URLSearchParams(this.entries).toString();
  }

  equals(other: Params): boolean {
    return (
      this.entries.length === other.entries.length &&
      this.entries.every(([k, v], i) => k === other.entries[i]![0] && v === other.entries[i]![1])
    );
  }
}

/** Parse a URL like `Url::parse`, mapping failures to `PlexError` with `url::ParseError` wording. */
export function parseUrl(input: string, base?: string | URL): URL {
  try {
    return new URL(input, base);
  } catch {
    const text = input.trim();
    if (base === undefined && !/^[A-Za-z][A-Za-z0-9+.-]*:/.test(text)) {
      throw PlexError.url("relative URL without a base");
    }
    const m = /^[A-Za-z][A-Za-z0-9+.-]*:(?:\/\/)?([^/?#]*)/.exec(text);
    const authority = m?.[1] ?? "";
    const hostPort = authority.slice(authority.lastIndexOf("@") + 1);
    if (hostPort === "" || hostPort.startsWith(":")) throw PlexError.url("empty host");
    const port = /:(\d+)$/.exec(hostPort);
    if (port && Number(port[1]) > 65535) throw PlexError.url("invalid port number");
    if (/:[^\]]*[^\d:\]][^:]*$/.test(hostPort) && !hostPort.startsWith("[")) throw PlexError.url("invalid port number");
    throw PlexError.url("invalid domain character");
  }
}

/** Appends `extra` (already form-encoded) to the URL's existing query, leaving that query as it is. */
export function appendQuery(url: URL, extra: string): void {
  if (extra === "") return;
  const existing = url.search.startsWith("?") ? url.search.slice(1) : url.search;
  url.search = existing === "" ? extra : `${existing}&${extra}`;
}

/** A request being assembled; `Http.send` runs it. */
export class RequestBuilder {
  readonly method: string;
  readonly url: URL;
  readonly headers: Record<string, string> = {};
  body: string | null = null;
  timeoutMs: number | null = null;
  signal: AbortSignal | null = null;

  constructor(method: string, url: URL) {
    this.method = method;
    this.url = url;
  }

  header(name: string, value: string): this {
    this.headers[name] = value;
    return this;
  }

  /** URL-encoded form body (sets the content type). */
  form(pairs: [string, string][]): this {
    this.body = new URLSearchParams(pairs).toString();
    this.headers["Content-Type"] = "application/x-www-form-urlencoded";
    return this;
  }

  /** Total timeout for this request, replacing the client default. */
  timeout(ms: number): this {
    this.timeoutMs = ms;
    return this;
  }

  /** Cancels the request when aborted (no counterpart in reqwest, where dropping the future does it). */
  abortWith(signal: AbortSignal): this {
    this.signal = signal;
    return this;
  }
}

const CONNECT_TIMEOUT_MS = 8_000;
const TOTAL_TIMEOUT_MS = 60_000;
/** setTimeout overflows (and fires at once) beyond 2^31-1 ms. */
const MAX_TIMER_MS = 2 ** 31 - 1;

/** Called once the body of a response has been read: stops the total-timeout timer. */
const finishers = new WeakMap<Response, () => void>();

/** Stops the timers of a response whose body has been consumed (or abandoned). */
export function finishResponse(resp: Response): void {
  finishers.get(resp)?.();
}

/** Shared HTTP plumbing for plex.tv and server clients. */
export class Http {
  readonly info: ClientInfo;
  private readonly defaults: Record<string, string>;
  /** Origins that already answered once; the connect timeout only applies to the first request. */
  private readonly reached = new Set<string>();

  constructor(info: ClientInfo) {
    this.info = info;
    this.defaults = info.headerMap();
  }

  request(method: string, url: URL, token: string | null | undefined, params: Params): RequestBuilder {
    const u = new URL(url);
    appendQuery(u, params.toQueryString());
    const rb = new RequestBuilder(method, u);
    if (token !== null && token !== undefined) rb.header("X-Plex-Token", token);
    return rb;
  }

  /**
   * Sends the request; non-2xx answers become errors (401 is `Unauthorized`). reqwest has a
   * real connect timeout of 8s; fetch has none, so it is approximated by bounding the time to
   * the response headers of the first request to an origin. The 60s total timeout covers the
   * body too (it is stopped when `json`/`bytes`/`empty` finished reading).
   */
  async send(rb: RequestBuilder): Promise<Response> {
    const method = rb.method;
    const url = rb.url;
    const path = url.pathname;
    log.debug(`plex request method=${method} url=${redact(url)}`);

    const ac = new AbortController();
    let timedOut = false;
    const total = setTimeout(() => {
      timedOut = true;
      ac.abort();
    }, Math.min(rb.timeoutMs ?? TOTAL_TIMEOUT_MS, MAX_TIMER_MS));
    total.unref?.();
    const origin = url.origin;
    const connect = this.reached.has(origin)
      ? null
      : setTimeout(() => {
          timedOut = true;
          ac.abort();
        }, CONNECT_TIMEOUT_MS);
    connect?.unref?.();
    const external = rb.signal;
    const onExternal = () => ac.abort();
    if (external) {
      if (external.aborted) ac.abort();
      else external.addEventListener("abort", onExternal, { once: true });
    }
    const cleanup = () => {
      clearTimeout(total);
      if (connect) clearTimeout(connect);
      external?.removeEventListener("abort", onExternal);
    };

    let resp: Response;
    try {
      resp = await fetch(url, {
        method,
        headers: { ...this.defaults, ...rb.headers },
        body: rb.body ?? undefined,
        signal: ac.signal,
      });
    } catch (e) {
      cleanup();
      if (external?.aborted) throw e;
      const detail = timedOut ? "operation timed out" : e instanceof Error ? e.message : String(e);
      throw PlexError.http(`error sending request for url (${redact(url)}): ${detail}`);
    }
    if (connect) clearTimeout(connect);
    this.reached.add(origin);
    finishers.set(resp, cleanup);

    if (resp.ok) return resp;
    if (resp.status === 401) {
      await resp.body?.cancel().catch(() => undefined);
      cleanup();
      throw PlexError.unauthorized();
    }
    const body = await resp.text().catch(() => "");
    cleanup();
    throw PlexError.statusError(method, path, resp.status, body);
  }

  /** Reads the whole body as bytes, stopping the total timer afterwards. */
  async readBytes(resp: Response, url: URL): Promise<Uint8Array> {
    try {
      return new Uint8Array(await resp.arrayBuffer());
    } catch (e) {
      throw PlexError.http(`error decoding response body for url (${redact(url)}): ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      finishResponse(resp);
    }
  }

  /**
   * Sends and decodes the JSON answer. Without `normalize` the raw JSON value comes back
   * (`serde_json::Value`); with it, the value goes through that normalizer.
   */
  json(rb: RequestBuilder): Promise<unknown>;
  json<T>(rb: RequestBuilder, normalize: (v: unknown) => T): Promise<T>;
  async json<T>(rb: RequestBuilder, normalize?: (v: unknown) => T): Promise<unknown> {
    const resp = await this.send(rb);
    let path = new URL(rb.url).pathname;
    try {
      path = new URL(resp.url).pathname;
    } catch {
      // keep the request path
    }
    const bytes = await this.readBytes(resp, rb.url);
    return decode(path, bytes, normalize ?? ((v) => v));
  }

  async empty(rb: RequestBuilder): Promise<void> {
    const resp = await this.send(rb);
    // The body is irrelevant, but it must be consumed or cancelled so the connection is freed.
    await resp.body?.cancel().catch(() => undefined);
    finishResponse(resp);
  }
}

const textDecoder = new TextDecoder("utf-8");

/** Decode a JSON body with `normalize`. An empty body counts as `{}`. */
export function decode<T>(path: string, bytes: Uint8Array, normalize: (v: unknown) => T): T {
  // Some endpoints answer with an empty body where a container is expected.
  const text = textDecoder.decode(bytes);
  const body = /^[ \t\n\f\r]*$/.test(text) ? "{}" : text;
  let value: unknown;
  try {
    value = JSON.parse(body);
  } catch (e) {
    throw PlexError.decode(path, e instanceof Error ? e.message : String(e));
  }
  return decodeValue(path, value, normalize);
}

/** Run `normalize` over an already parsed value, mapping decode failures to `PlexError.decode`. */
export function decodeValue<T>(path: string, value: unknown, normalize: (v: unknown) => T): T {
  try {
    return normalize(value);
  } catch (e) {
    if (e instanceof DecodeFailure) throw PlexError.decode(path, e);
    throw e;
  }
}

/** The URL for logs: the token query parameter is replaced by `***`. */
export function redact(url: URL): string {
  const u = new URL(url);
  const pairs: [string, string][] = [];
  for (const [k, v] of u.searchParams) {
    pairs.push([k, k.toLowerCase() === "x-plex-token" ? "***" : v]);
  }
  u.search = pairs.length === 0 ? "" : new URLSearchParams(pairs).toString();
  return u.toString();
}
