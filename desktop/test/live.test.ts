// Listener lifecycle against fake servers that hold their notification stream open.
// Port of crates/core/tests/live.rs. The Rust test runs its worker in a subprocess with its
// own config dir so the settings written by `AppState::new` stay isolated; here the config
// dir is a temp dir for the duration of the test.
import { expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { ClientInfo, PlexServer } from "../plex";
import * as live from "../core/live";
import { AppState, NullSink } from "../core/state";
import type { Session } from "../core/state";

interface Fake {
  url: string;
  /** Notification streams currently connected. */
  open: { count: number };
  stop(): Promise<void>;
}

/**
 * A server that answers ordinary requests with an empty container and holds
 * `/:/eventsource/notifications` open until the client hangs up.
 */
function fakeServer(): Fake {
  const open = { count: 0 };
  const server = Bun.serve({
    port: 0,
    fetch(req) {
      const path = new URL(req.url).pathname;
      if (path.includes("/:/eventsource/notifications")) {
        let counted = false;
        const body = new ReadableStream<Uint8Array>({
          start(controller) {
            open.count += 1;
            counted = true;
            // Parked like a real server between events; the comment line flushes the headers.
            controller.enqueue(new TextEncoder().encode(":\n\n"));
          },
          cancel() {
            if (counted) open.count -= 1;
            counted = false;
          },
        });
        // Client hang-up (abort) also surfaces on the request signal.
        req.signal.addEventListener("abort", () => {
          if (counted) open.count -= 1;
          counted = false;
        });
        return new Response(body, { headers: { "Content-Type": "text/event-stream" } });
      }
      return Response.json({ MediaContainer: { size: 0 } });
    },
  });
  return { url: `http://127.0.0.1:${server.port}`, open, stop: async () => void (await server.stop(true)) };
}

async function until(what: string, done: () => boolean): Promise<void> {
  const start = Date.now();
  while (!done()) {
    if (Date.now() - start > 10_000) throw new Error(`timed out waiting for ${what}`);
    await Bun.sleep(20);
  }
}

const session = (url: string): Session => ({
  tv: null,
  server: new PlexServer(new ClientInfo("test", "Patina", "test"), url, "token"),
  serverId: url,
  serverName: "Fake",
  user: null,
  owned: true,
  servers: [],
});

test("repeated server switches leave one listener", async () => {
  const config = mkdtempSync(join(tmpdir(), "patina-live-"));
  const previous = process.env.XDG_CONFIG_HOME;
  process.env.XDG_CONFIG_HOME = join(config, "config");
  const app = new AppState(NullSink);
  const servers = [fakeServer(), fakeServer(), fakeServer()];
  try {
    const total = () => servers.reduce((n, s) => n + s.open.count, 0);

    // Switch between servers over and over, as the session flow does: new session, then a fresh listener.
    for (let i = 0; i < 12; i++) {
      const fake = servers[i % servers.length]!;
      app.setSession(session(fake.url), []);
      live.watch(app);
      await until("the new listener to connect", () => fake.open.count === 1);
    }
    const last = servers[11 % servers.length]!;
    expect(live.listening(app)).toBe(true);
    // Every earlier listener closed its connection; only the current server has one.
    await until("old listeners to hang up", () => total() === 1);
    expect(last.open.count).toBe(1);

    // Signing out leaves none.
    live.stop(app);
    await until("the listener to hang up", () => total() === 0);
    expect(live.listening(app)).toBe(false);

    // Replacing a listener that is waiting out a backoff after its server went away also leaves one.
    await Promise.all(servers.map((s) => s.stop()));
    app.setSession(session("http://127.0.0.1:9"), []);
    live.watch(app);
    await Bun.sleep(100);
    live.watch(app);
    live.stop(app);
    await Bun.sleep(100);
    expect(live.listening(app)).toBe(false);
  } finally {
    app.dispose();
    await Promise.all(servers.map((s) => s.stop()));
    if (previous === undefined) delete process.env.XDG_CONFIG_HOME;
    else process.env.XDG_CONFIG_HOME = previous;
    rmSync(config, { recursive: true, force: true });
  }
}, 30_000);
