// plex.tv models: PIN, account, resources, Home users, friends (port of crates/plex/src/models/tv.rs).

import { boolean, extraOf, list, obj, optI64, optString, optional } from "../de";

/**
 * A PIN used for the plex.tv/link sign-in flow.
 */
export interface Pin {
  id: number | null;
  code: string | null;
  authToken: string | null;
  expiresAt: string | null;
  expiresIn: number | null;
  trusted: boolean;
}

export function normalizePin(v: unknown): Pin {
  const o = obj(v, "Pin");
  return {
    id: optI64(o.id),
    code: optString(o.code),
    authToken: optString(o.authToken),
    expiresAt: optString(o.expiresAt),
    expiresIn: optI64(o.expiresIn),
    trusted: boolean(o.trusted),
  };
}

/** `Pin::default()`, with fields overridden by `over`. */
export function defaultPin(over: Partial<Pin> = {}): Pin {
  return { ...normalizePin({}), ...over };
}

/**
 * The signed-in plex.tv account.
 */
export interface User {
  id: number | null;
  uuid: string | null;
  username: string | null;
  title: string | null;
  friendlyName: string | null;
  email: string | null;
  thumb: string | null;
  authToken: string | null;
  homeAdmin: boolean;
  restricted: boolean;
  subscription: Subscription | null;
}

const USER_KEYS = new Set(["id", "uuid", "username", "title", "friendlyName", "email", "thumb", "authToken", "homeAdmin", "restricted", "subscription"]);

export function normalizeUser(v: unknown): User {
  const o = obj(v, "User");
  return {
    id: optI64(o.id),
    uuid: optString(o.uuid),
    username: optString(o.username),
    title: optString(o.title),
    friendlyName: optString(o.friendlyName),
    email: optString(o.email),
    thumb: optString(o.thumb),
    authToken: optString(o.authToken),
    homeAdmin: boolean(o.homeAdmin),
    restricted: boolean(o.restricted),
    subscription: optional(o.subscription, normalizeSubscription, "subscription"),
    ...extraOf(o, USER_KEYS),
  };
}

/** `User::default()`, with fields overridden by `over`. */
export function defaultUser(over: Partial<User> = {}): User {
  return { ...normalizeUser({}), ...over };
}

export interface Subscription {
  active: boolean;
  status: string | null;
  plan: string | null;
}

export function normalizeSubscription(v: unknown): Subscription {
  const o = obj(v, "Subscription");
  return {
    active: boolean(o.active),
    status: optString(o.status),
    plan: optString(o.plan),
  };
}

/** `Subscription::default()`, with fields overridden by `over`. */
export function defaultSubscription(over: Partial<Subscription> = {}): Subscription {
  return { ...normalizeSubscription({}), ...over };
}

/**
 * A device linked to the account (servers, players) from `/api/v2/resources`.
 */
export interface Resource {
  name: string | null;
  product: string | null;
  productVersion: string | null;
  platform: string | null;
  platformVersion: string | null;
  device: string | null;
  clientIdentifier: string | null;
  createdAt: string | null;
  lastSeenAt: string | null;
  provides: string | null;
  ownerId: number | null;
  sourceTitle: string | null;
  publicAddress: string | null;
  accessToken: string | null;
  owned: boolean;
  home: boolean;
  synced: boolean;
  relay: boolean;
  presence: boolean;
  httpsRequired: boolean;
  publicAddressMatches: boolean;
  connections: Connection[];
}

export function normalizeResource(v: unknown): Resource {
  const o = obj(v, "Resource");
  return {
    name: optString(o.name),
    product: optString(o.product),
    productVersion: optString(o.productVersion),
    platform: optString(o.platform),
    platformVersion: optString(o.platformVersion),
    device: optString(o.device),
    clientIdentifier: optString(o.clientIdentifier),
    createdAt: optString(o.createdAt),
    lastSeenAt: optString(o.lastSeenAt),
    provides: optString(o.provides),
    ownerId: optI64(o.ownerId),
    sourceTitle: optString(o.sourceTitle),
    publicAddress: optString(o.publicAddress),
    accessToken: optString(o.accessToken),
    owned: boolean(o.owned),
    home: boolean(o.home),
    synced: boolean(o.synced),
    relay: boolean(o.relay),
    presence: boolean(o.presence),
    httpsRequired: boolean(o.httpsRequired),
    publicAddressMatches: boolean(o.publicAddressMatches),
    connections: list(o.connections, normalizeConnection, "connections"),
  };
}

/** `Resource::default()`, with fields overridden by `over`. */
export function defaultResource(over: Partial<Resource> = {}): Resource {
  return { ...normalizeResource({}), ...over };
}

export interface Connection {
  protocol: string | null;
  address: string | null;
  port: number | null;
  uri: string | null;
  local: boolean;
  relay: boolean;
  IPv6: boolean;
}

export function normalizeConnection(v: unknown): Connection {
  const o = obj(v, "Connection");
  return {
    protocol: optString(o.protocol),
    address: optString(o.address),
    port: optI64(o.port),
    uri: optString(o.uri),
    local: boolean(o.local),
    relay: boolean(o.relay),
    IPv6: boolean(o.IPv6),
  };
}

/** `Connection::default()`, with fields overridden by `over`. */
export function defaultConnection(over: Partial<Connection> = {}): Connection {
  return { ...normalizeConnection({}), ...over };
}

/**
 * A member of the Plex Home.
 */
export interface HomeUser {
  id: number | null;
  uuid: string | null;
  title: string | null;
  username: string | null;
  email: string | null;
  thumb: string | null;
  restricted: boolean;
  /** Whether switching to this user requires a PIN. */
  protected: boolean;
  admin: boolean;
  guest: boolean;
}

export function normalizeHomeUser(v: unknown): HomeUser {
  const o = obj(v, "HomeUser");
  return {
    id: optI64(o.id),
    uuid: optString(o.uuid),
    title: optString(o.title),
    username: optString(o.username),
    email: optString(o.email),
    thumb: optString(o.thumb),
    restricted: boolean(o.restricted),
    protected: boolean(o.protected),
    admin: boolean(o.admin),
    guest: boolean(o.guest),
  };
}

/** `HomeUser::default()`, with fields overridden by `over`. */
export function defaultHomeUser(over: Partial<HomeUser> = {}): HomeUser {
  return { ...normalizeHomeUser({}), ...over };
}

/**
 * A friend / shared user.
 */
export interface Friend {
  id: number | null;
  uuid: string | null;
  title: string | null;
  username: string | null;
  email: string | null;
  thumb: string | null;
  restricted: boolean;
  home: boolean;
  status: string | null;
}

export function normalizeFriend(v: unknown): Friend {
  const o = obj(v, "Friend");
  return {
    id: optI64(o.id),
    uuid: optString(o.uuid),
    title: optString(o.title),
    username: optString(o.username),
    email: optString(o.email),
    thumb: optString(o.thumb),
    restricted: boolean(o.restricted),
    home: boolean(o.home),
    status: optString(o.status),
  };
}

/** `Friend::default()`, with fields overridden by `over`. */
export function defaultFriend(over: Partial<Friend> = {}): Friend {
  return { ...normalizeFriend({}), ...over };
}

/**
 * Per-user state for a discover item (watchlist membership, views).
 */
export interface UserState {
  watchlistedAt: number | null;
  viewCount: number | null;
  viewOffset: number | null;
  lastViewedAt: number | null;
}

export function normalizeUserState(v: unknown): UserState {
  const o = obj(v, "UserState");
  return {
    watchlistedAt: optI64(o.watchlistedAt),
    viewCount: optI64(o.viewCount),
    viewOffset: optI64(o.viewOffset),
    lastViewedAt: optI64(o.lastViewedAt),
  };
}

/** `UserState::default()`, with fields overridden by `over`. */
export function defaultUserState(over: Partial<UserState> = {}): UserState {
  return { ...normalizeUserState({}), ...over };
}


export namespace User {
  /** Attributes plex.tv sent that are not modelled (serde's flattened `extra` map). */
  export function extra(u: User): Record<string, unknown> {
    return extraOf(u as unknown as Record<string, unknown>, USER_KEYS);
  }

  export function displayName(u: User): string {
    const friendly = u.friendlyName !== null && u.friendlyName !== "" ? u.friendlyName : null;
    return friendly ?? u.title ?? u.username ?? "Plex user";
  }
}

export namespace Resource {
  export function name(r: Resource): string {
    return r.name ?? "Plex Media Server";
  }

  export function isServer(r: Resource): boolean {
    return (r.provides ?? "").split(",").some((p) => p === "server");
  }

  export function id(r: Resource): string {
    return r.clientIdentifier ?? "";
  }
}

export namespace HomeUser {
  export function name(h: HomeUser): string {
    return h.title ?? h.username ?? "User";
  }
}
