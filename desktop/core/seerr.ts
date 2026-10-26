// Seerr: request movies and shows the server doesn't have yet, and follow or
// manage requests. The instance is set in `[seerr]` of config.toml; with no API
// key Patina signs in with the Plex account. The webview gets the shapes below,
// never the key or the session.

import { SeerrClient, SeerrError, normalizeUrl } from "../seerr";
import type { RawMediaInfo, RawMediaRequest, RawMovieDetails, RawResult, RawTvDetails, RawUser } from "../seerr";
import type { AppState } from "./state";

/** Where a title stands in Seerr (its `MediaStatus`); `null` when nobody has requested it. */
export type Availability = "unknown" | "pending" | "processing" | "partial" | "available" | "blocklisted" | "deleted";
/** A request's own state (`MediaRequestStatus`). */
export type RequestState = "pending" | "approved" | "declined" | "failed" | "completed";
export type SeerrMediaType = "movie" | "tv";

export interface SeerrTitle {
  mediaType: SeerrMediaType;
  tmdbId: number;
  title: string;
  year: number | null;
  overview: string;
  poster: string | null;
  backdrop: string | null;
  /** TMDB vote average, 0–10. */
  rating: number | null;
  status: Availability | null;
  status4k: Availability | null;
  /** Rating key of the copy on the Plex server, once available. */
  ratingKey: string | null;
}

export interface SeerrSeason {
  number: number;
  name: string;
  episodes: number | null;
  airDate: string | null;
  status: Availability | null;
  status4k: Availability | null;
  /** State of the newest open request covering it, if any. */
  requested: RequestState | null;
  requested4k: RequestState | null;
}

export interface SeerrRequest {
  id: number;
  state: RequestState;
  mediaType: SeerrMediaType;
  tmdbId: number;
  title: string;
  year: number | null;
  poster: string | null;
  is4k: boolean;
  seasons: number[];
  requestedBy: string;
  /** Requested by the signed-in Seerr user. */
  mine: boolean;
  createdAt: string | null;
  /** Availability of the title itself. */
  status: Availability | null;
}

export interface SeerrDetails extends SeerrTitle {
  tagline: string;
  genres: string[];
  /** Minutes (an episode's, for shows). */
  runtime: number | null;
  tvdbId: number | null;
  seasons: SeerrSeason[];
  requests: SeerrRequest[];
}

export interface SeerrPage<T> {
  page: number;
  pages: number;
  results: T[];
}

/** What the signed-in Seerr user may do. */
export interface SeerrCan {
  requestMovie: boolean;
  requestTv: boolean;
  request4kMovie: boolean;
  request4kTv: boolean;
  /** Choose the Radarr/Sonarr server, quality profile and folder. */
  advanced: boolean;
  /** Approve, decline, retry and delete anyone's requests. */
  manage: boolean;
}

export interface SeerrStatus {
  url: string;
  /** How Patina signs in. */
  auth: "apiKey" | "plex";
  version: string | null;
  user: { id: number; name: string; avatar: string | null };
  can: SeerrCan;
}

/** The instance as configured, without talking to it. */
export interface SeerrConfig {
  url: string;
  auth: "apiKey" | "plex";
}

export interface RequestOptionServer {
  id: number;
  name: string;
  isDefault: boolean;
  activeProfileId: number | null;
  activeDirectory: string | null;
  profiles: { id: number; name: string }[];
  rootFolders: { path: string; freeSpace: number | null }[];
}

export interface NewRequest {
  mediaType: SeerrMediaType;
  tmdbId: number;
  tvdbId?: number | null;
  seasons?: number[];
  is4k?: boolean;
  serverId?: number | null;
  profileId?: number | null;
  rootFolder?: string | null;
}

// Seerr's permission bits (server/lib/permissions.ts). ADMIN implies all of them.
const P = {
  ADMIN: 2,
  MANAGE_REQUESTS: 16,
  REQUEST: 32,
  REQUEST_4K: 1024,
  REQUEST_4K_MOVIE: 2048,
  REQUEST_4K_TV: 4096,
  REQUEST_ADVANCED: 8192,
  REQUEST_MOVIE: 262144,
  REQUEST_TV: 524288,
} as const;

/** Whether `permissions` grant any of `wanted` (admins have them all). */
export function hasAny(permissions: number, ...wanted: number[]): boolean {
  if (permissions & P.ADMIN) return true;
  return wanted.some((p) => (permissions & p) !== 0);
}

/** What the user may do, given which kinds have a 4K server set up. */
export function capabilities(permissions: number, has4k: { movie: boolean; tv: boolean }): SeerrCan {
  return {
    requestMovie: hasAny(permissions, P.REQUEST, P.REQUEST_MOVIE),
    requestTv: hasAny(permissions, P.REQUEST, P.REQUEST_TV),
    request4kMovie: has4k.movie && hasAny(permissions, P.REQUEST_4K, P.REQUEST_4K_MOVIE),
    request4kTv: has4k.tv && hasAny(permissions, P.REQUEST_4K, P.REQUEST_4K_TV),
    advanced: hasAny(permissions, P.REQUEST_ADVANCED, P.MANAGE_REQUESTS),
    manage: hasAny(permissions, P.MANAGE_REQUESTS),
  };
}

const AVAILABILITY: Record<number, Availability> = {
  1: "unknown",
  2: "pending",
  3: "processing",
  4: "partial",
  5: "available",
  6: "blocklisted",
  7: "deleted",
};
const REQUEST_STATES: Record<number, RequestState> = { 1: "pending", 2: "approved", 3: "declined", 4: "failed", 5: "completed" };

export const availability = (n: number | undefined): Availability | null => (n == null ? null : (AVAILABILITY[n] ?? null));
export const requestState = (n: number): RequestState => REQUEST_STATES[n] ?? "pending";

/** TMDB artwork URL for a Seerr image path (absolute URLs pass through). */
export function tmdbImage(path: string | null | undefined, size: "w500" | "w1280"): string | null {
  if (!path) return null;
  if (/^https?:\/\//.test(path)) return path;
  return `https://image.tmdb.org/t/p/${size}${path.startsWith("/") ? "" : "/"}${path}`;
}

const yearOf = (date: string | null | undefined): number | null => {
  const y = date ? Number.parseInt(date.slice(0, 4), 10) : NaN;
  return Number.isFinite(y) ? y : null;
};

function userName(u: RawUser | undefined): string {
  if (!u) return "Unknown";
  return u.displayName || u.username || u.plexUsername || u.jellyfinUsername || u.email || `User ${u.id}`;
}

function titleFrom(mediaType: SeerrMediaType, raw: RawResult | RawMovieDetails | RawTvDetails): SeerrTitle {
  const r = raw as RawResult;
  const info = raw.mediaInfo;
  return {
    mediaType,
    tmdbId: raw.id,
    title: (mediaType === "movie" ? r.title : r.name) ?? r.title ?? r.name ?? "Untitled",
    year: yearOf(mediaType === "movie" ? r.releaseDate : r.firstAirDate),
    overview: raw.overview ?? "",
    poster: tmdbImage(raw.posterPath, "w500"),
    backdrop: tmdbImage(raw.backdropPath, "w1280"),
    rating: raw.voteAverage ? raw.voteAverage : null,
    status: availability(info?.status),
    status4k: availability(info?.status4k),
    ratingKey: info?.ratingKey || null,
  };
}

/** Movies and shows of a search or discover page (people and collections are left out). */
export function titlesFrom(results: RawResult[]): SeerrTitle[] {
  return results.filter((r) => r.mediaType === "movie" || r.mediaType === "tv").map((r) => titleFrom(r.mediaType as SeerrMediaType, r));
}

function requestFrom(r: RawMediaRequest, me: number | null, known?: Pick<SeerrTitle, "title" | "year" | "poster">): SeerrRequest {
  const mediaType: SeerrMediaType = r.type ?? r.media?.mediaType ?? "movie";
  return {
    id: r.id,
    state: requestState(r.status),
    mediaType,
    tmdbId: r.media?.tmdbId ?? 0,
    title: known?.title ?? "",
    year: known?.year ?? null,
    poster: known?.poster ?? null,
    is4k: !!r.is4k,
    seasons: (r.seasons ?? []).map((s) => s.seasonNumber).sort((a, b) => a - b),
    requestedBy: userName(r.requestedBy),
    mine: me != null && r.requestedBy?.id === me,
    createdAt: r.createdAt ?? null,
    status: availability(r.is4k ? r.media?.status4k : r.media?.status),
  };
}

/** Seasons of a show (without specials), with their availability and open requests. */
export function seasonsFrom(tv: RawTvDetails): SeerrSeason[] {
  const info: RawMediaInfo | undefined = tv.mediaInfo;
  const open = (info?.requests ?? []).filter((r) => r.status !== 3).sort((a, b) => (a.createdAt ?? "").localeCompare(b.createdAt ?? ""));
  const requested = (n: number, is4k: boolean): RequestState | null => {
    let state: RequestState | null = null;
    for (const r of open) if (!!r.is4k === is4k && r.seasons?.some((s) => s.seasonNumber === n)) state = requestState(r.status);
    return state;
  };
  return (tv.seasons ?? [])
    .filter((s) => s.seasonNumber > 0)
    .map((s) => {
      const media = info?.seasons?.find((m) => m.seasonNumber === s.seasonNumber);
      return {
        number: s.seasonNumber,
        name: s.name || `Season ${s.seasonNumber}`,
        episodes: s.episodeCount ?? null,
        airDate: s.airDate ?? null,
        status: availability(media?.status),
        status4k: availability(media?.status4k),
        requested: requested(s.seasonNumber, false),
        requested4k: requested(s.seasonNumber, true),
      };
    });
}

// The client, kept while the settings it was made from stay the same (it holds the Plex session).
let cached: { key: string; client: SeerrClient; me: RawUser | null } | null = null;

/** The configured instance; `null` when Seerr is not set up. */
export function config(app: AppState): SeerrConfig | null {
  const s = app.settings().seerr;
  if (s.url.trim() === "") return null;
  return { url: s.url.trim(), auth: s.api_key.trim() ? "apiKey" : "plex" };
}

function plexToken(app: AppState): string {
  const token = app.settings().auth.token;
  if (!token) throw new SeerrError("Not signed in to plex.tv: add a Seerr API key to connect.");
  return token;
}

function clientFor(url: string, apiKey: string, app: AppState): { key: string; client: SeerrClient; me: RawUser | null } {
  const key = apiKey ? `${url}|key|${apiKey}` : `${url}|plex|${plexToken(app)}`;
  if (cached?.key === key) return cached;
  const client = new SeerrClient(url, apiKey ? { apiKey } : { plexToken: plexToken(app) });
  return { key, client, me: null };
}

function current(app: AppState) {
  const s = app.settings().seerr;
  if (s.url.trim() === "") throw new SeerrError("Seerr is not set up. Add it in Settings.");
  const entry = clientFor(s.url.trim(), s.api_key.trim(), app);
  cached = entry;
  return entry;
}

async function me(entry: { client: SeerrClient; me: RawUser | null }): Promise<RawUser> {
  entry.me ??= await entry.client.me();
  return entry.me;
}

/** Talk to the instance: version, the signed-in user and what they may do. */
export async function status(app: AppState): Promise<SeerrStatus> {
  return statusOf(current(app));
}

async function statusOf(entry: { client: SeerrClient; me: RawUser | null }): Promise<SeerrStatus> {
  const { client } = entry;
  const [info, user] = await Promise.all([client.status().catch(() => null), client.me()]);
  entry.me = user;
  // Without a 4K server there is nothing to request in 4K. Users may not be allowed to list them.
  const has4k = async (kind: "radarr" | "sonarr") => (await client.services(kind).catch(() => [])).some((s) => s.is4k);
  const [movie4k, tv4k] = await Promise.all([has4k("radarr"), has4k("sonarr")]);
  return {
    url: client.baseUrl,
    auth: client.authKind,
    version: info?.version ?? null,
    user: { id: user.id, name: userName(user), avatar: user.avatar ? avatarUrl(client.baseUrl, user.avatar) : null },
    can: capabilities(user.permissions ?? 0, { movie: movie4k, tv: tv4k }),
  };
}

/** Avatars are plex.tv or gravatar URLs, or paths on the Seerr server. */
function avatarUrl(base: string, avatar: string): string {
  return /^https?:\/\//.test(avatar) ? avatar : `${base}${avatar.startsWith("/") ? "" : "/"}${avatar}`;
}

/**
 * Check the instance answers and accepts the sign-in, then save it.
 * `apiKey` null keeps the saved key; empty signs in with the Plex account.
 */
export async function connect(app: AppState, url: string, apiKey: string | null): Promise<SeerrStatus> {
  const base = normalizeUrl(url);
  const key = (apiKey ?? app.settings().seerr.api_key).trim();
  const entry = clientFor(base, key, app);
  const result = await statusOf(entry);
  app.tryUpdateSettings((s) => {
    s.seerr.url = base;
    s.seerr.api_key = key;
  });
  cached = entry;
  return result;
}

export function disconnect(app: AppState): void {
  cached = null;
  app.tryUpdateSettings((s) => {
    s.seerr.url = "";
    s.seerr.api_key = "";
  });
}

const DISCOVER: Record<string, string> = {
  trending: "/discover/trending",
  movies: "/discover/movies",
  tv: "/discover/tv",
  "upcoming-movies": "/discover/movies/upcoming",
  "upcoming-tv": "/discover/tv/upcoming",
};

export async function discover(app: AppState, list: string, page: number): Promise<SeerrPage<SeerrTitle>> {
  const path = DISCOVER[list];
  if (!path) throw new SeerrError(`Unknown list ${list}.`);
  const got = await current(app).client.discover(path, Math.max(1, page));
  return { page: got.page, pages: got.totalPages, results: titlesFrom(got.results) };
}

export async function search(app: AppState, query: string, page: number): Promise<SeerrPage<SeerrTitle>> {
  const q = query.trim();
  if (q === "") return { page: 1, pages: 0, results: [] };
  const got = await current(app).client.search(q, Math.max(1, page));
  return { page: got.page, pages: got.totalPages, results: titlesFrom(got.results) };
}

export async function details(app: AppState, mediaType: SeerrMediaType, tmdbId: number): Promise<SeerrDetails> {
  const entry = current(app);
  const user = await me(entry);
  const raw = mediaType === "movie" ? await entry.client.movie(tmdbId) : await entry.client.tv(tmdbId);
  const title = titleFrom(mediaType, raw);
  remember(mediaType, tmdbId, title);
  const tv = mediaType === "tv" ? (raw as RawTvDetails) : null;
  const runtime = tv ? (tv.episodeRunTime?.[0] ?? null) : ((raw as RawMovieDetails).runtime ?? null);
  return {
    ...title,
    tagline: raw.tagline ?? "",
    genres: (raw.genres ?? []).map((g) => g.name),
    runtime: runtime || null,
    tvdbId: tv?.externalIds?.tvdbId ?? raw.mediaInfo?.tvdbId ?? null,
    seasons: tv ? seasonsFrom(tv) : [],
    requests: (raw.mediaInfo?.requests ?? [])
      .map((r) => requestFrom({ ...r, type: r.type ?? mediaType, media: { ...raw.mediaInfo, ...r.media, tmdbId } }, user.id, title))
      .sort((a, b) => (b.createdAt ?? "").localeCompare(a.createdAt ?? "")),
  };
}

/** Make a request. Resolves to its state: `approved` when the user's requests are approved automatically. */
export async function request(app: AppState, r: NewRequest): Promise<RequestState> {
  if (r.mediaType === "tv" && !r.seasons?.length) throw new SeerrError("Choose at least one season.");
  const made = await current(app).client.createRequest({
    mediaType: r.mediaType,
    mediaId: r.tmdbId,
    tvdbId: r.mediaType === "tv" ? (r.tvdbId ?? undefined) : undefined,
    seasons: r.mediaType === "tv" ? r.seasons : undefined,
    is4k: !!r.is4k,
    serverId: r.serverId ?? undefined,
    profileId: r.profileId ?? undefined,
    rootFolder: r.rootFolder ?? undefined,
  });
  return requestState(made.status);
}

/** Radarr (movies) or Sonarr (shows) servers to send a request to, with their profiles and folders. */
export async function requestOptions(app: AppState, mediaType: SeerrMediaType, is4k: boolean): Promise<RequestOptionServer[]> {
  const { client } = current(app);
  const kind = mediaType === "movie" ? "radarr" : "sonarr";
  const servers = (await client.services(kind)).filter((s) => !!s.is4k === is4k);
  return Promise.all(
    servers.map(async (s) => {
      const d = await client.service(kind, s.id);
      return {
        id: s.id,
        name: s.name,
        isDefault: !!s.isDefault,
        activeProfileId: d.server.activeProfileId ?? s.activeProfileId ?? null,
        activeDirectory: d.server.activeDirectory ?? s.activeDirectory ?? null,
        profiles: d.profiles ?? [],
        rootFolders: (d.rootFolders ?? []).map((f) => ({ path: f.path, freeSpace: f.freeSpace ?? null })),
      };
    }),
  );
}

export const REQUEST_FILTERS = ["all", "pending", "approved", "processing", "available", "unavailable", "failed"] as const;
const PAGE_SIZE = 20;

// Titles of requested media: a request only names its TMDB id.
const titles = new Map<string, Pick<SeerrTitle, "title" | "year" | "poster">>();
function remember(mediaType: SeerrMediaType, tmdbId: number, t: Pick<SeerrTitle, "title" | "year" | "poster">) {
  const key = `${mediaType}:${tmdbId}`;
  titles.delete(key);
  titles.set(key, { title: t.title, year: t.year, poster: t.poster });
  if (titles.size > 1000) titles.delete(titles.keys().next().value!);
}

export async function requests(app: AppState, filter: string, page: number, mine = false): Promise<SeerrPage<SeerrRequest>> {
  if (!(REQUEST_FILTERS as readonly string[]).includes(filter)) throw new SeerrError(`Unknown filter ${filter}.`);
  const entry = current(app);
  const user = await me(entry);
  const p = Math.max(1, page);
  const got = await entry.client.requests({
    take: PAGE_SIZE,
    skip: (p - 1) * PAGE_SIZE,
    filter,
    sort: "added",
    requestedBy: mine ? user.id : undefined,
  });
  const results = await Promise.all(
    got.results.map(async (r) => {
      const mediaType: SeerrMediaType = r.type ?? r.media?.mediaType ?? "movie";
      const tmdbId = r.media?.tmdbId;
      let known = tmdbId != null ? titles.get(`${mediaType}:${tmdbId}`) : undefined;
      if (!known && tmdbId != null) {
        try {
          const raw = mediaType === "movie" ? await entry.client.movie(tmdbId) : await entry.client.tv(tmdbId);
          known = titleFrom(mediaType, raw);
          remember(mediaType, tmdbId, known);
        } catch {
          // Shown by its TMDB id instead.
        }
      }
      const out = requestFrom({ ...r, type: mediaType }, user.id, known);
      if (!out.title) out.title = `TMDB ${tmdbId ?? "?"}`;
      return out;
    }),
  );
  return { page: got.pageInfo.page, pages: got.pageInfo.pages, results };
}

/** `approve`, `decline`, `retry` or `delete` a request. */
export async function requestAction(app: AppState, id: number, action: string): Promise<void> {
  const { client } = current(app);
  switch (action) {
    case "approve":
    case "decline":
      return void (await client.setRequestStatus(id, action));
    case "retry":
      return void (await client.retryRequest(id));
    case "delete":
      return client.deleteRequest(id);
    default:
      throw new SeerrError(`Unknown request action ${action}.`);
  }
}

/** The Seerr web page of a title. */
export function pageUrl(app: AppState, mediaType: SeerrMediaType, tmdbId: number): string {
  return `${current(app).client.baseUrl}/${mediaType === "tv" ? "tv" : "movie"}/${tmdbId}`;
}

/** Forget the client (tests, and a sign-out that changes the Plex token). */
export function reset(): void {
  cached = null;
  titles.clear();
}
