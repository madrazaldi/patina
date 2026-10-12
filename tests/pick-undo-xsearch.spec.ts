import { test, expect, type Page } from "@playwright/test";

// Pick something to watch, undo, and search across servers. Uses the development IPC fixture in web/test/bridge.ts.

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

test("Pick something to watch offers candidates, narrows them and rerolls without repeats", async ({ page }) => {
  await page.getByRole("button", { name: "Pick something", exact: true }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Pick something to watch" })).toBeVisible();
  const cards = page.locator(".results .card");
  await expect(cards).toHaveCount(3);
  await expect(cards.first()).toHaveAttribute("aria-label", "Title 00010 (2024)");
  const first = (await calls(page, "pick_something")).at(-1).args.query;
  expect(first).toMatchObject({ kind: "movie", unwatched: true, maxMinutes: null, exclude: [] });

  await page.getByLabel("Runtime").selectOption("90");
  await page.getByLabel("Audience rating").selectOption("7");
  await page.getByRole("button", { name: "Drama", exact: true }).click();
  await expect.poll(async () => (await calls(page, "pick_something")).at(-1).args.query).toMatchObject({ maxMinutes: 90, minRating: 7, genres: ["Drama"], exclude: [] });

  await page.getByRole("button", { name: /Reroll/ }).click();
  await expect(cards.first()).toHaveAttribute("aria-label", "Title 00013 (2024)");
  const again = (await calls(page, "pick_something")).at(-1).args.query;
  expect(again.exclude).toHaveLength(3);
  // The R key rerolls too, but not while a form field has focus.
  await page.locator(".page-head").click();
  await page.keyboard.press("r");
  await expect(cards.first()).toHaveAttribute("aria-label", "Title 00016 (2024)");
});

test("Pick something is in the command palette and on Home", async ({ page }) => {
  await page.keyboard.press("Control+k");
  await page.getByRole("combobox", { name: "Type a command or search" }).fill("pick");
  await page.getByRole("option", { name: /Pick something to watch/ }).first().click();
  await expect(page.getByRole("heading", { level: 1, name: "Pick something to watch" })).toBeVisible();
  await page.getByRole("button", { name: "Home", exact: true }).click();
  await page.getByRole("button", { name: "Pick something to watch" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Pick something to watch" })).toBeVisible();
});

async function markWatched(page: Page) {
  await page.locator(".card").first().click({ button: "right" });
  await page.getByRole("menuitem", { name: "Mark as watched" }).click();
}

test("marking watched offers Undo in the toast, and the toast button undoes it", async ({ page }) => {
  await markWatched(page);
  const toast = page.locator(".toast", { hasText: "Marked 1 watched" });
  await expect(toast).toBeVisible();
  await toast.getByRole("button", { name: "Undo" }).click();
  await expect.poll(async () => (await calls(page, "undo_action")).map((c: any) => c.args.id)).toEqual([7]);
  await expect(page.locator(".toast", { hasText: "Undid: Marked 1 watched" })).toBeVisible();
  // The toast that offered Undo is gone once used.
  await expect(toast).toHaveCount(0);
});

test("Ctrl+Z undoes the latest action, but not while typing in a field", async ({ page }) => {
  await markWatched(page);
  await expect(page.locator(".toast", { hasText: "Marked 1 watched" })).toBeVisible();

  const search = page.getByPlaceholder("Search", { exact: true });
  await search.focus();
  await page.keyboard.press("Control+z");
  await page.waitForTimeout(150);
  expect(await calls(page, "undo_action")).toHaveLength(0);

  await search.blur();
  await page.keyboard.press("Control+z");
  await expect.poll(async () => (await calls(page, "undo_action")).length).toBe(1);
  expect((await calls(page, "undo_action"))[0].args.id).toBe(7);

  // Nothing is left to undo.
  await page.keyboard.press("Control+z");
  await expect(page.getByText("Nothing to undo", { exact: true })).toBeVisible();
});

test("Ctrl+Z is left alone inside a dialog", async ({ page }) => {
  await markWatched(page);
  await page.keyboard.press("Control+k");
  await expect(page.getByRole("dialog", { name: "Command palette" })).toBeVisible();
  await page.keyboard.press("Control+z");
  await page.waitForTimeout(150);
  expect(await calls(page, "undo_action")).toHaveLength(0);
});

test("searching all servers shows results as servers answer and lets you choose a copy to play", async ({ page }) => {
  await page.evaluate(() => {
    const t = (window as any).__PATINA_TEST__;
    t.setServers();
    t.xsearch.hold = true;
  });
  await page.getByPlaceholder("Search", { exact: true }).fill("shared");
  await page.getByLabel("Search all servers").check();

  // The connected server answered; the other is still working.
  const strip = page.getByRole("status", { name: "Servers searched" });
  await expect(strip).toContainText("Other Server searching…");
  await expect(page.getByRole("button", { name: /^Shared 1 \(2024\)/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /Only Elsewhere/ })).toHaveCount(0);

  await page.evaluate(() => (window as any).__PATINA_TEST__.xsearch.release());
  await expect(strip).toContainText("Other Server 2 titles");
  const row = page.getByRole("button", { name: /^Shared 1 \(2024\)/ });
  await expect(row).toContainText("Mock Server");
  await expect(row).toContainText("1080p");
  await expect(row).toContainText("Other Server");
  await expect(row).toContainText("4K");
  await expect(page.getByRole("button", { name: /Only Elsewhere/ })).toBeVisible();

  const searches = await calls(page, "search_all_servers");
  expect(searches.at(-1).args.query).toBe("shared");
  expect(await calls(page, "search")).toHaveLength(0);

  await row.click();
  const dialog = page.getByRole("dialog", { name: "Shared 1 (2024)" });
  await expect(dialog).toBeVisible();
  const other = dialog.locator("li", { hasText: "Other Server" });
  await expect(other).toContainText("4K");
  await expect(other).toContainText("12.4 Mbps");
  await other.getByRole("button", { name: /Play/ }).click();
  await expect(page.locator("dialog")).toHaveCount(0);
  await expect.poll(async () => (await calls(page, "play")).length).toBe(1);
  const request = (await calls(page, "play"))[0].args.request;
  expect(request.item.patinaServer).toBe("other");
  expect(request.item.title).toBe("Shared 41");
});

test("a title found on another server opens its details from that server", async ({ page }) => {
  await page.evaluate(() => (window as any).__PATINA_TEST__.setServers());
  await page.getByPlaceholder("Search", { exact: true }).fill("shared");
  await page.getByLabel("Search all servers").check();
  await page.getByRole("button", { name: /Only Elsewhere/ }).click();
  const dialog = page.getByRole("dialog", { name: "Only Elsewhere (2020)" });
  await dialog.getByRole("button", { name: /Details on Other Server/ }).click();
  await expect.poll(async () => (await calls(page, "metadata")).at(-1)?.args).toMatchObject({ ratingKey: "42", server: "other" });
});

test("the all-servers switch only appears with more than one server", async ({ page }) => {
  await page.getByPlaceholder("Search", { exact: true }).fill("shared");
  await expect(page.getByLabel("Search all servers")).toHaveCount(0);
  await page.evaluate(() => (window as any).__PATINA_TEST__.setServers());
  await expect(page.getByLabel("Search all servers")).toBeVisible();
});
