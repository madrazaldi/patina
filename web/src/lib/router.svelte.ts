// In-memory navigation with back/forward history. Each history entry
// remembers the scroll position of the content area, so going back lands
// where you left off.

import type { Metadata } from "./types";

export type Route =
  | { name: "home" }
  | { name: "library"; id: string }
  | { name: "item"; ratingKey: string }
  | { name: "browse"; path: string; title: string }
  | { name: "search" }
  | { name: "playlists" }
  | { name: "playlist"; id: string }
  | { name: "watchlist" }
  | { name: "activity" }
  | { name: "server" }
  | { name: "settings" };

interface Entry {
  route: Route;
  scroll: number;
}

export const routeKey = (r: Route) => JSON.stringify(r);

/** Where clicking an item should go. */
export function routeFor(item: Metadata): Route {
  return item.type === "playlist"
    ? { name: "playlist", id: item.ratingKey ?? "" }
    : { name: "item", ratingKey: item.ratingKey ?? "" };
}

export const browseRoute = (path: string, title: string): Route => ({ name: "browse", path, title });

/** "/item/100" style paths (used by PATINA_ROUTE in debug builds). */
export function parseRoute(path: string): Route | null {
  const [, a, b] = path.split("/");
  switch (a) {
    case "":
      return { name: "home" };
    case "library":
      return b ? { name: "library", id: b } : null;
    case "item":
      return b ? { name: "item", ratingKey: b } : null;
    case "playlist":
      return b ? { name: "playlist", id: b } : null;
    case "search":
    case "playlists":
    case "watchlist":
    case "activity":
    case "server":
    case "settings":
      return { name: a };
    default:
      return null;
  }
}

const scroller = () => document.getElementById("content");

class Navigator {
  #stack = $state<Entry[]>([{ route: { name: "home" }, scroll: 0 }]);
  #index = $state(0);

  get route(): Route {
    return this.#stack[this.#index].route;
  }
  get canBack() {
    return this.#index > 0;
  }
  get canForward() {
    return this.#index < this.#stack.length - 1;
  }

  is(route: Route) {
    return routeKey(route) === routeKey(this.route);
  }

  push(route: Route) {
    if (this.is(route)) return;
    this.#save();
    this.#stack = [...this.#stack.slice(0, this.#index + 1), { route, scroll: 0 }];
    this.#index = this.#stack.length - 1;
    this.#restore(0);
  }

  replace(route: Route) {
    this.#stack[this.#index] = { route, scroll: 0 };
    this.#restore(0);
  }

  back() {
    if (!this.canBack) return;
    this.#save();
    this.#index--;
    this.#restore(this.#stack[this.#index].scroll);
  }

  forward() {
    if (!this.canForward) return;
    this.#save();
    this.#index++;
    this.#restore(this.#stack[this.#index].scroll);
  }

  /** Forget history (e.g. after switching servers). */
  reset(route: Route = { name: "home" }) {
    this.#stack = [{ route, scroll: 0 }];
    this.#index = 0;
    this.#restore(0);
  }

  #save() {
    const el = scroller();
    if (el) this.#stack[this.#index].scroll = el.scrollTop;
  }

  // Content loads asynchronously, so keep trying until the page is tall
  // enough (or give up after a couple of seconds, or when the user scrolls).
  #restore(target: number) {
    const started = performance.now();
    const step = () => {
      const el = scroller();
      if (!el) return;
      if (target === 0) {
        el.scrollTop = 0;
        return;
      }
      if (el.scrollHeight - el.clientHeight >= target) {
        el.scrollTop = target;
      } else if (performance.now() - started < 2000) {
        requestAnimationFrame(step);
      }
    };
    queueMicrotask(() => requestAnimationFrame(step));
  }
}

export const nav = new Navigator();
