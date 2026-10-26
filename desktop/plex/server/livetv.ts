// Live TV & DVR (port of crates/plex/src/server/livetv.rs). These endpoints vary a lot
// between tuner setups and server versions, so they return raw JSON for the caller to pick apart.

import { Params } from "../client";
import { StatusApi } from "./status";

/** `url::form_urlencoded::byte_serialize` */
function urlencode(s: string): string {
  return new URLSearchParams([["", s]]).toString().slice(1);
}

export class LiveTvApi extends StatusApi {
  /** Configured DVRs, each with its tuner devices, lineup and channel mappings. */
  dvrs(): Promise<unknown> {
    return this.raw("GET", "/livetv/dvrs", new Params());
  }

  /** Channels of a DVR lineup. `epg` is the EPG provider path, e.g. `/tv.plex.providers.epg.cloud:2`. */
  lineupChannels(epg: string, lineup: string): Promise<unknown> {
    const p = new Params().set("lineup", lineup);
    return this.raw("GET", `${epg.replace(/\/+$/, "")}/lineups/dvr/channels`, p);
  }

  /** Guide listings between two unix timestamps. */
  epgGrid(epg: string, begins: number, ends: number): Promise<unknown> {
    const p = new Params().set("type", "1,4").set("beginsAt<", ends).set("endsAt>", begins);
    return this.raw("GET", `${epg.replace(/\/+$/, "")}/grid`, p);
  }

  /** Start watching a channel. The response describes the new live session. */
  tune(dvrId: string, channel: string): Promise<unknown> {
    const path = `/livetv/dvrs/${dvrId}/channels/${urlencode(channel)}/tune`;
    return this.raw("POST", path, new Params().set("autoPreview", 1));
  }

  /** Recording rules, with their upcoming grabs. */
  subscriptions(): Promise<unknown> {
    return this.raw("GET", "/media/subscriptions", new Params().set("includeGrabs", 1));
  }

  /** Upcoming recordings. */
  scheduledRecordings(): Promise<unknown> {
    return this.raw("GET", "/media/subscriptions/scheduled", new Params());
  }

  /** The recording template for a guide item (its default rule parameters). */
  subscriptionTemplate(guid: string): Promise<unknown> {
    return this.raw("GET", "/media/subscriptions/template", new Params().set("guid", guid));
  }

  /** Create a recording rule. `params` usually come from `subscriptionTemplate`. */
  createSubscription(params: Params): Promise<unknown> {
    return this.raw("POST", "/media/subscriptions", params);
  }

  deleteSubscription(id: string): Promise<void> {
    return this.delete(`/media/subscriptions/${id}`, new Params());
  }
}
