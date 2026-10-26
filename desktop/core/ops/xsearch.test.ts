import { describe, expect, test } from "bun:test";

import { Metadata, normalizeMedia } from "../../plex";
import { groupKey, Results, type ServerState, subtitle, type Target, version } from "./xsearch";
import { metadataFromJson } from "./util";

function item(kind: string, rk: string, title: string, guid: string | null, year: number): Metadata {
  return metadataFromJson({ ratingKey: rk, type: kind, title, guid, year });
}

function target(id: string, current: boolean): Target {
  return { id, name: `Server ${id}`, current, online: true };
}

describe("xsearch", () => {
  test("same_guid_is_one_group_with_a_copy_per_server", () => {
    const [a, b] = [target("a", true), target("b", false)];
    const r = new Results("copper", [a, b]);
    r.add(b, [item("movie", "7", "Copper Sky", "plex://movie/1", 2015)]);
    r.add(a, [item("movie", "100", "Copper Sky", "plex://movie/1", 2015), item("movie", "101", "Other", "plex://movie/2", 2000)]);
    expect(r.groups.length).toBe(2);
    const g = r.groups[0]!;
    expect(g.copies.length).toBe(2);
    // The current server's copy comes first even though it answered second.
    expect(g.copies[0]!.current).toBe(true);
    expect(Metadata.serverId(g.copies[0]!.item)).toBeNull();
    expect(Metadata.serverId(g.copies[1]!.item)).toBe("b");
    expect(r.done).toBe(true);
  });

  test("items_without_a_usable_guid_group_by_title_and_year", () => {
    const [a, b] = [target("a", true), target("b", false)];
    const r = new Results("x", [a, b]);
    r.add(a, [item("movie", "1", "Paper Moons!", "local://5", 2019)]);
    r.add(b, [item("movie", "9", "paper moons", null, 2019), item("movie", "10", "Paper Moons", null, 1999)]);
    expect(r.groups.length).toBe(2);
    expect(r.groups[0]!.copies.length).toBe(2);
    expect(r.groups[1]!.copies.length).toBe(1);
  });

  test("episodes_group_by_show_and_number_without_a_guid", () => {
    const mk = (rk: string) =>
      metadataFromJson({ ratingKey: rk, type: "episode", title: "Relay", grandparentTitle: "Harbor Lights", parentIndex: 2, index: 3 });
    expect(groupKey(mk("1"))).toBe(groupKey(mk("2")));
    expect(subtitle(mk("1"))).toBe("Harbor Lights S02E03");
  });

  test("duplicates_on_one_server_are_ignored_but_other_items_are_kept", () => {
    const a = target("a", true);
    const r = new Results("x", [a]);
    const m = item("movie", "1", "Same", "plex://movie/1", 2000);
    r.add(a, [m, { ...m }, item("movie", "2", "Same", "plex://movie/1", 2000)]);
    expect(r.groups.length).toBe(1);
    expect(r.groups[0]!.copies.length).toBe(2);
  });

  test("a_failing_server_does_not_hide_the_others", () => {
    const [a, b, c] = [target("a", true), target("b", false), target("c", false)];
    const r = new Results("x", [a, b, c]);
    r.fail(b, "timedOut", "slow");
    expect(r.done).toBe(false);
    r.add(a, [item("movie", "1", "Found", "plex://movie/1", 2000)]);
    expect(r.done).toBe(false);
    r.fail(c, "failed", "refused");
    expect(r.done).toBe(true);
    expect(r.groups.length).toBe(1);
    const states: ServerState[] = r.servers.map((s) => s.state);
    expect(states).toEqual(["done", "timedOut", "failed"]);
    expect(r.servers[1]!.error).toBe("slow");
  });

  test("offline_servers_are_not_waited_for", () => {
    const off = target("b", false);
    off.online = false;
    const r = new Results("x", [target("a", true), off]);
    expect(r.servers[1]!.state).toBe("offline");
    expect(r.done).toBe(false);
  });

  test("versions_describe_quality", () => {
    const media = normalizeMedia({
      videoResolution: "1080",
      videoCodec: "hevc",
      container: "mkv",
      bitrate: 6200,
      Part: [{ size: 2_254_857_830 }],
    });
    const v = version(media);
    expect(v.label).toBe("1080p · HEVC · MKV · 6.2 Mbps · 2.1 GB");
    expect(v.resolution).toBe("1080p");
    const bare = version(normalizeMedia({}));
    expect(bare.label).toBe("Unknown quality");
    const uhd = normalizeMedia({ videoResolution: "4k", bitrate: 800 });
    expect(version(uhd).label).toBe("4K · 800 kbps");
  });
});
