// The other servers of the signed-in account: connecting to them on demand and
// keeping those connections, so items found on a second server keep being
// browsed, played and reported against that server (never the current one).
// Port of crates/core/src/servers.rs.

import { Metadata as M, Resource, connectResource } from "../plex";
import type { Metadata, PlexServer } from "../plex";
import * as offline from "./offline";
import type { AppState } from "./state";

/** How long a connection to another server is reused before it is raced again. */
const REUSE_MS = 15 * 60 * 1000;

const CONNECTIONS = new Map<string, { at: number; server: PlexServer }>();
// (account, machine id) pairs as one key.
const keyOf = (account: string, id: string) => JSON.stringify([account, id]);

/** Whether `id` names the session's own server (or nothing at all). */
export function isCurrent(app: AppState, id?: string | null): boolean {
  if (id == null || id === "") return true;
  const s = app.session();
  if (!s) return false;
  return id === s.serverId || s.server.machineIdentifier() === id;
}

/**
 * The server with machine identifier `id`: the session's own, or another server of
 * the account, connected to on first use. No id means the current server.
 */
export async function serverFor(app: AppState, id?: string | null): Promise<PlexServer> {
  if (isCurrent(app, id)) return app.server();
  const machine = id ?? "";
  const session = app.session();
  if (!session) throw new Error("Not connected to a server.");
  const key = keyOf(offline.account(session), machine);
  const known = CONNECTIONS.get(key);
  if (known && Date.now() - known.at < REUSE_MS) return known.server;
  const tv = session.tv;
  if (!tv) throw new Error("Other servers need a plex.tv sign-in.");
  const resource = session.servers.find((r) => Resource.id(r) === machine);
  if (!resource) throw new Error("That server is no longer on this account.");
  let server: PlexServer;
  try {
    server = await connectResource(tv, resource);
  } catch (e) {
    throw new Error(`connecting to ${Resource.name(resource)}: ${(e as Error).message}`);
  }
  CONNECTIONS.set(key, { at: Date.now(), server });
  return server;
}

/** The server an item lives on: its own tag, else the current one. */
export function serverOf(app: AppState, item: Metadata): Promise<PlexServer> {
  return serverFor(app, M.serverId(item));
}

/** Forget a connection that stopped working so the next use connects again. */
export function forget(app: AppState, id: string): void {
  const session = app.session();
  if (session) CONNECTIONS.delete(keyOf(offline.account(session), id));
}

/** Tag items with the server they came from, leaving the current server's items alone. */
export function tag(app: AppState, items: Metadata[], server?: string | null): void {
  const s = server && !isCurrent(app, server) ? server : null;
  for (const item of items) tagOne(item, s);
}

/** Tag an item and the items of its related hubs (shown on its page). */
function tagOne(item: Metadata, server: string | null): void {
  M.setServerId(item, server);
  for (const hub of item.Related?.Hub ?? []) {
    for (const m of hub.Metadata) M.setServerId(m, server);
  }
}

/** A server's display name: the account's resource name, or the session's own. */
export function nameOf(app: AppState, id?: string | null): string {
  const session = app.session();
  if (!session) return "";
  if (isCurrent(app, id)) return session.serverName;
  const r = session.servers.find((r) => Resource.id(r) === id);
  return r ? Resource.name(r) : "";
}

/**
 * Artwork of an item on another server, resized by that server. The webview's own
 * cache keeps it, so it is not written to the on-disk artwork cache.
 */
export async function image(app: AppState, id: string, source: string, w: number, h: number): Promise<[Uint8Array, string] | null> {
  if (source === "") return null;
  try {
    const server = await serverFor(app, id);
    const [bytes, ctype] = await server.image(source, w, h);
    if (bytes.length === 0) return null;
    return [bytes, ctype && ctype.startsWith("image/") ? ctype : "image/jpeg"];
  } catch {
    return null;
  }
}
