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
