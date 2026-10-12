import { test, expect, type Page } from "@playwright/test";

const pageErrors = new WeakMap<Page, string[]>();
test.afterEach(async ({ page }) => { expect(pageErrors.get(page) ?? []).toEqual([]); });
test.beforeEach(async ({ page }) => { const errors: string[] = []; pageErrors.set(page, errors); page.on("pageerror", (e) => errors.push(e.message)); await page.goto("/test/"); await expect(page.getByRole("button", { name: "Movies", exact: true })).toBeVisible(); });

test("filtered library survives details and back with focus", async ({ page }) => {
  await page.getByRole("button", { name: "Movies", exact: true }).click();
  await page.getByRole("button", { name: "Unwatched", exact: true }).click();
  const card = page.locator(".virtual-grid .card").first();
  await card.focus(); await card.press("Enter");
  await expect(page.getByRole("heading", { level: 1, name: "Title 00000" })).toBeVisible();
  await page.getByTitle("Back (⌘[)").click();
  await expect(page.getByRole("button", { name: "Unwatched", exact: true })).toHaveClass(/active/);
  await expect(page.locator(".virtual-grid .card").first()).toBeFocused();
});

test("letter jump fetches destination pages and bounds rendered cards", async ({ page }, testInfo) => {
  await page.getByRole("button", { name: "Movies", exact: true }).click();
  await page.getByRole("button", { name: "Z", exact: true }).click();
  await expect(page.locator('.virtual-grid [data-index="9600"] .card')).toBeVisible();
  expect(await page.locator(".virtual-grid .card").count()).toBeLessThan(100);
  const starts = await page.evaluate(() => (window as any).__PATINA_TEST__.calls.filter((c: any) => c.cmd === "section_items").map((c: any) => c.args.start));
  expect(starts.some((n: number) => n >= 9480)).toBeTruthy();
  expect(starts.filter((n: number) => n > 120 && n < 9360)).toHaveLength(0);
  const before = await page.locator("#content").evaluate((el) => el.scrollTop);
  await page.locator('.virtual-grid [data-index="9600"] .card').click();
  await expect(page.getByRole("heading", { level: 1, name: "Title 09600" })).toBeVisible();
  await page.getByTitle("Back (⌘[)").click();
  await expect.poll(() => page.locator("#content").evaluate((el) => el.scrollTop)).toBe(before);
  await expect(page.locator('.virtual-grid [data-index="9600"] .card')).toBeFocused();
  await page.screenshot({ path: testInfo.outputPath("library.png"), animations: "disabled" });
});

test("Enter on Cancel never deletes and focus returns", async ({ page }) => {
  await page.getByRole("button", { name: "Downloads", exact: true }).click();
  await page.getByTitle("Delete download", { exact: true }).click();
  await expect(page.getByRole("button", { name: "Cancel", exact: true })).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(page.getByRole("button", { name: "Delete", exact: true })).toBeFocused();
  await page.keyboard.press("Tab");
  await page.keyboard.press("Enter");
  await expect(page.locator("dialog")).toHaveCount(0);
  await expect(page.getByTitle("Delete download", { exact: true })).toBeFocused();
  expect(await page.evaluate(() => (window as any).__PATINA_TEST__.calls.some((c: any) => c.cmd === "remove_download"))).toBeFalsy();
});

test("live refresh keeps cards visible and offers retry", async ({ page }) => {
  await page.getByRole("button", { name: "Movies", exact: true }).click();
  await expect(page.locator(".virtual-grid .card").first()).toBeVisible();
  await page.evaluate(() => { const t = (window as any).__PATINA_TEST__; t.fail(true); t.emit("revision", 1); });
  await expect(page.getByText("Showing saved results")).toBeVisible();
  await expect(page.locator(".virtual-grid .card").first()).toBeVisible();
  await page.evaluate(() => (window as any).__PATINA_TEST__.fail(false));
  await page.getByRole("button", { name: "Retry", exact: true }).click();
  await expect(page.getByText("Showing saved results")).toHaveCount(0);
});

test("saved views, bulk actions and customized Home persist", async ({ page }) => {
  await page.getByRole("button", { name: "Movies", exact: true }).click();
  await page.getByRole("button", { name: "Unwatched", exact: true }).click();
  await page.getByLabel("Saved view name").fill("Unwatched favorites");
  await page.getByRole("button", { name: "Save view", exact: true }).click();
  await page.getByRole("button", { name: "Select titles", exact: true }).click();
  await page.locator(".virtual-grid .card").nth(0).click();
  await page.locator(".virtual-grid .card").nth(1).click();
  await page.getByRole("button", { name: "Mark watched", exact: true }).click();
  await expect(page.getByText("0 selected", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Home", exact: true }).click();
  await page.getByRole("button", { name: "Customize Home" }).click();
  await page.getByLabel("Compact Continue Watching").check();
  await page.getByLabel("Recently Added", { exact: true }).uncheck();
  await expect.poll(() => page.evaluate(() => (window as any).__PATINA_TEST__.settings.ui.saved_views.length)).toBe(1);
  await expect.poll(() => page.evaluate(() => (window as any).__PATINA_TEST__.settings.ui.home_layouts['alice:mock']?.compact_continue)).toBe(true);
  await page.getByRole("button", { name: "Movies", exact: true }).click();
  await expect(page.getByRole("button", { name: "Unwatched favorites", exact: true })).toBeVisible();
});

test("late library responses cannot cross a server switch", async ({ page }) => {
  await page.evaluate(() => (window as any).__PATINA_TEST__.delay(500));
  await page.getByRole("button", { name: "Movies", exact: true }).click();
  await page.evaluate(() => (window as any).__PATINA_TEST__.switchServer());
  await expect(page.getByText("Other Server", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Customize Home" })).toBeVisible();
  await page.waitForTimeout(650);
  await expect(page.locator(".virtual-grid")).toHaveCount(0);
});

test("bulk queue is submitted once and playlist receives every selected key", async ({ page }) => {
  await page.getByRole("button", { name: "Movies", exact: true }).click();
  await page.getByRole("button", { name: "Select titles", exact: true }).click();
  await page.locator(".virtual-grid .card").nth(0).click();
  await page.locator(".virtual-grid .card").nth(1).click();
  await page.getByRole("button", { name: "Add to queue", exact: true }).click();
  await expect.poll(() => page.evaluate(() => (window as any).__PATINA_TEST__.calls.filter((c: any) => c.cmd === "queue_items").length)).toBe(1);
  expect(await page.evaluate(() => (window as any).__PATINA_TEST__.calls.find((c: any) => c.cmd === "queue_items").args.items.length)).toBe(2);
  await page.locator(".virtual-grid .card").nth(0).click();
  await page.locator(".virtual-grid .card").nth(1).click();
  await page.getByRole("button", { name: "Add to playlist", exact: true }).click();
  await page.getByRole("button", { name: /Favorites/ }).click();
  await expect.poll(() => page.evaluate(() => (window as any).__PATINA_TEST__.calls.find((c: any) => c.cmd === "add_to_playlist")?.args.ratingKey)).toBe("0,1");
});

test("arrow navigation leaves visible rows in place", async ({ page }) => {
  await page.getByRole("button", { name: "Movies", exact: true }).click();
  const first = page.locator('.virtual-grid [data-index="0"] .card');
  await first.focus();
  const before = await page.locator("#content").evaluate((el) => el.scrollTop);
  await first.press("ArrowRight");
  await expect(page.locator('.virtual-grid [data-index="1"] .card')).toBeFocused();
  expect(await page.locator("#content").evaluate((el) => el.scrollTop)).toBe(before);
  // Moving to an offscreen row still scrolls far enough to reveal its card.
  for (let i = 0; i < 5; i++) await page.keyboard.press("ArrowDown");
  await expect.poll(() => page.locator("#content").evaluate((el) => el.scrollTop)).toBeGreaterThan(before);
  expect(await page.locator(".virtual-grid .card:focus").evaluate((el) => {
    const card = el.getBoundingClientRect(), viewport = document.getElementById("content")!.getBoundingClientRect();
    return card.top >= viewport.top && card.bottom <= viewport.bottom;
  })).toBe(true);
});

test("download Play all selects only one profile and server", async ({ page }) => {
  await page.evaluate(() => {
    const episode = { ratingKey: "10", type: "episode", title: "Episode", grandparentTitle: "Shared show", parentIndex: 1, index: 1 };
    const downloads = [
      { id: "mock:alice:10", account: "alice", serverId: "mock" },
      { id: "mock:alice:11", account: "alice", serverId: "mock" },
      { id: "mock:bob:10", account: "bob", serverId: "mock" },
      { id: "other:alice:10", account: "alice", serverId: "other" },
    ].map((d) => ({ ...d, item: episode, state: "done", size: 100, received: 100, serverName: d.serverId }));
    (window as any).__PATINA_TEST__.emit("downloads", downloads);
  });
  await page.getByRole("button", { name: "Downloads", exact: true }).click();
  await expect(page.getByRole("button", { name: "Play all", exact: true })).toHaveCount(3);
  await page.getByRole("button", { name: "Play all", exact: true }).first().click();
  const calls = await page.evaluate(() => (window as any).__PATINA_TEST__.calls.filter((c: any) => c.cmd === "play_downloads"));
  expect(calls[0].args.ids).toEqual(["mock:alice:10", "mock:alice:11"]);
});

test("a late search retry cannot overwrite newer successful results", async ({ page }) => {
  await page.evaluate(() => {
    const bridge = (window as any).__TAURI_INTERNALS__, invoke = bridge.invoke;
    const requests: { resolve: (value: unknown) => void; reject: (error: Error) => void }[] = [];
    (window as any).__searchRequests = requests;
    bridge.invoke = (cmd: string, args: unknown) => cmd === "search"
      ? new Promise((resolve, reject) => requests.push({ resolve, reject }))
      : invoke(cmd, args);
  });
  await page.getByPlaceholder("Search", { exact: true }).fill("movie");
  await expect.poll(() => page.evaluate(() => (window as any).__searchRequests.length)).toBe(1);
  await page.evaluate(() => (window as any).__searchRequests[0].reject(new Error("First failure")));
  await page.getByRole("button", { name: "Retry", exact: true }).click();
  await expect.poll(() => page.evaluate(() => (window as any).__searchRequests.length)).toBe(2);
  await page.getByRole("button", { name: "Retry", exact: true }).click();
  await expect.poll(() => page.evaluate(() => (window as any).__searchRequests.length)).toBe(3);
  await page.evaluate(() => (window as any).__searchRequests[2].resolve([]));
  await expect(page.getByText("No results", { exact: true })).toBeVisible();
  await page.evaluate(async () => {
    (window as any).__searchRequests[1].reject(new Error("Late failure"));
    await new Promise(requestAnimationFrame);
  });
  await expect(page.getByText("No results", { exact: true })).toBeVisible();
  await expect(page.getByText("Late failure", { exact: true })).toHaveCount(0);
});

for (const typing of [false, true]) {
  test(`pending scroll restoration ${typing ? "ignores text-field arrows" : "yields to keyboard scrolling"}`, async ({ page }) => {
    await page.getByRole("button", { name: "Movies", exact: true }).click();
    await page.getByRole("button", { name: "Z", exact: true }).click();
    const card = page.locator('.virtual-grid [data-index="9600"] .card');
    await expect(card).toBeVisible();
    const before = await page.locator("#content").evaluate((el) => el.scrollTop);
    await card.click();
    await expect(page.getByRole("heading", { level: 1, name: "Title 09600" })).toBeVisible();
    await page.evaluate(async () => {
      const { libraryMemory } = await import("/src/lib/library.svelte.ts");
      libraryMemory.clear();
      (window as any).__PATINA_TEST__.delay(700);
    });
    await page.getByTitle("Back (⌘[)").click();
    await expect(page.locator(".virtual-grid")).toHaveCount(0);
    await page.evaluate((typing) => {
      const target = typing ? document.querySelector<HTMLInputElement>('input[placeholder="Search"]')! : document.body;
      target.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
    }, typing);
    await expect(page.locator(".virtual-grid")).toBeVisible();
    if (typing) await expect.poll(() => page.locator("#content").evaluate((el) => el.scrollTop)).toBe(before);
    else expect(await page.locator("#content").evaluate((el) => el.scrollTop)).toBe(0);
  });
}

test("server settings are grouped and task schedules live in Maintenance", async ({ page }, testInfo) => {
  await page.getByRole("button", { name: "Server", exact: true }).click();
  await page.getByRole("button", { name: "Settings", exact: true }).last().click();
  await expect(page.getByRole("heading", { name: "General" })).toBeVisible();
  await expect(page.getByText("Secure connections")).toHaveCount(0);
  await expect(page.getByText("Debug logging")).toHaveCount(0);
  await page.locator(".groups").getByRole("button", { name: /Network/ }).click();
  await expect(page.getByText("Secure connections")).toBeVisible();
  await page.getByPlaceholder("Search all settings").fill("quality");
  await expect(page.getByRole("heading", { name: "Transcoder" })).toBeVisible();
  await expect(page.getByText("Secure connections")).toHaveCount(0);
  await page.screenshot({ path: testInfo.outputPath("server-settings.png"), animations: "disabled" });

  await page.getByRole("button", { name: "Maintenance", exact: true }).click();
  await page.getByRole("button", { name: "Run Clean Old Bundles on schedule" }).click();
  const saved = await page.evaluate(() => (window as any).__PATINA_TEST__.calls.filter((c: any) => c.cmd === "set_pref").map((c: any) => c.args));
  expect(saved).toEqual([{ id: "ButlerTaskCleanOldBundles", value: "true" }]);
  await expect(page.getByRole("button", { name: "Run Clean Old Bundles on schedule" })).toHaveClass(/on/);
  await expect(page.getByText("Every 3 days", { exact: true })).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath("server-maintenance.png"), animations: "disabled", fullPage: true });
});

test("automatic playback preferences are edited in Settings", async ({ page }) => {
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Automatic playback" })).toBeVisible();
  await page.getByPlaceholder("Not set").first().fill("eng, ja");
  await page.getByPlaceholder("Not set").first().press("Tab");
  await expect.poll(() => page.evaluate(() => (window as any).__PATINA_TEST__.calls.filter((c: any) => c.cmd === "set_playback_prefs").map((c: any) => c.args.rules.audioLanguages))).toEqual([["eng", "ja"]]);
  await expect(page.getByText("ja audio", { exact: false })).toBeVisible();
  await page.getByRole("button", { name: "Clear", exact: true }).click();
  await expect.poll(() => page.evaluate(() => (window as any).__PATINA_TEST__.calls.some((c: any) => c.cmd === "set_playback_prefs" && c.args.show === "500" && Object.keys(c.args.rules).length === 0))).toBe(true);
});

test("smart download policies and the storage budget live on the Downloads page", async ({ page }) => {
  await page.getByRole("button", { name: "Downloads", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Smart downloads" })).toBeVisible();
  await expect(page.getByText("Station Eleven Below")).toBeVisible();
  await page.getByRole("spinbutton").first().fill("5");
  await page.getByRole("spinbutton").first().press("Tab");
  await page.getByPlaceholder("No limit").fill("20");
  await page.getByPlaceholder("No limit").press("Tab");
  await page.getByTitle("Pause", { exact: true }).click();
  const calls = await page.evaluate(() => (window as any).__PATINA_TEST__.calls.filter((c: any) => c.cmd === "update_smart_download" || c.cmd === "set_download_budget").map((c: any) => [c.cmd, c.args.patch ?? c.args.bytes]));
  expect(calls).toEqual([["update_smart_download", { keep: 5 }], ["set_download_budget", 20 * 1024 ** 3], ["update_smart_download", { enabled: false }]]);
});

const movie = (key: string, title: string) => ({
  ratingKey: key, key: `/library/metadata/${key}`, type: "movie", title, year: 2024, duration: 600000,
  Media: [
    { id: 1, height: 2160, videoResolution: "4k", videoCodec: "hevc", Part: [] },
    { id: 2, height: 1080, videoResolution: "1080", videoCodec: "h264", Part: [] },
  ],
  Genre: [], Role: [], Director: [], Writer: [], Country: [], Collection: [], Label: [], Mood: [], Style: [], Image: [], smart: false,
});
const callsTo = (page: Page, cmd: string) => page.evaluate((c) => (window as any).__PATINA_TEST__.calls.filter((x: any) => x.cmd === c).map((x: any) => x.args), cmd);

test("a saved queue is offered but never plays until Resume is pressed", async ({ page }) => {
  await page.evaluate(() => {
    const t = (window as any).__PATINA_TEST__;
    t.setSavedQueue({ current: { ratingKey: "7", key: "/library/metadata/7", type: "movie", title: "Saved Movie" }, positionMs: 61000, index: 1, count: 3, savedAt: 1 });
    t.switchServer();
  });
  await expect(page.getByText("Saved Movie")).toBeVisible();
  await expect(page.getByText("at 1:01")).toBeVisible();
  await expect(page.getByText("2 of 3 in the queue")).toBeVisible();
  expect(await callsTo(page, "resume_saved")).toHaveLength(0);
  await page.getByRole("button", { name: "Resume", exact: true }).click();
  await expect.poll(() => callsTo(page, "resume_saved")).toHaveLength(1);
  await expect(page.getByText("Saved Movie")).toHaveCount(0);
});

test("a playback failure says why and offers retry, another version and a lower quality", async ({ page }) => {
  const failure = { id: 1, item: movie("9", "Broken Movie"), reason: "The stream could not be loaded.", detail: "loading failed", kind: "network", phase: "start", positionMs: 61000, mediaIndex: 0, quality: "original", lowerQuality: { id: "20m-1080", label: "1080p 20 Mbps" }, canRetry: true, remaining: 0 };
  await page.evaluate((f) => (window as any).__PATINA_TEST__.emit("playback-failure", f), failure);
  await expect(page.getByText("The stream could not be loaded.")).toBeVisible();
  await page.getByRole("button", { name: "Try again from 1:01" }).click();
  await page.getByRole("button", { name: "Try 1080p H264" }).click();
  await page.getByRole("button", { name: "Try 1080p 20 Mbps" }).click();
  expect(await callsTo(page, "recover_playback")).toEqual([{ action: { kind: "retry" } }, { action: { kind: "version", index: 1 } }, { action: { kind: "lowerQuality" } }]);
  await page.getByTitle("Dismiss").click();
  expect(await callsTo(page, "dismiss_failure")).toHaveLength(1);
  await page.evaluate(() => (window as any).__PATINA_TEST__.emit("playback-failure", null));
  await expect(page.getByText("The stream could not be loaded.")).toHaveCount(0);
});

test("the player panel shows controls and diagnostics and sends tune commands", async ({ page }) => {
  await page.evaluate((item) => {
    const t = (window as any).__PATINA_TEST__;
    t.emit("now-playing", { item, positionMs: 5000, durationMs: 600000, paused: false, transcoding: false, queueLen: 1, queuePos: 0, uid: 1, ending: false, stopAfter: false, live: false, local: false });
    t.emit("playback-state", {
      volume: 40, muted: false, speed: 1.5, repeat: "all", sleep: { mode: "minutes", endsAtMs: Date.now() + 29 * 60000 },
      chapters: [{ title: "Intro", timeMs: 0 }, { title: "Main", timeMs: 120000 }], chapter: 0,
      diagnostics: { buffering: true, bufferPercent: 35, bufferedSecs: 12.5, hwdec: "videotoolbox", droppedFrames: 3, decoderDroppedFrames: 1, connection: { kind: "relay", address: "relay.plex.direct" } },
    });
  }, movie("5", "Playing Movie"));
  const bar = page.locator("footer.now-playing");
  await expect(bar.getByText("Buffering 35%")).toBeVisible();
  await expect(bar.getByText("Vol 40%")).toBeVisible();
  await expect(bar.getByText("1.5x")).toBeVisible();
  await expect(bar.getByText("Repeat all")).toBeVisible();
  await expect(bar.getByText("Sleep 29m")).toBeVisible();

  await page.getByTitle("Player controls and diagnostics").click();
  await expect(page.getByText("Plex relay")).toBeVisible();
  await expect(page.getByText("12.5 s ahead", { exact: false })).toHaveCount(0); // buffering wins over the buffered duration
  await expect(page.getByText("Buffering, 35% full")).toBeVisible();
  await expect(page.getByText("Hardware (videotoolbox)")).toBeVisible();
  await expect(page.getByText("3 output · 1 decoder")).toBeVisible();

  await page.getByRole("button", { name: "2x", exact: true }).click();
  await page.getByRole("button", { name: "One", exact: true }).click();
  await page.getByRole("button", { name: "60m", exact: true }).click();
  await page.getByRole("button", { name: "End of item", exact: true }).click();
  await page.getByRole("button", { name: "Main", exact: false }).click();
  await page.getByTitle("Mute").click();
  expect(await callsTo(page, "tune")).toEqual([
    { command: { kind: "speed", value: 2 } },
    { command: { kind: "repeat", mode: "one" } },
    { command: { kind: "sleepMinutes", minutes: 60 } },
    { command: { kind: "sleepEndOfItem" } },
    { command: { kind: "chapter", index: 1 } },
    { command: { kind: "mute" } },
  ]);
});
