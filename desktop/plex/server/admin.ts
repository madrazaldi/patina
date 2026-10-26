// Preferences, accounts, activities, butler, updates, statistics and logs
// (port of crates/plex/src/server/admin.rs).

import { Params } from "../client";
import { pointer } from "../de";
import {
  normalizeButlerTask,
  type Account,
  type Activity,
  type ButlerTask,
  type Device,
  type MediaContainer,
  type ServerDevice,
  type Setting,
  type StatisticsResources,
} from "../models";
import { LiveTvApi } from "./livetv";

export class AdminApi extends LiveTvApi {
  // ----- Preferences -----------------------------------------------------

  async prefs(): Promise<Setting[]> {
    return (await this.get("/:/prefs", new Params())).Setting;
  }

  setPref(id: string, value: string): Promise<void> {
    return this.put("/:/prefs", new Params().set(id, value));
  }

  setPrefs(params: Params): Promise<void> {
    return this.put("/:/prefs", params);
  }

  // ----- Accounts, devices, clients --------------------------------------

  async accounts(): Promise<Account[]> {
    return (await this.get("/accounts", new Params())).Account;
  }

  async devices(): Promise<Device[]> {
    return (await this.get("/devices", new Params())).Device;
  }

  /** Players discovered on the local network (GDM). */
  async clients(): Promise<ServerDevice[]> {
    return (await this.get("/clients", new Params())).Server;
  }

  async servers(): Promise<ServerDevice[]> {
    return (await this.get("/servers", new Params())).Server;
  }

  /** Media providers exposed by the server (library, live TV, ...), raw. */
  mediaProviders(): Promise<unknown> {
    return this.raw("GET", "/media/providers", new Params());
  }

  // ----- Activities ------------------------------------------------------

  async activities(): Promise<Activity[]> {
    return (await this.get("/activities", new Params())).Activity;
  }

  cancelActivity(uuid: string): Promise<void> {
    return this.delete(`/activities/${uuid}`, new Params());
  }

  // ----- Butler (scheduled maintenance) ----------------------------------

  async butlerTasks(): Promise<ButlerTask[]> {
    const v = await this.raw("GET", "/butler", new Params());
    const list = pointer(v, "/ButlerTasks/ButlerTask");
    // `from_value(list).unwrap_or_default()`: anything that does not decode gives an empty list.
    if (!Array.isArray(list)) return [];
    try {
      return list.map(normalizeButlerTask);
    } catch {
      return [];
    }
  }

  startAllButlerTasks(): Promise<void> {
    return this.post("/butler", new Params());
  }

  stopAllButlerTasks(): Promise<void> {
    return this.delete("/butler", new Params());
  }

  startButlerTask(name: string): Promise<void> {
    return this.post(`/butler/${name}`, new Params());
  }

  stopButlerTask(name: string): Promise<void> {
    return this.delete(`/butler/${name}`, new Params());
  }

  // ----- Updates -----------------------------------------------------------

  updateStatus(): Promise<MediaContainer> {
    return this.get("/updater/status", new Params());
  }

  checkForUpdates(download: boolean): Promise<void> {
    return this.put("/updater/check", new Params().flag("download", download));
  }

  /** Install a downloaded update now, or `tonight` during maintenance. */
  applyUpdate(tonight: boolean, skip: boolean): Promise<void> {
    return this.put("/updater/apply", new Params().flag("tonight", tonight).flag("skip", skip));
  }

  // ----- Statistics --------------------------------------------------------

  /** Plays per account/device, bucketed by the server-defined `timespan` code. */
  statisticsMedia(timespan: number): Promise<MediaContainer> {
    return this.get("/statistics/media", new Params().set("timespan", timespan));
  }

  /** CPU and memory utilisation samples. */
  async statisticsResources(timespan: number): Promise<StatisticsResources[]> {
    return (await this.get("/statistics/resources", new Params().set("timespan", timespan))).StatisticsResources;
  }

  statisticsBandwidth(timespan: number): Promise<MediaContainer> {
    return this.get("/statistics/bandwidth", new Params().set("timespan", timespan));
  }

  // ----- Logging -----------------------------------------------------------

  /** Write a line into the server log. `level`: 0 error .. 4 verbose. */
  log(level: number, message: string, source: string): Promise<void> {
    const p = new Params().set("level", level).set("message", message).set("source", source);
    return this.empty("GET", "/log", p);
  }

  /** Diagnostics archive with the server logs (zip bytes). */
  async downloadLogs(): Promise<Uint8Array> {
    return (await this.bytes("/diagnostics/logs", new Params()))[0];
  }

  /** Download the server database backup (zip bytes). */
  async downloadDatabase(): Promise<Uint8Array> {
    return (await this.bytes("/diagnostics/databases", new Params()))[0];
  }
}
