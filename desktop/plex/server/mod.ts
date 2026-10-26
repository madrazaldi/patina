// Client bound to one Plex Media Server connection (port of crates/plex/src/server/mod.rs).
//
// The Rust `impl PlexServer` is spread over several files; here each file adds its
// endpoints in a class that extends the previous one, and `index.ts` ends the chain with
// `PlexServer`. This file holds the connection, URL building and request helpers.

import { appendQuery, Http, Params, ClientInfo, parseUrl } from "../client";
import { DecodeFailure } from "../de";

import { normalizeEnvelope, normalizeIdentity, type Identity, type MediaContainer } from "../models";

/** Server connection plumbing; the endpoint wrappers are added by the subclasses. */
export class ServerBase {
  /** Shared HTTP plumbing (internal). */
  readonly http: Http;
  protected _base: URL;
  protected _token: string;
  protected _machineIdentifier: string | null;

  /** `PlexServer::new(info, base_url, token)`; pass an `Http` instead of `info` to share a client (`with_http`). */
  constructor(infoOrHttp: ClientInfo | Http, baseUrl: string, token: string) {
    this.http = infoOrHttp instanceof Http ? infoOrHttp : new Http(infoOrHttp);
    const base = parseUrl(baseUrl);
    if (!base.pathname.endsWith("/")) base.pathname = `${base.pathname}/`;
    this._base = base;
    this._token = token;
    this._machineIdentifier = null;
  }

  /** A copy of this client (the Rust type is `Clone`). */
  clone(): this {
    return Object.assign(Object.create(Object.getPrototypeOf(this) as object) as this, this);
  }

  /** Returns a copy bound to the server's machine identifier. */
  withMachineIdentifier(id: string | null): this {
    const copy = this.clone();
    copy._machineIdentifier = id;
    return copy;
  }

  /** `PartialEq`: same base URL and token. */
  equals(other: ServerBase): boolean {
    return this._base.href === other._base.href && this._token === other._token;
  }

  /** Like the Rust `Debug` impl: the token is never included. */
  toJSON(): { base: string; machineIdentifier: string | null } {
    return { base: this._base.href, machineIdentifier: this._machineIdentifier };
  }

  baseUrl(): URL {
    return new URL(this._base);
  }

  token(): string {
    return this._token;
  }

  clientInfo(): ClientInfo {
    return this.http.info;
  }

  machineIdentifier(): string | null {
    return this._machineIdentifier;
  }

  /** Resolve a server path (`/library/...`) or absolute URL. */
  url(path: string): URL {
    if (path.startsWith("http://") || path.startsWith("https://")) return parseUrl(path);
    // "./" keeps a colon in the first segment (`/tv.plex.providers.epg.cloud:2/...`) from reading as a scheme.
    return parseUrl(`./${path.replace(/^\/+/, "")}`, this._base);
  }

  /** A URL with the token embedded, for handing to external programs. */
  authedUrl(path: string, params: Params): URL {
    const url = this.url(path);
    const extra = new Params(params.entries).set("X-Plex-Token", this._token);
    appendQuery(url, extra.toQueryString());
    return url;
  }

  /**
   * `server://<machine>/com.plexapp.plugins.library<path>` URIs used by
   * playlists, collections and play queues.
   */
  libraryUri(path: string): string {
    return `server://${this._machineIdentifier ?? ""}/com.plexapp.plugins.library${path}`;
  }

  /** Library URI for one or more items (`/library/metadata/1,2,3`). */
  itemUri(ratingKeys: readonly string[]): string {
    return this.libraryUri(`/library/metadata/${ratingKeys.join(",")}`);
  }

  // ----- request helpers ----------------------------------------------

  protected async request<T>(method: string, path: string, params: Params, normalize: (v: unknown) => T): Promise<T> {
    const rb = this.http.request(method, this.url(path), this._token, params);
    return this.http.json(rb, normalize);
  }

  protected async container(method: string, path: string, params: Params): Promise<MediaContainer> {
    const env = await this.request(method, path, params, normalizeEnvelope);
    return env.MediaContainer;
  }

  protected get(path: string, params: Params = new Params()): Promise<MediaContainer> {
    return this.container("GET", path, params);
  }

  protected async empty(method: string, path: string, params: Params = new Params()): Promise<void> {
    const rb = this.http.request(method, this.url(path), this._token, params);
    return this.http.empty(rb);
  }

  protected put(path: string, params: Params = new Params()): Promise<void> {
    return this.empty("PUT", path, params);
  }

  protected delete(path: string, params: Params = new Params()): Promise<void> {
    return this.empty("DELETE", path, params);
  }

  protected post(path: string, params: Params = new Params()): Promise<void> {
    return this.empty("POST", path, params);
  }

  /** Escape hatch: any endpoint, raw JSON back. */
  async raw(method: string, path: string, params: Params = new Params()): Promise<unknown> {
    return this.request(method, path, params, (v) => v);
  }

  /** Escape hatch returning the typed container. */
  fetch(path: string, params: Params = new Params()): Promise<MediaContainer> {
    return this.get(path, params);
  }

  /** GET raw bytes (images, subtitles, downloads) and the response content type. */
  async bytes(path: string, params: Params = new Params()): Promise<[Uint8Array, string | null]> {
    const rb = this.http.request("GET", this.url(path), this._token, params);
    const resp = await this.http.send(rb);
    const ctype = resp.headers.get("content-type");
    return [await this.http.readBytes(resp, rb.url), ctype];
  }

  /** Streaming GET, for large downloads (subject to the 60s total timeout, like the Rust client). */
  async stream(path: string, params: Params = new Params()): Promise<Response> {
    const rb = this.http.request("GET", this.url(path), this._token, params);
    return this.http.send(rb);
  }

  /** Streaming GET without the usual request timeout, for whole media files. */
  async download(path: string, params: Params = new Params()): Promise<Response> {
    const rb = this.http.request("GET", this.url(path), this._token, params).timeout(DOWNLOAD_TIMEOUT_MS);
    return this.http.send(rb);
  }

  /** Resume a media transfer. Identity encoding keeps byte offsets meaningful. */
  async downloadRange(path: string, offset: number, validator?: string | null): Promise<Response> {
    const rb = this.http
      .request("GET", this.url(path), this._token, new Params().set("download", 1))
      .header("Accept-Encoding", "identity")
      .timeout(DOWNLOAD_TIMEOUT_MS);
    if (offset > 0) {
      rb.header("Range", `bytes=${offset}-`);
      if (validator !== null && validator !== undefined) rb.header("If-Range", validator);
    }
    return this.http.send(rb);
  }

  // ----- server info ---------------------------------------------------

  /** Server root: capabilities, version, flags (in `extra`) and feature directories. */
  capabilities(): Promise<MediaContainer> {
    return this.get("/", new Params());
  }

  /**
   * Unauthenticated identity check; used to probe connections. `signal` is an addition to
   * the Rust signature so that losing connection races can be cancelled.
   */
  async identity(signal?: AbortSignal): Promise<Identity> {
    const rb = this.http.request("GET", this.url("/identity"), this._token, new Params());
    if (signal) rb.abortWith(signal);
    return this.http.json(rb, (v) => {
      const o = v as Record<string, unknown> | null;
      if (typeof o !== "object" || o === null || Array.isArray(o)) {
        throw new DecodeFailure("", "invalid type: expected struct Wrap");
      }
      if (o.MediaContainer === undefined) throw new DecodeFailure("", "missing field `MediaContainer`");
      try {
        return normalizeIdentity(o.MediaContainer);
      } catch (e) {
        if (e instanceof DecodeFailure) throw e.under("MediaContainer");
        throw e;
      }
    });
  }

  /** Resized image URL via the photo transcoder. `source` is a server path or absolute URL. */
  imageUrl(source: string, width: number, height: number): URL {
    const params = new Params()
      .set("url", source)
      .set("width", width)
      .set("height", height)
      .set("minSize", 1)
      .set("upscale", 1);
    return this.authedUrl("/photo/:/transcode", params);
  }

  /** Fetch a resized image through the photo transcoder. */
  image(source: string, width: number, height: number): Promise<[Uint8Array, string | null]> {
    const params = new Params()
      .set("url", source)
      .set("width", width)
      .set("height", height)
      .set("minSize", 1)
      .set("upscale", 1);
    return this.bytes("/photo/:/transcode", params);
  }
}

/** 24 hours, the timeout of media downloads. */
const DOWNLOAD_TIMEOUT_MS = 60 * 60 * 24 * 1000;
