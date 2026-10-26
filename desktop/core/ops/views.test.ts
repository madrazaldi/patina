import { describe, expect, test } from "bun:test";

import { defaultSettings, parseSettings, settingsToToml } from "../config";
import { forget, matching, newSavedView, parse, type Picked, scopeOf, toJson, upsert } from "./views";

/** An entry as the desktop app writes it. */
function guiEntry(id: string, name: string, library: string): Record<string, unknown> {
  return {
    id,
    name,
    scope: "alice:home",
    library,
    mediaType: 0,
    sort: "addedAt",
    desc: true,
    unwatched: true,
    toggles: ["hdr"],
    picked: [{ filter: "genre", title: "Genre", value: "10", label: "Drama" }],
    somethingNew: { a: 1 },
  };
}

describe("saved views", () => {
  test("reads_what_the_desktop_app_wrote", () => {
    const stored = [guiEntry("v1", "Recent drama", "1"), guiEntry("v2", "Elsewhere", "2"), "garbage", { name: "no id" }];
    const views = matching(stored, "alice:home", "1");
    expect(views.length).toBe(1);
    const v = views[0]!;
    expect([v.id, v.name, v.sort]).toEqual(["v1", "Recent drama", "addedAt"]);
    expect(v.desc && v.unwatched && v.tui === undefined).toBe(true);
    expect(v.toggles).toEqual(["hdr"]);
    const picked: Picked[] = [{ filter: "genre", title: "Genre", value: "10", label: "Drama" }];
    expect(v.picked).toEqual(picked);
    // Another account or server doesn't see it.
    expect(matching(stored, "bob:home", "1")).toEqual([]);
    // Unknown fields survive a rewrite.
    const back = toJson(v);
    expect(back.somethingNew).toEqual({ a: 1 });
    expect(back.mediaType).toBe(0);
    expect("tui" in back).toBe(false);
  });

  test("terminal_views_keep_the_desktop_shape", () => {
    const view = newSavedView("alice:home", "1", "  Unwatched HDR ");
    view.unwatched = true;
    view.toggles = ["hdr"];
    view.tui = { hideWatched: true, filter: "dune" };
    const json = toJson(view);
    expect(json.name).toBe("Unwatched HDR");
    for (const key of ["id", "name", "scope", "library", "mediaType", "sort", "desc", "unwatched", "toggles", "picked"]) {
      expect(key in json).toBe(true);
    }
    expect(json.tui).toEqual({ hideWatched: true, filter: "dune" });
    expect(parse(json)).toEqual(view);
    // And it round-trips through the config file.
    const settings = defaultSettings();
    settings.ui.saved_views.push(json);
    const text = settingsToToml(settings);
    const again = parseSettings(text);
    expect(matching(again.ui.saved_views, "alice:home", "1")).toEqual([view]);
  });

  test("saving_the_same_name_replaces_and_removing_deletes", () => {
    const stored: unknown[] = [guiEntry("v1", "Recent drama", "1")];
    const view = newSavedView("alice:home", "1", "recent DRAMA");
    view.sort = "titleSort";
    upsert(stored, view, toJson(view));
    expect(stored.length).toBe(1);
    expect((stored[0] as Record<string, unknown>).id).toBe("v1");
    expect((stored[0] as Record<string, unknown>).sort).toBe("titleSort");
    const other = newSavedView("alice:home", "1", "Another");
    upsert(stored, other, toJson(other));
    expect(stored.length).toBe(2);
    // A view of the same name in another library is a different view.
    const elsewhere = newSavedView("alice:home", "2", "Another");
    upsert(stored, elsewhere, toJson(elsewhere));
    expect(stored.length).toBe(3);
    forget(stored, other.id);
    expect(stored.length).toBe(2);
    forget(stored, "missing");
    expect(stored.length).toBe(2);
  });

  test("scope_matches_the_desktop_apps", () => {
    expect(scopeOf({ accountId: "acct", serverId: "srv" })).toBe("acct:srv");
  });
});
