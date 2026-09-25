// Item actions shared by cards, menus and detail pages.

import { api, type PlayOptions } from "./api";
import { app, act } from "./state.svelte";
import type { Metadata } from "./types";

/** Hand an item (or container) to mpv. */
export function play(item: Metadata, opts: PlayOptions = {}) {
  api.play($state.snapshot(item) as Metadata, opts).catch((e) => app.error(e));
}

export function setWatched(item: Metadata, watched: boolean) {
  return act(
    api.setWatched(item.ratingKey ?? "", watched),
    watched ? "Marked as watched" : "Marked as unwatched",
    true,
  );
}

/** Open the item menu at the pointer. */
export function openMenu(item: Metadata, e: MouseEvent) {
  e.preventDefault();
  e.stopPropagation();
  app.menu = { item, x: e.clientX, y: e.clientY };
}

/** Queue an item (or everything in a container) next, or at the end. Starts playback if idle. */
export function enqueue(item: Metadata, next: boolean) {
  return act(api.queue({ kind: "add", item: $state.snapshot(item) as Metadata, next }));
}

/** Download an item, or every episode/track below it, for offline playback. */
export async function download(item: Metadata) {
  try {
    const n = await api.download(item.ratingKey ?? "");
    app.toast(n === 0 ? "Already downloaded" : n === 1 ? "Downloading…" : `Downloading ${n} items…`);
  } catch (e) {
    app.error(e);
  }
}

/** The finished download of an item on the current server, if any. */
export function downloaded(item: Metadata) {
  return app.downloads.find((d) => d.serverId === app.session?.serverId && (!d.account || d.account === app.session?.accountId) && d.item.ratingKey === item.ratingKey);
}
