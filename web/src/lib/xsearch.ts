// Helpers for results of a search across every server of the account.

import type { SearchGroup, ServerCopy, ServerStatus } from "./types";

const rank = (resolution: string) => (resolution === "4K" ? 2160 : resolution === "SD" ? 480 : Number.parseInt(resolution, 10) || 0);

/** The best resolution among a copy's files ("4K", "1080p"), if the server reports any. */
export function quality(copy: ServerCopy): string | null {
  let best: string | null = null;
  for (const v of copy.versions) {
    if (v.resolution && (best === null || rank(v.resolution) > rank(best))) best = v.resolution;
  }
  return best;
}

/** "Alpha 1080p" — a server with the quality of its best file. */
export const badge = (copy: ServerCopy) => [copy.serverName, quality(copy)].filter(Boolean).join(" ");

/** What a server's search came to, for the status strip. */
export function stateLabel(s: ServerStatus): string {
  switch (s.state) {
    case "pending":
      return "searching…";
    case "done":
      return s.count === 1 ? "1 title" : `${s.count} titles`;
    case "timedOut":
      return "too slow to answer";
    case "offline":
      return "offline";
    default:
      return "unavailable";
  }
}

/** A copy needs no choice when the title is on the connected server only, in one file. */
export const needsChoice = (g: SearchGroup) => g.copies.length > 1 || g.copies[0]?.versions.length > 1 || !g.copies[0]?.current;
