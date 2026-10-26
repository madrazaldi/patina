import { describe, expect, test } from "bun:test";

import { defaultButlerTask } from "../../plex";
import { grouped, joinButler, type Pref } from "./server";

function pref(id: string, group: string, kind: string): Pref {
  return { id, label: id, summary: null, kind, value: "", options: [], advanced: false, group };
}

describe("server", () => {
  test("groups_keep_plex_order_with_ungrouped_last", () => {
    const groups = grouped([
      pref("Odd", "", "text"),
      pref("FriendlyName", "general", "text"),
      pref("TranscoderQuality", "transcoder", "int"),
      pref("LogVerbose", "general", "bool"),
      pref("DlnaEnabled", "dlna", "bool"),
    ]);
    expect(groups.map((g) => g.label)).toEqual(["General", "Transcoder", "DLNA", "Other"]);
    expect(groups[0]!.prefs.length).toBe(2);
  });

  test("butler_switches_join_their_tasks", () => {
    const task = (name: string) => defaultButlerTask({ name });
    const m = joinButler(
      [task("BackupDatabase"), task("CheckForUpdates")],
      [
        pref("FriendlyName", "general", "text"),
        pref("ButlerStartHour", "butler", "int"),
        pref("ButlerTaskBackupDatabase", "butler", "bool"),
        pref("ButlerDatabaseBackupPath", "butler", "text"),
      ],
    );
    expect(m.tasks[0]!.pref?.id).toBe("ButlerTaskBackupDatabase");
    expect(m.tasks[1]!.pref).toBeNull();
    expect(m.schedule.map((p) => p.id)).toEqual(["ButlerStartHour", "ButlerDatabaseBackupPath"]);
  });
});
