import { test, expect, type Page } from "@playwright/test";

// Seerr: connecting in Settings, requesting from Search and a title page, and managing requests.
// Uses the development IPC fixture in web/test/bridge.ts.

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
const connect = async (page: Page) => {
  await page.evaluate(() => (window as any).__PATINA_TEST__.seerr.on());
  await page.evaluate(() => (window as any).__PATINA_TEST__.emit("settings-changed", null));
  await expect(page.getByRole("button", { name: "Requests", exact: true })).toBeVisible();
};

test("connecting in Settings shows the error, then the account, and adds Requests to the sidebar", async ({ page }) => {
  await expect(page.getByRole("button", { name: "Requests", exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await page.getByLabel("Seerr address").fill("seerr.test:5055");
  await page.getByLabel("Seerr API key").fill("wrong");
  await page.getByRole("button", { name: "Connect", exact: true }).click();
  await expect(page.getByText("Seerr did not accept the sign-in")).toBeVisible();
  await page.getByLabel("Seerr API key").fill("mock-key");
  await page.getByRole("button", { name: "Connect", exact: true }).click();
  await expect(page.getByText("Connected as Mock Admin")).toBeVisible();
  expect((await calls(page, "seerr_connect")).at(-1).args).toEqual({ url: "seerr.test:5055", apiKey: "mock-key" });
  await expect(page.getByRole("button", { name: "Requests", exact: true })).toBeVisible();
  // Reconnecting with the key field empty keeps the saved key.
  await page.getByRole("button", { name: "Reconnect", exact: true }).click();
  await expect.poll(async () => (await calls(page, "seerr_connect")).at(-1).args.apiKey).toBeNull();
});

test("search offers titles the server lacks and requests seasons of a show", async ({ page }, testInfo) => {
  await connect(page);
  await page.locator(".topbar .search input").fill("ferry");
  const section = page.getByRole("region", { name: "Request with Seerr" });
  await expect(section.getByRole("button", { name: "Ferry Season (2021), Show, Partly available" })).toBeVisible();
  await section.getByRole("button", { name: /Ferry Season/ }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Ferry Season" })).toBeVisible();
  await page.getByRole("button", { name: "Request more", exact: true }).click();
  const dialog = page.locator("dialog");
  // Season 1 is on the server already; season 2's request failed and stays Seerr's until retried or deleted.
  await expect(dialog.getByRole("checkbox", { name: "Season 1" })).toHaveCount(0);
  await expect(dialog.getByRole("checkbox", { name: "Season 2" })).toHaveCount(0);
  await dialog.getByRole("checkbox", { name: "Season 3" }).click();
  await page.screenshot({ path: testInfo.outputPath("seerr-request.png"), animations: "disabled" });
  await dialog.getByRole("button", { name: "Request 1 season", exact: true }).click();
  await expect(page.getByText("Ferry Season requested and approved")).toBeVisible();
  expect((await calls(page, "seerr_request")).at(-1).args).toEqual({
    mediaType: "tv", tmdbId: 8001, tvdbId: 7001, seasons: [3], is4k: false, serverId: null, profileId: null, rootFolder: null,
  });
});

test("an available title opens the server's copy, and requests can be approved", async ({ page }) => {
  await connect(page);
  await page.getByRole("button", { name: "Requests", exact: true }).click();
  await page.getByRole("button", { name: /Title 00001/ }).click();
  await page.getByRole("button", { name: "Open in Plex", exact: true }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Title 00001" })).toBeVisible();
  await page.getByTitle("Back (⌘[)").click();
  await page.getByTitle("Back (⌘[)").click();
  await page.getByRole("button", { name: "All requests", exact: true }).click();
  const row = page.getByRole("button", { name: /Northern Static/ });
  await expect(row).toContainText("Pending approval");
  await row.getByTitle("Approve").click();
  await expect(page.getByText("Request approved")).toBeVisible();
  expect((await calls(page, "seerr_request_action")).at(-1).args).toEqual({ id: 7, action: "approve" });
  await expect(row).toContainText("Approved");
});

test("deleting the only request on the last page steps back to the new last page", async ({ page }) => {
  await page.evaluate(() => (window as any).__PATINA_TEST__.seerr.many(41));
  await connect(page);
  await page.getByRole("button", { name: "Requests", exact: true }).click();
  await page.getByRole("button", { name: "All requests", exact: true }).click();
  await page.getByRole("button", { name: "Older" }).click();
  await page.getByRole("button", { name: "Older" }).click();
  await expect(page.getByText("Page 3 of 3")).toBeVisible();
  await page.getByTitle("Delete request").click();
  await page.getByRole("button", { name: "Delete", exact: true }).click();
  await expect(page.getByText("Page 2 of 2")).toBeVisible();
  await expect(page.locator(".rows .list-row")).toHaveCount(20);
});
