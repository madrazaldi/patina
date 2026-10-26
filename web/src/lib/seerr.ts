// Seerr helpers for the pages: labels for statuses, and what can still be requested.

import { api } from "./api";
import type { SeerrAvailability, SeerrCan, SeerrDetails, SeerrRequestState, SeerrStatus, SeerrTitle } from "./types";

/** "Available", "Requested", ...; null when there's nothing to say. */
export function availabilityLabel(a: SeerrAvailability | null): string | null {
  switch (a) {
    case "pending":
      return "Requested";
    case "processing":
      return "Processing";
    case "partial":
      return "Partly available";
    case "available":
      return "Available";
    case "blocklisted":
      return "Blocklisted";
    case "deleted":
      return "Deleted";
    default:
      return null;
  }
}

export const REQUEST_LABELS: Record<SeerrRequestState, string> = {
  pending: "Pending approval",
  approved: "Approved",
  declined: "Declined",
  failed: "Failed",
  completed: "Completed",
};

export const requestLabel = (s: SeerrRequestState) => REQUEST_LABELS[s];

/** On the server already (all of it, for shows). */
export const isAvailable = (a: SeerrAvailability | null) => a === "available";
/** Nothing has been requested or added yet. */
export const isNew = (a: SeerrAvailability | null) => a == null || a === "unknown" || a === "deleted";

export const canRequestKind = (can: SeerrCan, t: Pick<SeerrTitle, "mediaType">, is4k: boolean) =>
  t.mediaType === "movie" ? (is4k ? can.request4kMovie : can.requestMovie) : is4k ? can.request4kTv : can.requestTv;

/** Seasons of a show that can still be requested in this quality. */
export function openSeasons(d: SeerrDetails, is4k: boolean) {
  return d.seasons.filter((s) => {
    const status = is4k ? s.status4k : s.status;
    const requested = is4k ? s.requested4k : s.requested;
    // Seerr keeps a failed request's seasons until it is retried or deleted.
    return status !== "available" && status !== "partial" && status !== "processing" && requested !== "pending" && requested !== "approved" && requested !== "failed";
  });
}

/** Whether a request button makes sense for this title in this quality. */
export function requestable(d: SeerrDetails, can: SeerrCan, is4k: boolean): boolean {
  if (!canRequestKind(can, d, is4k)) return false;
  if (d.mediaType === "tv") return openSeasons(d, is4k).length > 0;
  return isNew(is4k ? d.status4k : d.status);
}

// The status (user and permissions) is fetched once per instance and account (`<url>|<app.scope>`:
// a Plex Home switch signs in to Seerr as someone else) and shared by the pages.
let cached: { key: string; promise: Promise<SeerrStatus> } | null = null;

export function seerrStatus(key: string): Promise<SeerrStatus> {
  if (cached?.key !== key) {
    const promise = api.seerrStatus();
    cached = { key, promise };
    // A failure is not remembered: the next page asks again.
    promise.catch(() => cached?.promise === promise && (cached = null));
  }
  return cached.promise;
}

export function forgetSeerrStatus() {
  cached = null;
}
