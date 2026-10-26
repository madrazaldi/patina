// About the install: versions, paths, choices and the mpv check.

import { existsSync } from "node:fs";
import { join } from "node:path";

import { probe } from "../../mpv";
import * as config from "../config";
import type { AppState } from "../state";

export interface Choice {
  id: string;
  label: string;
  color: string | null;
}

export interface Info {
  version: string;
  configPath: string;
  mpvConfigDir: string;
  mpvConfExists: boolean;
  qualities: Choice[];
  accents: Choice[];
  /** Debug builds: `PATINA_ROUTE=/item/100` opens a page on startup. */
  debugRoute: string | null;
  /** Debug builds: `PATINA_PLAY=<ratingKey>` starts playback on startup. */
  debugPlay: string | null;
}

let debugBuild = process.env.PATINA_DEBUG_BUILD === "1";

/** Whether this is a development build (`cfg!(debug_assertions)`); the shell sets it. */
export function setDebugBuild(on: boolean): void {
  debugBuild = on;
  // The plex.tv override (desktop/plex/tv.ts) is honoured in debug builds only.
  if (on) process.env.PATINA_DEBUG_BUILD = "1";
  else delete process.env.PATINA_DEBUG_BUILD;
}

export function info(): Info {
  const mpvDir = config.mpvConfigDir();
  const debug = (k: string): string | null => (debugBuild ? (process.env[k] ?? null) : null);
  return {
    version: config.VERSION,
    configPath: join(config.configDir(), "config.toml"),
    mpvConfigDir: mpvDir,
    mpvConfExists: existsSync(join(mpvDir, "mpv.conf")),
    qualities: config.QUALITIES.map((q) => ({ id: q.id, label: q.label, color: null })),
    accents: config.ACCENTS.map((a) => ({ id: a.id, label: a.label, color: a.color })),
    debugRoute: debug("PATINA_ROUTE"),
    debugPlay: debug("PATINA_PLAY"),
  };
}

/** "mpv v0.41.0 — mpv" for the configured (or auto-detected) mpv. */
export async function probeMpv(app: AppState): Promise<string> {
  const program = config.mpvProgram(app.settings().player);
  let version: string;
  try {
    version = await probe(program);
  } catch (e) {
    throw new Error(`Could not run ${program}: ${e instanceof Error ? e.message : String(e)}`);
  }
  // "mpv v0.41.0 Copyright © …" → "mpv v0.41.0"
  const short = version.split(/\s+/).filter(Boolean).slice(0, 2).join(" ");
  return `${short} — ${program}`;
}
