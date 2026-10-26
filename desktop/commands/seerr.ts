// Commands for Seerr: discover and request titles, follow and manage requests.

import * as seerr from "../core/seerr";
import type { Handler } from "./types";

const mediaType = (v: unknown): seerr.SeerrMediaType => {
  if (v !== "movie" && v !== "tv") throw new Error(`Unknown media type ${String(v)}.`);
  return v;
};

export const seerrCommands: Record<string, Handler> = {
  /** The configured instance (`url`, `auth`), or null; does not contact it. */
  seerr_config: (app) => seerr.config(app),
  /** Contact the instance: version, the signed-in user and what they may do. */
  seerr_status: (app) => seerr.status(app),
  /** Check and save an instance; an empty `apiKey` signs in with the Plex account, a null one keeps the saved key. */
  seerr_connect: (app, a) => seerr.connect(app, String(a.url ?? ""), a.apiKey == null ? null : String(a.apiKey)),
  seerr_disconnect: (app) => seerr.disconnect(app),
  /** A discover list: `trending`, `movies`, `tv`, `upcoming-movies` or `upcoming-tv`. */
  seerr_discover: (app, a) => seerr.discover(app, String(a.list), Number(a.page) || 1),
  seerr_search: (app, a) => seerr.search(app, String(a.query ?? ""), Number(a.page) || 1),
  seerr_details: (app, a) => seerr.details(app, mediaType(a.mediaType), Number(a.tmdbId)),
  /** Request a movie, or seasons of a show. Returns the request's state. */
  seerr_request: (app, a) =>
    seerr.request(app, {
      mediaType: mediaType(a.mediaType),
      tmdbId: Number(a.tmdbId),
      tvdbId: a.tvdbId == null ? null : Number(a.tvdbId),
      seasons: Array.isArray(a.seasons) ? a.seasons.map(Number) : undefined,
      is4k: !!a.is4k,
      serverId: a.serverId == null ? null : Number(a.serverId),
      profileId: a.profileId == null ? null : Number(a.profileId),
      rootFolder: a.rootFolder == null ? null : String(a.rootFolder),
    }),
  /** Radarr or Sonarr servers to choose from (advanced requests). */
  seerr_request_options: (app, a) => seerr.requestOptions(app, mediaType(a.mediaType), !!a.is4k),
  /** Requests by `filter` (see `REQUEST_FILTERS`), 20 a page; `mine` limits them to the user's own. */
  seerr_requests: (app, a) => seerr.requests(app, String(a.filter ?? "all"), Number(a.page) || 1, !!a.mine),
  /** `approve`, `decline`, `retry` or `delete` a request. */
  seerr_request_action: (app, a) => seerr.requestAction(app, Number(a.id), String(a.action)),
  /** Open a title's page on the Seerr website. */
  seerr_open: (app, a, ctx) => ctx.host.openUrl(seerr.pageUrl(app, mediaType(a.mediaType), Number(a.tmdbId))),
};
