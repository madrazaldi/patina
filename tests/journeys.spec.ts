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
