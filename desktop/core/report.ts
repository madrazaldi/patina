// An ordered worker for everything the player tells Plex.
//
// Timeline reports, scrobbles, stream choices and play-queue edits used to be
// awaited while handling mpv's events, so one slow or failing request (the
// shared HTTP timeout is 60 seconds) delayed queue edits and playback
// updates. The player now only enqueues an `Op`; one task sends them in
// order and keeps the backlog small while Plex is slow:
//
// * progress checkpoints for the same item collapse into the newest one;
// * the final `stopped` report replaces the pending progress it supersedes and
//   is never dropped (it, scrobbles and stream choices are retried);
// * a progress report is never sent after a newer report of the same item was
//   queued, and requests that hang are cut off after a timeout.

import { Channel } from "../mpv/channel";
import type { PlexServer, TimelineUpdate } from "../plex";
import { logger } from "./log";

const log = logger("report");

/** Edits mirrored to the server's play queue. */
export type QueueOp =
  | { kind: "remove"; queue: number; item: number }
  | { kind: "move"; queue: number; item: number; after: number | null }
  | { kind: "shuffle"; queue: number }
  | { kind: "unshuffle"; queue: number };

/** One thing to tell the server. */
export type Op =
  | { kind: "timeline"; update: TimelineUpdate }
  | { kind: "scrobble"; ratingKey: string }
  | { kind: "streams"; partId: number; audio: number | null; subtitle: number | null }
  | { kind: "playQueue"; op: QueueOp }
  | { kind: "pingTranscode"; session: string }
  | { kind: "stopTranscode"; session: string };

/** The item a timeline report or scrobble is about. */
function subject(op: Op): string | null {
  switch (op.kind) {
    case "timeline":
      return op.update.ratingKey;
    case "scrobble":
      return op.ratingKey;
    default:
      return null;
  }
}

export function isProgress(op: Op): boolean {
  return op.kind === "timeline" && op.update.state !== "stopped";
}

/** Worth retrying: losing it would leave the server with a wrong resume point or watch state. */
function isImportant(op: Op): boolean {
  switch (op.kind) {
    case "timeline":
      return op.update.state === "stopped";
    case "scrobble":
    case "streams":
    case "playQueue":
    case "stopTranscode":
      return true;
    case "pingTranscode":
      return false;
  }
}

/** An `Op` for one server (`T`; a `PlexServer` in the app). */
export interface Job<T> {
  target: T;
  op: Op;
  attempts: number;
  /** `Date.now()` ms before which the job must not be tried again. */
  notBefore: number | null;
}

/** The backlog, with coalescing applied as jobs are added. */
export class Pending<T> {
  private jobs: Job<T>[] = [];

  /** `eq` is `PartialEq` on the target. */
  constructor(private readonly eq: (a: T, b: T) => boolean = (a, b) => a === b) {}

  get length(): number {
    return this.jobs.length;
  }

  isEmpty(): boolean {
    return this.jobs.length === 0;
  }

  ops(): Op[] {
    return this.jobs.map((j) => j.op);
  }

  push(target: T, op: Op): void {
    const job: Job<T> = { target, op, attempts: 0, notBefore: null };
    switch (op.kind) {
      case "timeline": {
        // Fold into the last pending job about this item when that is
        // progress: only the newest checkpoint (or the final stop) matters.
        let last = -1;
        for (let i = this.jobs.length - 1; i >= 0; i--) {
          const j = this.jobs[i]!;
          if (this.eq(j.target, target) && subject(j.op) === subject(op)) {
            last = i;
            break;
          }
        }
        if (last >= 0 && isProgress(this.jobs[last]!.op)) {
          this.jobs[last] = { ...job, notBefore: this.jobs[last]!.notBefore };
        } else {
          this.jobs.push(job);
        }
        break;
      }
      case "streams": {
        const same = this.jobs.find((j) => this.eq(j.target, target) && j.op.kind === "streams" && j.op.partId === op.partId);
        if (same && same.op.kind === "streams") {
          same.op = { ...same.op, audio: op.audio ?? same.op.audio, subtitle: op.subtitle ?? same.op.subtitle };
        } else {
          this.jobs.push(job);
        }
        break;
      }
      case "pingTranscode": {
        const queued = this.jobs.some(
          (j) => this.eq(j.target, target) && (j.op.kind === "pingTranscode" || j.op.kind === "stopTranscode") && j.op.session === op.session,
        );
        if (!queued) this.jobs.push(job);
        break;
      }
      case "stopTranscode":
        this.jobs = this.jobs.filter((j) => !(this.eq(j.target, target) && j.op.kind === "pingTranscode" && j.op.session === op.session));
        this.jobs.push(job);
        break;
      case "scrobble":
      case "playQueue":
        this.jobs.push(job);
        break;
    }
  }

  pop(): Job<T> | undefined {
    return this.jobs.shift();
  }

  /** Requeue a failed job at the front, to keep the order it was sent in. */
  retry(job: Job<T>, delayMs: number): void {
    job.attempts += 1;
    job.notBefore = Date.now() + delayMs;
    this.jobs.unshift(job);
  }

  nextReady(): number | null {
    return this.jobs[0]?.notBefore ?? null;
  }
}

/** How the worker reaches the server: resolves when delivered, rejects with the reason otherwise. */
export interface Sink<T> {
  send(target: T, op: Op): Promise<void>;
}

/** Tuning, so tests can run quickly. */
export interface Config {
  /** Longest a single request may take before it is abandoned (ms). */
  timeoutMs: number;
  /** Attempts made at an important job (the first included). */
  attempts: number;
  backoffMs: number;
}

export function defaultConfig(): Config {
  return { timeoutMs: 10_000, attempts: 4, backoffMs: 2000 };
}

type Msg<T> = { kind: "job"; target: T; op: Op } | { kind: "flush"; done: () => void };

/** The sending side. Cheap to use from anywhere; never blocks. */
export class Reporter<T> {
  private constructor(private readonly tx: Channel<Msg<T>>) {}

  /** Start a worker. */
  static spawn<T>(sink: Sink<T>, config: Config = defaultConfig(), eq?: (a: T, b: T) => boolean): Reporter<T> {
    const tx = new Channel<Msg<T>>();
    void worker(tx, sink, config, eq).catch((e) => log.warn(`report worker stopped: ${e instanceof Error ? e.message : String(e)}`));
    return new Reporter(tx);
  }

  send(target: T, op: Op): void {
    this.tx.send({ kind: "job", target, op });
  }

  /** Wait until everything sent so far was delivered or given up on; false if `timeoutMs` came first. */
  async flush(timeoutMs: number): Promise<boolean> {
    let done!: () => void;
    const finished = new Promise<true>((resolve) => {
      done = () => resolve(true);
    });
    if (!this.tx.send({ kind: "flush", done })) return true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const timeout = new Promise<false>((resolve) => {
      timer = setTimeout(() => resolve(false), timeoutMs);
    });
    const result = await Promise.race([finished, timeout]);
    clearTimeout(timer);
    return result;
  }

  /** Stop the worker once it has handled what was sent (dropping the last sender in Rust). */
  close(): void {
    this.tx.close();
  }
}

async function worker<T>(rx: Channel<Msg<T>>, sink: Sink<T>, config: Config, eq?: (a: T, b: T) => boolean): Promise<void> {
  const pending = new Pending<T>(eq);
  let waiters: (() => void)[] = [];
  let closed = false;
  const take = (m: Msg<T>) => {
    if (m.kind === "job") pending.push(m.target, m.op);
    else waiters.push(m.done);
  };
  for (;;) {
    // Take in everything that arrived while the last request was running.
    for (;;) {
      const m = rx.tryRecv();
      if (m === undefined) break;
      take(m);
    }
    if (rx.closed) closed = true;
    if (pending.isEmpty()) {
      const done = waiters;
      waiters = [];
      for (const w of done) w();
      if (closed) return;
      const m = await rx.recv();
      if (m === null) return;
      take(m);
      continue;
    }
    // A retried job waits out its backoff, still absorbing new jobs.
    const at = pending.nextReady();
    if (at !== null && at > Date.now()) {
      if (closed) {
        await Bun.sleep(at - Date.now());
      } else {
        const r = await rx.recvTimeout(at - Date.now());
        if (!r.timeout) {
          if (r.value === null) closed = true;
          else take(r.value);
        }
        continue;
      }
    }
    const job = pending.pop();
    if (!job) continue;
    let error: string;
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
      const timeout = new Promise<"timeout">((resolve) => {
        timer = setTimeout(() => resolve("timeout"), config.timeoutMs);
      });
      const result = await Promise.race([sink.send(job.target, job.op).then(() => "ok" as const), timeout]);
      if (result === "ok") continue;
      error = `no answer within ${config.timeoutMs / 1000}s`;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      clearTimeout(timer);
    }
    if (isImportant(job.op) && job.attempts + 1 < config.attempts) {
      log.debug(`plex report failed, will retry: ${error}`);
      pending.retry(job, config.backoffMs * 2 ** job.attempts);
    } else {
      log.debug(`plex report dropped: ${error}`);
    }
  }
}

/** The app's sink: talks to the Plex server a job was queued for. */
const plexSink: Sink<PlexServer> = {
  async send(server, op) {
    switch (op.kind) {
      case "timeline":
        return server.timeline(op.update);
      case "scrobble":
        return server.scrobble(op.ratingKey);
      case "streams":
        return server.selectStreams(op.partId, op.audio, op.subtitle, true);
      case "playQueue": {
        const q = op.op;
        switch (q.kind) {
          case "remove":
            await server.removeFromPlayQueue(q.queue, q.item);
            return;
          case "move":
            await server.movePlayQueueItem(q.queue, q.item, q.after);
            return;
          case "shuffle":
            await server.shufflePlayQueue(q.queue);
            return;
          case "unshuffle":
            await server.unshufflePlayQueue(q.queue);
            return;
        }
        return;
      }
      case "pingTranscode":
        return server.pingTranscode(op.session);
      case "stopTranscode":
        return server.stopTranscode(op.session);
    }
  },
};

/** `PlexServer: PartialEq`: same base URL and token (kept here so this module stays light). */
function sameServer(a: PlexServer, b: PlexServer): boolean {
  return a === b || a.equals(b);
}

let REPORTER: Reporter<PlexServer> | null = null;

/** Wait for the app-wide reporter to deliver what it has (nothing to do if it never started). */
export async function flush(timeoutMs: number): Promise<boolean> {
  return REPORTER ? REPORTER.flush(timeoutMs) : true;
}

/**
 * The app-wide reporter. One worker for all playback keeps reports in order
 * across consecutive playbacks (a retry never overtakes the stop before it).
 */
export function reporter(): Reporter<PlexServer> {
  REPORTER ??= Reporter.spawn(plexSink, defaultConfig(), sameServer);
  return REPORTER;
}
