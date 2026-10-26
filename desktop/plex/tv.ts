// plex.tv account client (port of crates/plex/src/tv.rs).

import { Http, Params, ClientInfo, parseUrl } from "./client";
import { PlexError } from "./error";
import { DecodeFailure, list, pointer } from "./de";
import {
  Resource,
  normalizeEnvelope,
  normalizeFriend,
  normalizeHomeUser,
  normalizeMetadata,
  normalizePin,
  normalizeResource,
  normalizeUser,
  normalizeUserState,
  type Friend,
  type HomeUser,
  type MediaContainer,
  type Metadata,
  type Pin,
  type User,
  type UserState,
} from "./models";

const PLEX_TV = "https://plex.tv";
const CLIENTS = "https://clients.plex.tv";
const DISCOVER = "https://discover.provider.plex.tv";
const METADATA = "https://metadata.provider.plex.tv";

/**
 * Debug builds: `PATINA_PLEX_TV_URL` points the account and resource endpoints at
 * a mock server (tools/mock_pms.py), so multi-server features can be tried offline.
 * The desktop app sets `PATINA_DEBUG_BUILD=1` for dev-channel builds (see core/ops/setup.ts);
 * release builds ignore the override.
 */
function accountBase(def: string): string {
  const url = process.env.PATINA_PLEX_TV_URL;
  if (process.env.PATINA_DEBUG_BUILD === "1" && url !== undefined) return url;
  return def;
}

/** `Url::parse(base)?.join(path)?` */
function joinUrl(base: string, path: string): URL {
  return parseUrl(path, parseUrl(base));
}

/** Client for plex.tv account services. */
export class PlexTv {
  /** Shared HTTP plumbing (used by `connectResource` to bind servers to the same client). */
  readonly http: Http;
  private readonly _token: string | null;

  constructor(info: ClientInfo, token: string | null = null, http?: Http) {
    this.http = http ?? new Http(info);
    this._token = token;
  }

  withToken(token: string): PlexTv {
    return new PlexTv(this.http.info, token, this.http);
  }

  token(): string | null {
    return this._token;
  }

  clientInfo(): ClientInfo {
    return this.http.info;
  }

  private async get<T>(base: string, path: string, params: Params, normalize: (v: unknown) => T): Promise<T> {
    const rb = this.http.request("GET", joinUrl(base, path), this._token, params);
    return this.http.json(rb, normalize);
  }

  /** Escape hatch: call any plex.tv endpoint and get raw JSON back. */
  async raw(method: string, url: string, params: Params = new Params()): Promise<unknown> {
    const rb = this.http.request(method, parseUrl(url), this._token, params);
    return this.http.json(rb);
  }

  // ----- Sign-in -------------------------------------------------------

  /**
   * Create a PIN for the link flow. `strong` PINs are long and meant to be
   * embedded in the app.plex.tv auth URL rather than typed.
   */
  async createPin(strong: boolean): Promise<Pin> {
    const rb = this.http.request("POST", joinUrl(PLEX_TV, "/api/v2/pins"), null, new Params().flag("strong", strong));
    return this.http.json(rb, normalizePin);
  }

  /** Poll a PIN; `authToken` is set once the user approved it. */
  async checkPin(id: number): Promise<Pin> {
    const rb = this.http.request("GET", joinUrl(PLEX_TV, `/api/v2/pins/${id}`), null, new Params());
    return this.http.json(rb, normalizePin);
  }

  /** URL the user opens in a browser to approve a strong PIN. */
  authUrl(code: string): string {
    const info = this.http.info;
    const query = new Params()
      .set("clientID", info.identifier)
      .set("code", code)
      .set("context[device][product]", info.product)
      .set("context[device][platform]", info.platform)
      .set("context[device][device]", info.device)
      .set("context[device][deviceName]", info.deviceName)
      .set("context[device][version]", info.version)
      .toQueryString();
    return `https://app.plex.tv/auth#?${query}`;
  }

  /** Username/password sign-in. `code` is the 2FA verification code, if enabled. */
  async signIn(login: string, password: string, code?: string | null): Promise<User> {
    const form: [string, string][] = [
      ["login", login],
      ["password", password],
      ["rememberMe", "true"],
    ];
    if (code !== null && code !== undefined) form.push(["verificationCode", code]);
    const rb = this.http.request("POST", joinUrl(PLEX_TV, "/api/v2/users/signin"), null, new Params()).form(form);
    return this.http.json(rb, normalizeUser);
  }

  /** Validates the current token and returns the account. */
  async user(): Promise<User> {
    return this.get(accountBase(PLEX_TV), "/api/v2/user", new Params(), normalizeUser);
  }

  // ----- Resources -----------------------------------------------------

  /** All devices the account can access, including servers with their connections. */
  async resources(): Promise<Resource[]> {
    const params = new Params().set("includeHttps", 1).set("includeRelay", 1).set("includeIPv6", 1);
    return this.get(accountBase(CLIENTS), "/api/v2/resources", params, (v) => list(v, normalizeResource));
  }

  async servers(): Promise<Resource[]> {
    return (await this.resources()).filter(Resource.isServer);
  }

  // ----- Plex Home -----------------------------------------------------

  async homeUsers(): Promise<HomeUser[]> {
    const v = await this.get(CLIENTS, "/api/v2/home/users", new Params(), (x) => x);
    let items: unknown;
    if (Array.isArray(v)) items = v;
    else if (typeof v === "object" && v !== null) items = (v as Record<string, unknown>).users ?? [];
    else items = [];
    try {
      return list(items, normalizeHomeUser);
    } catch (e) {
      if (e instanceof DecodeFailure) throw PlexError.other(`home users: ${e.detail}`);
      throw e;
    }
  }

  /** Switch to another Plex Home user. Returns that user with a fresh `authToken`. */
  async switchHomeUser(uuid: string, pin?: string | null): Promise<User> {
    const params = new Params().opt("pin", pin);
    const rb = this.http.request("POST", joinUrl(CLIENTS, `/api/v2/home/users/${uuid}/switch`), this._token, params);
    return this.http.json(rb, normalizeUser);
  }

  async friends(): Promise<Friend[]> {
    return this.get(PLEX_TV, "/api/v2/friends", new Params(), (v) => list(v, normalizeFriend));
  }

  // ----- Discover & watchlist -----------------------------------------

  /** The account watchlist. Items carry discover ids in `ratingKey`. */
  async watchlist(start: number, size: number): Promise<MediaContainer> {
    const params = new Params()
      .set("includeCollections", 1)
      .set("includeExternalMedia", 1)
      .set("sort", "watchlistedAt:desc")
      .set("X-Plex-Container-Start", start)
      .set("X-Plex-Container-Size", size);
    const env = await this.get(DISCOVER, "/library/sections/watchlist/all", params, normalizeEnvelope);
    return env.MediaContainer;
  }

  async addToWatchlist(discoverId: string): Promise<void> {
    return this.watchlistAction("addToWatchlist", discoverId);
  }

  async removeFromWatchlist(discoverId: string): Promise<void> {
    return this.watchlistAction("removeFromWatchlist", discoverId);
  }

  private async watchlistAction(action: string, discoverId: string): Promise<void> {
    const rb = this.http.request(
      "PUT",
      joinUrl(DISCOVER, `/actions/${action}`),
      this._token,
      new Params().set("ratingKey", discoverId),
    );
    return this.http.empty(rb);
  }

  /** Watchlist/view state for a discover item. */
  async userState(discoverId: string): Promise<UserState | null> {
    const v = await this.get(DISCOVER, `/library/metadata/${discoverId}/userState`, new Params(), (x) => x);
    let state = pointer(v, "/MediaContainer/UserState");
    if (state === undefined) return null;
    if (Array.isArray(state)) state = state.length > 0 ? state[state.length - 1] : null;
    try {
      return normalizeUserState(state);
    } catch {
      return null;
    }
  }

  /** Full discover metadata (cast, reviews, ratings) for a discover id. */
  async discoverMetadata(discoverId: string): Promise<Metadata | null> {
    const env = await this.get(METADATA, `/library/metadata/${discoverId}`, new Params(), normalizeEnvelope);
    return env.MediaContainer.Metadata[0] ?? null;
  }

  /** Search Plex's global catalogue (movies & shows not necessarily on any server). */
  async discoverSearch(query: string, limit: number): Promise<Metadata[]> {
    const params = new Params()
      .set("query", query)
      .set("limit", limit)
      .set("searchTypes", "movies,tv")
      .set("searchProviders", "discover")
      .set("includeMetadata", 1);
    const v = await this.get(DISCOVER, "/library/search", params, (x) => x);
    const out: Metadata[] = [];
    const groups = pointer(v, "/MediaContainer/SearchResults");
    if (Array.isArray(groups)) {
      for (const group of groups) {
        const results = typeof group === "object" && group !== null ? (group as Record<string, unknown>).SearchResult : undefined;
        if (!Array.isArray(results)) continue;
        for (const result of results) {
          const m = typeof result === "object" && result !== null ? (result as Record<string, unknown>).Metadata : undefined;
          if (m === undefined) continue;
          try {
            out.push(normalizeMetadata(m));
          } catch {
            // `if let Ok(m) = from_value`: entries that do not decode are skipped
          }
        }
      }
    }
    return out;
  }
}


