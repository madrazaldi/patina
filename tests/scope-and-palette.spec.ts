import { test, expect, type Page } from "@playwright/test";

// Regression coverage for server-scoped editors, modal keyboard behavior, watchlist lookups,
// targeted library refreshes and the command palette. Uses the development IPC fixture in web/test/bridge.ts.

const pageErrors = new WeakMap<Page, string[]>();
test.afterEach(async ({ page }) => { expect(pageErrors.get(page) ?? []).toEqual([]); });
test.beforeEach(async ({ page }) => {
  const errors: string[] = [];
  pageErrors.set(page, errors);
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("/test/");
  await expect(page.getByRole("button", { name: "Movies", exact: true })).toBeVisible();
});

const calls = (page: Page, cmd: string) => page.evaluate((cmd) => (window as any).__PATINA_TEST__.calls.filter((c: any) => c.cmd === cmd), cmd);

async function openEditor(page: Page) {
  await page.getByRole("button", { name: "Movies", exact: true }).click();
  const card = page.locator(".virtual-grid .card").first();
  await card.click({ button: "right" });
  await page.getByRole("menuitem", { name: "Edit…" }).click();
  const dialog = page.getByRole("dialog", { name: "Edit" });
  await expect(dialog).toBeVisible();
  return { card, dialog };
}

test("the metadata editor closes when the server changes and can't save to the new one", async ({ page }) => {
  const { dialog } = await openEditor(page);
  await expect(dialog.getByLabel("Title", { exact: true })).toHaveValue("Title 00000");
  await dialog.getByLabel("Title", { exact: true }).fill("Renamed");
  await page.evaluate(() => (window as any).__PATINA_TEST__.switchServer());
  await expect(page.getByText("Other Server", { exact: true })).toBeVisible();
  await expect(page.locator("dialog")).toHaveCount(0);
  expect(await calls(page, "edit_metadata")).toHaveLength(0);
});

test("a late editor response after a server change is discarded", async ({ page }) => {
  await page.getByRole("button", { name: "Movies", exact: true }).click();
  const card = page.locator(".virtual-grid .card").first();
  await page.evaluate(() => (window as any).__PATINA_TEST__.hold("metadata"));
  await card.click({ button: "right" });
  await page.getByRole("menuitem", { name: "Edit…" }).click();
  await expect(page.getByRole("dialog", { name: "Edit" })).toBeVisible();
  await page.evaluate(() => (window as any).__PATINA_TEST__.switchServer());
  await expect(page.locator("dialog")).toHaveCount(0);
  await page.evaluate(() => (window as any).__PATINA_TEST__.release("metadata"));
  await page.waitForTimeout(150);
  await expect(page.locator("dialog")).toHaveCount(0);
});

test("other dialogs and menus are cleared by a server change too", async ({ page }) => {
  await page.getByRole("button", { name: "Movies", exact: true }).click();
  await page.locator(".virtual-grid .card").first().click({ button: "right" });
  await page.getByRole("menuitem", { name: "Add to playlist…" }).click();
  await expect(page.getByRole("dialog", { name: "Add to playlist" })).toBeVisible();
  await page.evaluate(() => (window as any).__PATINA_TEST__.switchServer());
  await expect(page.locator("dialog")).toHaveCount(0);
  await page.locator(".card").first().click({ button: "right" });
  await expect(page.getByRole("menu")).toBeVisible();
  await page.evaluate(() => (window as any).__PATINA_TEST__.switchServer());
  await page.evaluate(() => (window as any).__PATINA_TEST__.emit("session", { session: { accountId: "bob", serverId: "other", serverName: "Other Server", baseUrl: "", owned: true, hasTv: true, servers: [] }, sections: [] }));
  await expect(page.getByRole("menu")).toHaveCount(0);
});

test("Tab stays inside the metadata editor and Escape returns focus", async ({ page }) => {
  const { card, dialog } = await openEditor(page);
  await expect(dialog.getByLabel("Title", { exact: true })).toBeFocused();
  const inside = () => page.evaluate(() => !!document.activeElement?.closest("dialog"));
  for (let i = 0; i < 25; i++) {
    await page.keyboard.press("Tab");
    expect(await inside()).toBe(true);
  }
  for (let i = 0; i < 25; i++) {
    await page.keyboard.press("Shift+Tab");
    expect(await inside()).toBe(true);
  }
  // The rest of the page is inert while the dialog is open.
  expect(await page.evaluate(() => {
    const outside = document.querySelector<HTMLElement>("nav.sidebar button")!;
    outside.focus();
    return document.activeElement === outside;
  })).toBe(false);
  await page.keyboard.press("Escape");
  await expect(page.locator("dialog")).toHaveCount(0);
  await expect(card).toBeFocused();
});

test("every editor and picker is a named modal dialog that closes with Escape", async ({ page }) => {
  await page.getByRole("button", { name: "Movies", exact: true }).click();
  const names: [string, string][] = [["Add to playlist…", "Add to playlist"], ["Add to collection…", "Add to collection"], ["Choose poster…", "Choose poster"], ["Fix match…", "Fix match"]];
  for (const [item, name] of names) {
    const card = page.locator(".virtual-grid .card").first();
    await card.click({ button: "right" });
    await page.getByRole("menuitem", { name: item }).click();
    const dialog = page.getByRole("dialog", { name });
    await expect(dialog).toBeVisible();
    expect(await page.evaluate(() => document.querySelector("dialog")!.matches(":modal"))).toBe(true);
    expect(await page.evaluate(() => !!document.activeElement?.closest("dialog"))).toBe(true);
    await page.keyboard.press("Escape");
    await expect(page.locator("dialog")).toHaveCount(0);
    await expect(card).toBeFocused();
  }
});

test("the seek bar is a keyboard operable slider", async ({ page }) => {
  await page.evaluate(() => {
    const item = { ratingKey: "1", type: "movie", title: "Title 00001", Media: [], Genre: [], Role: [], Director: [], Writer: [], Country: [], Collection: [], Label: [], Mood: [], Style: [], Image: [] };
    (window as any).__PATINA_TEST__.emit("now-playing", { item, positionMs: 100000, durationMs: 600000, paused: false, transcoding: false, queueLen: 1, queuePos: 0, uid: 1, ending: false, stopAfter: false, live: false, local: false });
  });
  const slider = page.getByRole("slider", { name: "Seek" });
  await expect(slider).toHaveAttribute("aria-valuemin", "0");
  await expect(slider).toHaveAttribute("aria-valuemax", "600");
  await expect(slider).toHaveAttribute("aria-valuenow", "100");
  await expect(slider).toHaveAttribute("aria-valuetext", "1:40 of 10:00");
  await slider.focus();
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("PageUp");
  await page.keyboard.press("ArrowLeft");
  await page.keyboard.press("PageDown");
  await page.keyboard.press("Home");
  const seeks = (await calls(page, "control")).map((c: any) => c.args.command).filter((c: any) => c.kind === "seekTo").map((c: any) => c.seconds);
  expect(seeks).toEqual([105, 110, 170, 165, 105, 0]);
  await page.keyboard.press("End");
  const last = (await calls(page, "control")).at(-1).args.command;
  expect(last).toEqual({ kind: "seekTo", seconds: 599 });
});

test("watchlist lookups run a few at a time and are repeated after library changes", async ({ page }) => {
  await page.getByRole("button", { name: "Watchlist", exact: true }).click();
  await expect.poll(() => page.evaluate(() => (window as any).__PATINA_TEST__.lookups.total)).toBe(100);
  await expect(page.getByText("Not on this server", { exact: true })).toHaveCount(50);
  const first = await page.evaluate(() => ({ ...(window as any).__PATINA_TEST__.lookups, present: undefined }));
  expect(first.max).toBeGreaterThan(1);
  expect(first.max).toBeLessThanOrEqual(6);

  // A scan adds a title: availability is looked up again, and the entry stops being dimmed.
  await page.evaluate(() => {
    const t = (window as any).__PATINA_TEST__;
    t.lookups.present.add(1);
    t.emit("library-changed", { all: false, sections: ["1"], items: ["1001"] });
    t.emit("revision", 1);
  });
  await expect.poll(() => page.evaluate(() => (window as any).__PATINA_TEST__.lookups.total)).toBe(200);
  await expect(page.getByText("Not on this server", { exact: true })).toHaveCount(49);
  expect(await page.evaluate(() => (window as any).__PATINA_TEST__.lookups.max)).toBeLessThanOrEqual(6);
});

test("failed watchlist lookups are retried instead of cached as unavailable", async ({ page }) => {
  await page.evaluate(() => { (window as any).__PATINA_TEST__.lookups.fail = true; });
  await page.getByRole("button", { name: "Watchlist", exact: true }).click();
  await expect(page.getByText("Couldn't check 100 titles", { exact: false })).toBeVisible();
  await expect(page.getByText("Not on this server", { exact: true })).toHaveCount(0);
  await page.evaluate(() => { (window as any).__PATINA_TEST__.lookups.fail = false; });
  await page.getByRole("button", { name: "Retry", exact: true }).click();
  await expect(page.getByText("Not on this server", { exact: true })).toHaveCount(50);
  await expect(page.getByText("Couldn't check", { exact: false })).toHaveCount(0);
});

test("a revision refetches only the visible library pages and the rest lazily", async ({ page }) => {
  const starts = () => page.evaluate(() => (window as any).__PATINA_TEST__.calls.filter((c: any) => c.cmd === "section_items").map((c: any) => c.args.start));
  await page.getByRole("button", { name: "Movies", exact: true }).click();
  await expect(page.locator(".virtual-grid .card").first()).toBeVisible();
  await page.getByRole("button", { name: "Z", exact: true }).click();
  await expect(page.locator('.virtual-grid [data-index="9600"] .card')).toBeVisible();
  const before = (await starts()).length;
  const emit = (sections: string[]) => page.evaluate((sections) => {
    const t = (window as any).__PATINA_TEST__;
    t.emit("library-changed", { all: false, sections, items: [] });
    t.emit("revision", 1);
  }, sections);

  // Another library changed: nothing is refetched.
  await emit(["99"]);
  await page.waitForTimeout(200);
  expect((await starts()).length).toBe(before);

  // This library changed: only what is on screen is refetched, and it stays on screen meanwhile.
  await emit(["1"]);
  await expect.poll(async () => (await starts()).length).toBeGreaterThan(before);
  await page.waitForTimeout(200);
  const refetched = (await starts()).slice(before);
  expect(refetched.length).toBeLessThanOrEqual(3);
  expect(refetched).not.toContain(0);
  await expect(page.locator('.virtual-grid [data-index="9600"] .card')).toBeVisible();

  // The first page was marked stale: scrolling back to it fetches it again.
  await page.locator("#content").evaluate((el) => (el.scrollTop = 0));
  await expect.poll(async () => (await starts()).slice(before + refetched.length)).toContain(0);

  // A revision with no detail could affect anything and is treated that way.
  const now = (await starts()).length;
  await page.evaluate(() => (window as any).__PATINA_TEST__.emit("revision", 2));
  await expect.poll(async () => (await starts()).length).toBeGreaterThan(now);
});

test("Ctrl+K opens a command palette that filters, navigates and closes with Escape", async ({ page }) => {
  await page.getByPlaceholder("Search", { exact: true }).focus();
  await page.keyboard.press("Control+k");
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await expect(palette).toBeVisible();
  const input = page.getByRole("combobox", { name: "Type a command or search" });
  await expect(input).toBeFocused();
  await expect(input).toHaveAttribute("aria-expanded", "true");
  const listbox = page.getByRole("listbox");
  await expect(listbox).toBeVisible();
  await expect(page.getByRole("option", { name: "Movies" })).toBeVisible();

  // Fuzzy filter: "stg" finds Settings; arrows move the highlight; Enter runs it.
  await input.fill("stg");
  await expect(page.getByRole("option", { name: "Settings" }).first()).toHaveAttribute("aria-selected", "true");
  const active = await input.getAttribute("aria-activedescendant");
  expect(await page.locator(`#${active}`).textContent()).toContain("Settings");
  await input.press("ArrowDown");
  expect(await input.getAttribute("aria-activedescendant")).not.toBe(active);
  await input.press("ArrowUp");
  expect(await input.getAttribute("aria-activedescendant")).toBe(active);
  await input.press("Enter");
  await expect(page.locator("dialog")).toHaveCount(0);
  await expect(page.getByRole("heading", { level: 1, name: "Settings" })).toBeVisible();

  // Ctrl+K again toggles it, and Escape restores focus.
  await page.keyboard.press("Control+k");
  await expect(palette).toBeVisible();
  await page.keyboard.press("Control+k");
  await expect(palette).toHaveCount(0);
  await page.getByRole("button", { name: "Home", exact: true }).focus();
  await page.keyboard.press("Control+k");
  await page.keyboard.press("Escape");
  await expect(palette).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Home", exact: true })).toBeFocused();
});

test("the palette's Search entry searches, and it lists libraries, playback, views and servers", async ({ page }) => {
  await page.evaluate(() => {
    const t = (window as any).__PATINA_TEST__;
    t.setServers();
    t.settings.ui.saved_views.push({ id: "v1", name: "Unwatched favorites", scope: "alice:mock", library: "1", mediaType: 0, sort: "titleSort", desc: false, unwatched: true, toggles: [], picked: [] });
    t.emit("settings-changed", null);
    const item = { ratingKey: "1", type: "movie", title: "Title 00001", Media: [], Genre: [], Role: [], Director: [], Writer: [], Country: [], Collection: [], Label: [], Mood: [], Style: [], Image: [] };
    t.emit("now-playing", { item, positionMs: 1000, durationMs: 600000, paused: false, transcoding: false, queueLen: 1, queuePos: 0, uid: 1, ending: false, stopAfter: false, live: false, local: false });
  });
  await page.keyboard.press("Control+k");
  const input = page.getByRole("combobox", { name: "Type a command or search" });
  await expect(page.getByRole("option", { name: "Pause" })).toBeVisible();
  await expect(page.getByRole("option", { name: "Favorites", exact: true })).toBeVisible();
  await expect(page.getByRole("option", { name: "Switch to Other Server" })).toBeVisible();
  await expect(page.getByRole("option", { name: "Movies: Unwatched favorites" })).toBeVisible();

  // Playback.
  await input.fill("pause");
  await input.press("Enter");
  expect((await calls(page, "control")).map((c: any) => c.args.command.kind)).toContain("togglePause");

  // Saved views.
  await page.keyboard.press("Control+k");
  await page.getByRole("combobox", { name: "Type a command or search" }).fill("unwatched fav");
  await page.getByRole("combobox", { name: "Type a command or search" }).press("Enter");
  await expect(page.getByRole("button", { name: "Unwatched", exact: true })).toHaveClass(/active/);

  // Servers.
  await page.keyboard.press("Control+k");
  await page.getByRole("combobox", { name: "Type a command or search" }).fill("other server");
  await page.getByRole("option", { name: "Switch to Other Server" }).click();
  expect((await calls(page, "switch_server")).map((c: any) => c.args.id)).toEqual(["other"]);

  // Search: the last entry takes what was typed to the search box, as Cmd+K used to focus it.
  await page.keyboard.press("Control+k");
  await page.getByRole("combobox", { name: "Type a command or search" }).fill("zzzz");
  const options = page.getByRole("option");
  await expect(options).toHaveCount(1);
  await expect(options.first()).toHaveText(/Search for “zzzz”/);
  await expect(options.first()).toHaveAttribute("aria-selected", "true");
  await page.getByRole("combobox", { name: "Type a command or search" }).press("Enter");
  await expect(page.locator("dialog")).toHaveCount(0);
  await expect(page.getByPlaceholder("Search", { exact: true })).toHaveValue("zzzz");
  await expect(page.getByPlaceholder("Search", { exact: true })).toBeFocused();
});
