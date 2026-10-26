// Picking the best connection of a server resource (port of crates/plex/src/connect.rs).

import { logger } from "../core/log";
import { PlexError } from "./error";
import { Resource } from "./models";
import { PlexServer } from "./server";
import type { PlexTv } from "./tv";

const log = logger("plex");

/** A candidate URI for reaching a server, ranked by preference. */
export class ConnectionCandidate {
  uri: string;
  local: boolean;
  relay: boolean;
  /** Lower is better. */
  rank: number;

  constructor(uri: string, local: boolean, relay: boolean, rank: number) {
    this.uri = uri;
    this.local = local;
    this.relay = relay;
    this.rank = rank;
  }

  static candidates(resource: Resource): ConnectionCandidate[] {
    const out: ConnectionCandidate[] = [];
    const push = (uri: string, local: boolean, relay: boolean, rank: number): void => {
      if (!out.some((c) => c.uri === uri)) out.push(new ConnectionCandidate(uri, local, relay, rank));
    };
    for (const c of resource.connections) {
      const uri = c.uri;
      if (uri === null) continue;
      const https = uri.startsWith("https");
      let rank: number;
      if (c.relay) rank = 5;
      else if (c.local) rank = https ? 0 : 1;
      else rank = https ? 2 : 3;
      push(uri, c.local, c.relay, rank);
      // plex.direct names need working DNS rebinding; offer the raw IP too.
      if (c.local && !c.relay && !resource.httpsRequired && c.address !== null && c.port !== null) {
        const host = c.IPv6 ? `[${c.address}]` : c.address;
        push(`http://${host}:${c.port}`, true, false, 4);
      }
    }
    out.sort((a, b) => a.rank - b.rank);
    return out;
  }
}

interface Attempt {
  rank: number;
  server: PlexServer | null;
  error: PlexError | Error | null;
}

/**
 * Race every connection of a server resource and return a client bound to
 * the best one that answers. Local direct connections win over remote, and
 * relays are the last resort.
 */
export async function connectResource(tv: PlexTv, resource: Resource): Promise<PlexServer> {
  const token = resource.accessToken ?? tv.token();
  if (token === null) throw PlexError.unauthorized();
  const candidates = ConnectionCandidate.candidates(resource);
  if (candidates.length === 0) throw PlexError.noConnection(Resource.name(resource));

  // Dropping the pending futures cancels them in Rust; here the leftovers are aborted.
  const aborts: AbortController[] = [];
  const pending = new Map<number, Promise<{ id: number; result: Attempt }>>();
  candidates.forEach((c, id) => {
    const ac = new AbortController();
    aborts.push(ac);
    const attempt = (async (): Promise<Attempt> => {
      const rank = c.rank;
      let timer: ReturnType<typeof setTimeout> | undefined;
      let timedOut = false;
      try {
        const server = PlexServer.withHttp(tv.http, c.uri, token);
        timer = setTimeout(() => {
          timedOut = true;
          ac.abort();
        }, 6000);
        const identity = await server.identity(ac.signal);
        return { rank, server: server.withMachineIdentifier(identity.machineIdentifier), error: null };
      } catch (e) {
        if (timedOut) return { rank, server: null, error: PlexError.other("timed out") };
        return { rank, server: null, error: e instanceof Error ? e : new Error(String(e)) };
      } finally {
        if (timer !== undefined) clearTimeout(timer);
      }
    })();
    pending.set(
      id,
      attempt.then((result) => ({ id, result })),
    );
  });

  const bestRank = candidates[0]!.rank;
  let best: { rank: number; server: PlexServer } | null = null;
  let firstSuccess: number | null = null;
  let lastErr: Error | null = null;
  try {
    for (;;) {
      if (pending.size === 0) break;
      // Once something works, give better-ranked candidates a short grace period.
      const grace = firstSuccess === null ? null : Math.max(0, 1200 - (Date.now() - firstSuccess));
      const next = Promise.race(pending.values());
      let done: { id: number; result: Attempt } | null;
      if (grace === null) {
        done = await next;
      } else {
        let timer: ReturnType<typeof setTimeout> | undefined;
        const timeout = new Promise<null>((resolve) => {
          timer = setTimeout(() => resolve(null), grace);
        });
        try {
          done = await Promise.race([next, timeout]);
        } finally {
          clearTimeout(timer);
        }
        if (done === null) break;
      }
      pending.delete(done.id);
      const { rank, server, error } = done.result;
      if (server !== null) {
        firstSuccess ??= Date.now();
        if (best === null || rank < best.rank) best = { rank, server };
        if (rank === bestRank) break;
      } else {
        log.debug(`connection attempt failed: ${error?.message ?? ""}`);
        lastErr = error;
      }
    }
  } finally {
    for (const ac of aborts) ac.abort();
  }
  if (best !== null) return best.server;
  if (lastErr instanceof PlexError && lastErr.kind === "unauthorized") throw PlexError.unauthorized();
  throw PlexError.noConnection(Resource.name(resource));
}
