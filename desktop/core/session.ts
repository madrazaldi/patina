// Sign-in, server discovery and connection management.
// Port of crates/core/src/session.rs.

import { MediaContainer, PlexError, PlexServer, PlexTv, Resource, User, connectResource } from "../plex";
import * as images from "./images";
import * as downloads from "./downloads";
import * as live from "./live";
import * as offline from "./offline";
import * as player from "./player";
import { errorText, sameServer, serverView, userView } from "./state";
import type { AppState, Session } from "./state";

/** `anyhow::Context`: the outer message, then the cause ("outer: inner"). */
function context(e: unknown, message: string): Error {
  return new Error(`${message}: ${errorText(e)}`);
}

function tvFor(app: AppState, token: string | null): PlexTv {
  return new PlexTv(app.clientInfo(), token);
}

/** Run a sign-in step, showing its error on the failure screen. */
export async function run(app: AppState, fut: Promise<void>): Promise<void> {
  try {
    await fut;
  } catch (e) {
    app.setStage({ kind: "failed", message: errorText(e) });
  }
}

/** Restore the previous session on startup (or after changing accounts). */
export async function restore(app: AppState): Promise<void> {
  const settings = app.settings();
  const { manual_url: url, manual_token: manualToken } = settings.server;
  if (url !== null && manualToken !== null) {
    app.setStage({ kind: "connecting", name: url });
    await run(app, connectManual(app, url, manualToken));
    return;
  }
  const token = settings.auth.token;
  if (token === null) {
    app.setStage({ kind: "login" });
    return;
  }
  await run(app, signIn(app, token));
}

/** Try the saved session again (from the failure screen). */
export async function retry(app: AppState): Promise<void> {
  app.setStage({ kind: "booting" });
  await restore(app);
}

/** Validate a plex.tv token and connect to the preferred server. */
export async function signIn(app: AppState, token: string): Promise<void> {
  app.setStage({ kind: "connecting", name: "plex.tv" });
  const tv = tvFor(app, token);
  let user: User;
  try {
    user = await tv.user();
  } catch (e) {
    if (e instanceof PlexError && e.kind === "unauthorized") {
      app.updateSettings((s) => {
        s.auth.token = null;
      });
      app.setStage({ kind: "login" });
      return;
    }
    throw context(e, "reaching plex.tv");
  }
  app.updateSettings((s) => {
    s.auth.token = token;
  });
  let servers: Resource[];
  try {
    servers = await tv.servers();
  } catch (e) {
    throw context(e, "listing servers");
  }
  if (servers.length === 0) {
    throw new Error(`No Plex Media Servers are available to ${User.displayName(user)}.`);
  }
  const preferred = app.settings().server.id;
  const choice =
    (preferred !== null ? servers.find((s) => Resource.id(s) === preferred) : undefined) ?? (servers.length === 1 ? servers[0] : undefined);
  if (choice) {
    await connect(app, tv, user, choice, servers);
  } else {
    app.setStage({ kind: "pickServer", servers: servers.map(serverView), user: userView(user) });
    app.pending = { tv, user, servers };
  }
}

/** Connect to a server offered on the "choose a server" screen. */
export async function pickServer(app: AppState, id: string): Promise<void> {
  const pending = app.pending;
  if (!pending) throw new Error("No server list to pick from.");
  const resource = pending.servers.find((s) => Resource.id(s) === id);
  if (!resource) throw new Error("That server is no longer available.");
  await connect(app, pending.tv, pending.user, resource, pending.servers);
}

/** Connect to one server resource and make it the active session. */
export async function connect(app: AppState, tv: PlexTv, user: User, resource: Resource, servers: Resource[]): Promise<void> {
  const name = Resource.name(resource);
  app.setStage({ kind: "connecting", name });
  let server: PlexServer;
  try {
    server = await connectResource(tv, resource);
  } catch (e) {
    throw context(e, `connecting to ${name}`);
  }
  app.updateSettings((s) => {
    s.server.id = Resource.id(resource);
    s.server.last_uri = String(server.baseUrl());
  });
  await activate(app, {
    tv,
    serverName: name,
    serverId: Resource.id(resource),
    owned: resource.owned,
    user,
    server,
    servers,
  });
}

async function connectManual(app: AppState, url: string, token: string): Promise<void> {
  let server = new PlexServer(app.clientInfo(), url, token);
  let id;
  try {
    id = await server.identity();
  } catch (e) {
    throw context(e, "contacting server");
  }
  server = server.withMachineIdentifier(id.machineIdentifier);
  let caps;
  try {
    caps = await server.capabilities();
  } catch (e) {
    throw context(e, "server rejected the token");
  }
  const name = MediaContainer.attr(caps, "friendlyName") ?? url;
  // A server token usually also works against plex.tv (watchlist, users).
  let tv: PlexTv | null = null;
  let user: User | null = null;
  try {
    tv = tvFor(app, token);
    user = await tv.user();
  } catch {
    user = null;
  }
  await activate(app, {
    tv: user ? tv : null,
    serverName: name,
    serverId: id.machineIdentifier ?? "",
    owned: true,
    user,
    server,
    servers: [],
  });
}

/**
 * Save a manual server and connect to it. Errors are returned (and shown on
 * the form) rather than sending the app to the failure screen.
 */
export async function signInManual(app: AppState, url: string, token: string): Promise<void> {
  const trimmed = url.trim();
  const tok = token.trim();
  const full = trimmed.startsWith("http") ? trimmed : `http://${trimmed}`;
  await connectManual(app, full, tok);
  app.updateSettings((s) => {
    s.server.manual_url = full;
    s.server.manual_token = tok;
  });
}

async function activate(app: AppState, session: Session): Promise<void> {
  const account = offline.account(session);
  let sections;
  try {
    sections = await session.server.sections();
  } catch (e) {
    throw context(e, "loading libraries");
  }
  app.updateSettings((s) => {
    s.auth.account_id = account;
  });
  app.setConnected(true);
  images.setServer(session.server);
  app.pending = null;
  app.setSession(
    session,
    sections.filter((d) => !d.hidden),
  );
  app.setStage({ kind: "ready" });
  app.bump();
  live.watch(app);
  downloads.resume(app);
}

export async function reloadSections(app: AppState): Promise<void> {
  const server = app.server();
  const sections = await server.sections();
  const current = app.session();
  if (current && sameServer(current.server, server)) {
    app.setConnected(true);
    app.setSections(sections.filter((d) => !d.hidden));
  }
}

/** Switch to another server of the same account. */
export async function switchServer(app: AppState, id: string): Promise<void> {
  const session = app.session();
  if (!session) throw new Error("Not connected.");
  if (!session.tv || !session.user) throw new Error("Switching servers needs a plex.tv sign-in.");
  const resource = session.servers.find((s) => Resource.id(s) === id);
  if (!resource) throw new Error("Unknown server.");
  try {
    await connect(app, session.tv, session.user, resource, session.servers);
  } catch (e) {
    // Stay on the current server.
    app.setStage({ kind: "ready" });
    throw e;
  }
}

/** Switch Plex Home user; the new user's token replaces the stored one. */
export async function switchUser(app: AppState, uuid: string, pin?: string | null): Promise<void> {
  const tv = app.tv();
  if (!tv) throw new Error("Not signed in to plex.tv.");
  let user: User;
  try {
    user = await tv.switchHomeUser(uuid, pin ?? null);
  } catch (e) {
    throw context(e, "switching user");
  }
  const token = user.authToken;
  if (token === null) throw new Error("plex.tv returned no token");
  app.updateSettings((s) => {
    s.auth.account_id = user.uuid ?? offline.digest(token);
  });
  // The previous user's listener must not outlive their session, even if signing in as the new one fails.
  live.stop(app);
  app.setSession(null, []);
  await run(app, signIn(app, token));
}

export function signOut(app: AppState): void {
  player.control({ kind: "stop" });
  live.stop(app);
  app.updateSettings((s) => {
    s.auth.token = null;
    s.auth.account_id = null;
    s.server = { id: null, last_uri: null, manual_url: null, manual_token: null };
  });
  images.setServer(null);
  app.pending = null;
  app.setSession(null, []);
  app.setStage({ kind: "login" });
}

export interface NewPin {
  id: number;
  code: string;
  url: string;
  attempt: number;
}

/**
 * Start a PIN sign-in. `strong` PINs are approved in the browser; short
 * ones are typed in at plex.tv/link.
 */
export async function createPin(app: AppState, strong: boolean): Promise<NewPin> {
  const tv = tvFor(app, null);
  let pin;
  try {
    pin = await tv.createPin(strong);
  } catch (e) {
    throw context(e, "contacting plex.tv");
  }
  const code = pin.code ?? "";
  const url = tv.authUrl(code);
  app.loginAttempt += 1;
  const attempt = app.loginAttempt;
  if (pin.id === null) throw new Error("plex.tv returned no PIN");
  return { id: pin.id, code, url, attempt };
}

/**
 * Poll a PIN until it is claimed (or 15 minutes pass), then sign in.
 * Returns early without signing in when the attempt is cancelled.
 */
export async function completePin(app: AppState, pinId: number, attempt: number): Promise<void> {
  const tv = tvFor(app, null);
  const current = () => app.loginAttempt === attempt;
  for (let i = 0; i < 450; i++) {
    await Bun.sleep(2000);
    if (!current()) return;
    try {
      const pin = await tv.checkPin(pinId);
      if (pin.authToken) {
        if (current()) await run(app, signIn(app, pin.authToken));
        return;
      }
    } catch {
      // Keep polling.
    }
  }
  throw new Error("The sign-in code expired. Please try again.");
}

export function cancelPin(app: AppState): void {
  app.loginAttempt += 1;
}

/** Sign in with an email or username and password; `code` is the 2FA code, if any. */
export async function passwordSignIn(app: AppState, login: string, password: string, code?: string | null): Promise<void> {
  const trimmed = code?.trim();
  const twoFactor = trimmed ? trimmed : null;
  const tv = tvFor(app, null);
  let user: User;
  try {
    user = await tv.signIn(login, password, twoFactor);
  } catch (e) {
    if (e instanceof PlexError && e.status === 401) throw new Error("Incorrect email/username or password.");
    throw e;
  }
  const token = user.authToken;
  if (token === null) throw new Error("plex.tv returned no token");
  await run(app, signIn(app, token));
}
