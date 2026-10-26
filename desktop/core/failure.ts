// Playback failures: why mpv (or the setup before it) gave up, and what can be tried next.
//
// The player reports a failure as one `PlaybackFailure` event carrying a
// readable reason and the actions that make sense for it. Front ends offer
// them and answer with `recover`. The request needed to try again stays
// here, so a front end only sends the choice.

import { Metadata, PlexError } from "../plex";
import { QUALITIES, quality as findQuality } from "./config";
import type { Quality } from "./config";
import { logger } from "./log";
import * as player from "./player";
import type { PlayRequest } from "./player";
import { Scope } from "./recovery";
import type { AppState, Session } from "./state";

const log = logger("failure");

export type FailureKind =
  /** The stream could not be loaded or was cut off: the connection or the server. */
  | "network"
  /** mpv could not make sense of the file. */
  | "format"
  /** mpv could not open an audio or video device. */
  | "output"
  | "other";

/** Whether playback never got going or broke after it had. */
export type Phase = "start" | "playing";

export interface QualityChoice {
  id: string;
  label: string;
}

export interface PlaybackFailure {
  /** Distinguishes this failure from earlier ones. */
  id: number;
  item: Metadata;
  /** A sentence for people. */
  reason: string;
  /** What mpv (or the underlying error) said. */
  detail: string | null;
  kind: FailureKind;
  phase: Phase;
  /** Where trying again resumes. */
  positionMs: number;
  /** The version (media) that failed. */
  mediaIndex: number;
  /** The quality preset that was in use. */
  quality: string;
  /** The next step down, when transcoding more aggressively is an option. */
  lowerQuality: QualityChoice | null;
  canRetry: boolean;
  /** Items after the failed one that stay queued. */
  remaining: number;
}

/** What to do about a failure. */
export type Recover =
  /** Try again from where it stopped. */
  | { kind: "retry" }
  /** Try another version (media) of the item. */
  | { kind: "version"; index: number }
  /** Transcode at the next lower quality. */
  | { kind: "lowerQuality" };

interface Failed {
  failure: PlaybackFailure;
  request: PlayRequest;
  /** The account and server it failed on: its rating keys mean nothing elsewhere. */
  scope: Scope | null;
  at: number;
  autoRetried: boolean;
}

let FAILED: Failed | null = null;
let NEXT_ID = 1;

/** The failure waiting for a decision, if any. */
export function current(): PlaybackFailure | null {
  return FAILED ? { ...FAILED.failure } : null;
}

/** Forget the failure (dismissed, or something else started playing). */
export function clear(app: AppState): void {
  if (FAILED) {
    FAILED = null;
    app.emit({ kind: "playbackFailure", failure: null });
  }
}

function scopeOf(app: AppState): Scope | null {
  const s = app.session();
  return s ? Scope.of(s) : null;
}

/** The account, profile or server changed: a failure from another one can't be acted on here. */
export function sessionChanged(app: AppState, session: Session | null): void {
  const scope = session ? Scope.of(session) : null;
  if (FAILED && !Scope.eq(FAILED.scope, scope)) {
    FAILED = null;
    app.emit({ kind: "playbackFailure", failure: null });
  }
}

/** Everything about a failure that the player knows. */
export interface Report {
  item: Metadata;
  error: string | null;
  kind: FailureKind | null;
  phase: Phase;
  positionMs: number;
  mediaIndex: number;
  quality: string;
  remaining: number;
  /** How to start the same playback again (from the failed item on). */
  request: PlayRequest;
  /** The account and server playback started on: its rating keys mean nothing elsewhere. */
  scope: Scope | null;
}

/** Publish a failure and keep what is needed to act on it. */
export function record(app: AppState, report: Report): void {
  // A failure that arrives after switching account or server can't be retried here.
  if (!Scope.eq(report.scope, scopeOf(app))) {
    log.info("dropping a playback failure from another account or server");
    return;
  }
  // A failure from before mpv was involved already carries readable text; mpv's is translated.
  let kind: FailureKind;
  let reason: string;
  let detail: string | null;
  if (report.kind !== null) {
    kind = report.kind;
    reason = report.error ?? "";
    detail = null;
  } else {
    const [k, text] = explain(report.error ?? "");
    kind = k;
    reason = text;
    detail = report.error !== null && report.error !== "" ? report.error : null;
  }
  const video = ["movie", "episode", "clip", "trailer"].includes(Metadata.kind(report.item));
  const lower = video ? lowerQuality(report.quality) : null;
  const live = report.request.live;
  const failure: PlaybackFailure = {
    id: NEXT_ID++,
    reason,
    detail,
    kind,
    phase: report.phase,
    positionMs: Math.max(0, report.positionMs),
    mediaIndex: report.mediaIndex,
    quality: report.quality,
    lowerQuality: lower ? { id: lower.id, label: lower.label } : null,
    canRetry: !live,
    remaining: report.remaining,
    item: report.item,
  };
  app.error(`Couldn't play ${Metadata.fullTitle(failure.item)}: ${failure.reason}`);
  app.emit({ kind: "playbackFailure", failure: { ...failure } });
  FAILED = { failure, request: report.request, scope: report.scope, at: performance.now(), autoRetried: false };
}

/** Turn what mpv said into a kind of failure and a sentence for people. */
export function explain(error: string): [FailureKind, string] {
  const e = error.toLowerCase();
  if (e.includes("loading failed")) {
    return ["network", "The stream could not be loaded. The connection to the server may have dropped, or the server could not serve this file."];
  } else if (e.includes("unrecognized file format") || e.includes("unknown format")) {
    return ["format", "mpv does not recognise this file's format."];
  } else if (e.includes("no audio or video") || e.includes("nothing to play")) {
    return ["format", "The file has no audio or video that mpv could play."];
  } else if (e.includes("demux") || e.includes("decod")) {
    return ["format", "mpv could not read or decode this file."];
  } else if (e.includes("audio output")) {
    return ["output", "mpv could not open the audio output. Check your sound device."];
  } else if (e.includes("video output")) {
    return ["output", "mpv could not open the video output. Check your graphics settings or mpv's vo setting."];
  }
  return ["other", "mpv stopped with an error."];
}

/** A transport failure (connect, timeout, request) or no reachable connection: the connection or the server. */
function isNetworkError(e: unknown): boolean {
  return e instanceof PlexError && (e.kind === "http" || e.kind === "noConnection");
}

/** Classify an error from before mpv was involved (building the queue, starting the player). */
export function explainError(e: unknown): [FailureKind, string] {
  // Walk the cause chain like `anyhow::Error::chain`.
  let kind: FailureKind = "other";
  for (let cause: unknown = e, depth = 0; cause != null && depth < 16; depth++) {
    if (isNetworkError(cause)) {
      kind = "network";
      break;
    }
    cause = cause instanceof Error ? cause.cause : null;
  }
  return [kind, e instanceof Error ? e.message : String(e)];
}

/**
 * The preset one step below `current`, as a fallback for a stream that will not play or keep up.
 * Direct play steps down to a 1080p transcode first; a 4K transcode is only offered by choice.
 */
export function lowerQuality(current: string): Quality | null {
  const find = (id: string) => QUALITIES.find((q) => q.id === id) ?? null;
  if (current === "original" || findQuality(current) === undefined) return find("20m-1080");
  const at = QUALITIES.findIndex((q) => q.id === current);
  if (at < 0) return null;
  return QUALITIES[at + 1] ?? null;
}

/** Act on the failure shown to the user. */
export function recover(app: AppState, action: Recover): void {
  const failed = FAILED;
  FAILED = null;
  if (!failed) throw new Error("There is nothing to retry.");
  const { failure, scope } = failed;
  const request: PlayRequest = { ...failed.request };
  if (!Scope.eq(scope, scopeOf(app))) {
    app.emit({ kind: "playbackFailure", failure: null });
    throw new Error("The account or server changed since this failed, so it can't be retried.");
  }
  switch (action.kind) {
    case "retry":
      break;
    case "version":
      if (!(action.index < Math.max(failure.item.Media.length, 1))) throw new Error("That version does not exist.");
      request.mediaIndex = action.index;
      break;
    case "lowerQuality": {
      const q = failure.lowerQuality;
      if (!q) throw new Error("There is no lower quality to try.");
      request.quality = q.id;
      break;
    }
  }
  app.emit({ kind: "playbackFailure", failure: null });
  request.resume = true;
  player.playFrom(app, request, failure.positionMs);
}

/** The server answered again after a network failure: try once more, on our own. */
export function onReconnect(app: AppState): void {
  const f = FAILED;
  if (
    f &&
    f.failure.kind === "network" &&
    f.failure.canRetry &&
    Scope.eq(f.scope, scopeOf(app)) &&
    !f.autoRetried &&
    performance.now() - f.at < 15 * 60 * 1000 &&
    !player.isActive()
  ) {
    f.autoRetried = true;
    app.toast("Reconnected; resuming playback");
    try {
      recover(app, { kind: "retry" });
    } catch {
      // `let _ = recover(...)`
    }
  }
}
