// Client of the Seerr API (`/api/v1`, https://docs.seerr.dev/api/seerr-api/),
// which Overseerr and Jellyseerr share. It signs in with an API key
// (`X-Api-Key`), or with the Plex account token (`POST /auth/plex`), whose
// session cookie it keeps and renews when Seerr forgets it.

import type { RawMediaRequest, RawMovieDetails, RawPage, RawRequestPage, RawResult, RawService, RawServiceDetails, RawTvDetails, RawUser } from "./models";

/** Requests give up after this long. */
const TIMEOUT_MS = 30_000;

export type SeerrAuth = { apiKey: string } | { plexToken: string };

export class SeerrError extends Error {
  /** HTTP status, when Seerr answered. */
  readonly status: number | undefined;
  constructor(message: string, status?: number) {
    super(message);
    this.name = "SeerrError";
    this.status = status;
  }
}

/**
 * The base URL as typed by the user, without a trailing slash or `/api/v1`.
 * A missing scheme means http, as Seerr usually runs on a LAN port.
 */
export function normalizeUrl(input: string): string {
  let url = input.trim();
  if (url === "") throw new SeerrError("Enter the address of your Seerr server.");
  if (!/^[a-z][a-z0-9+.-]*:\/\//i.test(url)) url = `http://${url}`;
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    throw new SeerrError(`“${input.trim()}” is not a valid address.`);
  }
  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") throw new SeerrError("The address must start with http:// or https://.");
  parsed.search = "";
  parsed.hash = "";
  return parsed.toString().replace(/\/+$/, "").replace(/\/api\/v1$/, "");
}

/**
 * Seerr validates queries against its OpenAPI schema and rejects some characters
 * unless they are percent-encoded, so encode everything `encodeURIComponent` leaves.
 */
export function encodeQuery(params: Record<string, string | number | boolean | undefined>): string {
  const enc = (v: string) => encodeURIComponent(v).replace(/[!'()*]/g, (c) => `%${c.charCodeAt(0).toString(16).toUpperCase()}`);
  return Object.entries(params)
    .filter(([, v]) => v !== undefined)
    .map(([k, v]) => `${enc(k)}=${enc(String(v))}`)
    .join("&");
}

export class SeerrClient {
  readonly baseUrl: string;
  private readonly auth: SeerrAuth;
  /** `connect.sid=...` after a Plex sign-in. */
  private cookie: string | null = null;
  private signingIn: Promise<void> | null = null;

  constructor(url: string, auth: SeerrAuth) {
    this.baseUrl = normalizeUrl(url);
    this.auth = auth;
  }

  /** How requests are signed in. */
  get authKind(): "apiKey" | "plex" {
    return "apiKey" in this.auth ? "apiKey" : "plex";
  }

  /** Version of the server; needs no sign-in. */
  status(): Promise<{ version?: string; commitTag?: string }> {
    return this.request("GET", "/status", undefined, undefined, false);
  }

  /** The signed-in user. */
  me(): Promise<RawUser> {
    return this.request("GET", "/auth/me");
  }

  search(query: string, page = 1): Promise<RawPage<RawResult>> {
    return this.request("GET", "/search", { query, page });
  }

  /** `path` is a discover list such as `/discover/trending` or `/discover/movies/upcoming`. */
  discover(path: string, page = 1): Promise<RawPage<RawResult>> {
    return this.request("GET", path, { page });
  }

  movie(tmdbId: number): Promise<RawMovieDetails> {
    return this.request("GET", `/movie/${tmdbId}`);
  }

  tv(tmdbId: number): Promise<RawTvDetails> {
    return this.request("GET", `/tv/${tmdbId}`);
  }

  requests(query: { take: number; skip: number; filter: string; sort: string; requestedBy?: number }): Promise<RawRequestPage> {
    return this.request("GET", "/request", query);
  }

  createRequest(body: {
    mediaType: "movie" | "tv";
    mediaId: number;
    tvdbId?: number;
    seasons?: number[];
    is4k: boolean;
    serverId?: number;
    profileId?: number;
    rootFolder?: string;
  }): Promise<RawMediaRequest> {
    return this.request("POST", "/request", undefined, body);
  }

  /** `approve` or `decline` a pending request. */
  setRequestStatus(id: number, status: "approve" | "decline"): Promise<RawMediaRequest> {
    return this.request("POST", `/request/${id}/${status}`);
  }

  retryRequest(id: number): Promise<RawMediaRequest> {
    return this.request("POST", `/request/${id}/retry`);
  }

  deleteRequest(id: number): Promise<void> {
    return this.request("DELETE", `/request/${id}`);
  }

  /** Radarr (`radarr`) or Sonarr (`sonarr`) servers set up in Seerr. */
  services(kind: "radarr" | "sonarr"): Promise<RawService[]> {
    return this.request("GET", `/service/${kind}`);
  }

  /** Quality profiles and root folders of one Radarr or Sonarr server. */
  service(kind: "radarr" | "sonarr", id: number): Promise<RawServiceDetails> {
    return this.request("GET", `/service/${kind}/${id}`);
  }

  private async signIn(): Promise<void> {
    if (!("plexToken" in this.auth)) return;
    const resp = await this.send("POST", "/auth/plex", undefined, { authToken: this.auth.plexToken }, null);
    if (!resp.ok) throw await failure(resp, "POST", "/auth/plex");
    const cookie = resp.headers.getSetCookie().map((c) => c.split(";")[0]!).find((c) => c.startsWith("connect.sid="));
    if (!cookie) throw new SeerrError("Seerr accepted the Plex sign-in but sent no session.");
    this.cookie = cookie;
  }

  /** Sign in once, however many requests are waiting for it. */
  private ensureSignedIn(): Promise<void> {
    this.signingIn ??= this.signIn().finally(() => (this.signingIn = null));
    return this.signingIn;
  }

  private async request<T>(
    method: string,
    path: string,
    query?: Record<string, string | number | boolean | undefined>,
    body?: unknown,
    authed = true,
  ): Promise<T> {
    const plex = authed && "plexToken" in this.auth;
    if (plex && !this.cookie) await this.ensureSignedIn();
    const sent = authed ? this.cookie : null;
    let resp = await this.send(method, path, query, body, sent);
    // Seerr restarted or the session expired: sign in again (once for all requests that noticed), and retry once.
    if (plex && sent && (resp.status === 401 || resp.status === 403)) {
      if (this.cookie === sent) this.cookie = null;
      if (!this.cookie) await this.ensureSignedIn();
      resp = await this.send(method, path, query, body, this.cookie);
    }
    if (!resp.ok) throw await failure(resp, method, path);
    const text = await resp.text();
    if (text.trim() === "") return undefined as T;
    try {
      return JSON.parse(text) as T;
    } catch {
      throw new SeerrError(`${this.baseUrl} did not answer like a Seerr server.`);
    }
  }

  private async send(
    method: string,
    path: string,
    query: Record<string, string | number | boolean | undefined> | undefined,
    body: unknown,
    cookie: string | null,
  ): Promise<Response> {
    const qs = query ? encodeQuery(query) : "";
    const url = `${this.baseUrl}/api/v1${path}${qs ? `?${qs}` : ""}`;
    const headers: Record<string, string> = { Accept: "application/json" };
    if ("apiKey" in this.auth) headers["X-Api-Key"] = this.auth.apiKey;
    if (cookie) headers.Cookie = cookie;
    if (body !== undefined) headers["Content-Type"] = "application/json";
    try {
      return await fetch(url, {
        method,
        headers,
        body: body === undefined ? undefined : JSON.stringify(body),
        redirect: "manual",
        signal: AbortSignal.timeout(TIMEOUT_MS),
      });
    } catch (e) {
      const why = e instanceof Error && e.name === "TimeoutError" ? "timed out" : e instanceof Error ? e.message : String(e);
      throw new SeerrError(`Could not reach Seerr at ${this.baseUrl}: ${why}`);
    }
  }
}

/** The error of a failed response, with Seerr's own message when it sent one. */
async function failure(resp: Response, method: string, path: string): Promise<SeerrError> {
  let detail = "";
  try {
    const text = await resp.text();
    const json = JSON.parse(text) as { message?: unknown };
    if (typeof json.message === "string") detail = json.message;
  } catch {
    // Not JSON: the status says enough.
  }
  // Seerr answers a bad key or session with 403 too, so a refused sign-in check is a sign-in problem.
  if (resp.status === 401 || (resp.status === 403 && path.startsWith("/auth/"))) {
    return new SeerrError("Seerr did not accept the sign-in. Check the API key, or that your Plex account has access.", resp.status);
  }
  if (resp.status === 403) return new SeerrError(detail ? `Seerr refused: ${detail}` : `Seerr refused ${method} ${path}.`, resp.status);
  if (resp.status >= 300 && resp.status < 400) return new SeerrError(`Seerr redirected ${method} ${path}; check the address.`, resp.status);
  return new SeerrError(detail || `${method} ${path} failed with HTTP ${resp.status}`, resp.status);
}
