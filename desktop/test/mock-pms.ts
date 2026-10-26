// Starts tools/mock_pms.py on a free port for integration tests. Everything the
// mock logs to stdout (timeline, scrobble, range requests...) goes to `eventsPath`.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const SCRIPT = resolve(import.meta.dir, "../../tools/mock_pms.py");

/** Whether `python3` can be run (integration tests are skipped without it). */
export function hasPython(): boolean {
  return Bun.which("python3") !== null && existsSync(SCRIPT);
}

export interface MockPms {
  /** Base URL such as `http://127.0.0.1:41234`. */
  url: string;
  /** Path of the file receiving the mock's stdout (its request log). */
  eventsPath: string;
  /** What the mock printed to stderr after its startup line. */
  log(): string;
  /** The request log written so far. */
  events(): string;
  /** Kill the process and wait for it to exit. */
  stop(): Promise<void>;
}

const running = new Set<{ kill(): void }>();
process.on("exit", () => {
  for (const p of running) {
    try {
      p.kill();
    } catch {
      // already gone
    }
  }
});

/**
 * Start the mock server with `--port 0 <args> --media <media>` and wait for its
 * startup line on stderr. `dir` receives the events log.
 */
export async function startMockPms(opts: { media: string; dir: string; args?: string[] }): Promise<MockPms> {
  mkdirSync(opts.media, { recursive: true });
  return startMock(SCRIPT, "mock PMS on ", [...(opts.args ?? []), "--media", opts.media], opts.dir);
}

/** Run a mock server script with `--port 0 <args>` and wait for its `<prefix><url>` line on stderr. */
export async function startMock(script: string, prefix: string, args: string[], dir: string): Promise<MockPms> {
  mkdirSync(dir, { recursive: true });
  const eventsPath = join(dir, "events.log");
  writeFileSync(eventsPath, "");
  const proc = Bun.spawn(["python3", "-I", script, "--port", "0", ...args], {
    stdin: "ignore",
    stdout: Bun.file(eventsPath),
    stderr: "pipe",
  });
  running.add(proc);
  let log = "";
  const reader = proc.stderr.getReader();
  const decoder = new TextDecoder();
  let first = "";
  while (!first.includes("\n")) {
    const { value, done } = await reader.read();
    if (done) break;
    first += decoder.decode(value, { stream: true });
  }
  const line = first.split("\n")[0].trim();
  if (!line.startsWith(prefix)) {
    proc.kill();
    running.delete(proc);
    throw new Error(`mock server startup failed: ${first}`);
  }
  log = first.slice(first.indexOf("\n") + 1);
  // Keep draining stderr so the mock never blocks on a full pipe.
  const drain = (async () => {
    for (;;) {
      const { value, done } = await reader.read();
      if (done) break;
      log += decoder.decode(value, { stream: true });
    }
  })().catch(() => {});
  return {
    url: line.slice(prefix.length),
    eventsPath,
    log: () => log,
    events: () => {
      try {
        return readFileSync(eventsPath, "utf8");
      } catch {
        return "";
      }
    },
    async stop() {
      proc.kill();
      await proc.exited;
      await drain;
      running.delete(proc);
    },
  };
}
