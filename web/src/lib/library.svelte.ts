import { app } from "./state.svelte";
import type { MediaContainer, Metadata, SavedView } from "./types";

export const PAGE = 120;
type Loader = (start: number, size: number) => Promise<MediaContainer>;

/**
 * Sparse page cache: a jump to Z never loads A–Y. Old results survive refresh failures.
 * A change on the server marks cached pages stale instead of dropping them: the pages in view refetch at once,
 * the others when they scroll into view, and everything stays on screen meanwhile.
 */
export class LibraryPages {
  pages = $state<Record<number, Metadata[]>>({});
  total = $state<number | null>(null);
  error = $state<string>();
  loading = $state(0);
  key = "";
  revision = -1;
  #generation = 0;
  #pending = new Set<number>();
  #failed = new Set<number>();
  /** Cached pages that may be out of date; they still display until refetched. */
  #stale = new Set<number>();
  #load?: Loader;
  #visible = 0;
  #visibleEnd = 0;

  /** `section` is the library the pages belong to: revisions that don't touch it leave the cache alone. */
  configure(key: string, revision: number, load: Loader, section?: string) {
    this.#load = load;
    if (this.key !== key) {
      this.detach();
      this.#load = load;
      this.key = key;
      this.pages = {};
      this.#stale.clear();
      this.total = null;
      this.error = undefined;
      this.revision = revision;
      this.ensure(0, PAGE);
      return;
    }
    if (this.revision !== revision) {
      const affected = section === undefined || app.affects(section, this.revision);
      this.revision = revision;
      if (affected) {
        this.refresh();
        return;
      }
    }
    if (this.total === null) this.ensure(0, PAGE);
  }
  at(index: number) { return this.pages[Math.floor(index / PAGE) * PAGE]?.[index % PAGE]; }
  /** Make sure the pages covering items `start` to `end` are loaded and current. */
  ensure(start: number, end: number) {
    this.#visible = start;
    this.#visibleEnd = end;
    for (let offset = Math.floor(start / PAGE) * PAGE; offset < Math.max(end, start + 1); offset += PAGE) {
      if (!this.pages[offset] || this.#stale.has(offset)) void this.#fetch(offset);
    }
  }
  /** Everything cached may be out of date: refetch what's in view now and the rest lazily. */
  refresh() {
    this.#generation++;
    this.#pending.clear();
    this.#failed.clear();
    this.loading = 0;
    this.error = undefined;
    for (const offset of Object.keys(this.pages).map(Number)) this.#stale.add(offset);
    this.ensure(this.#visible, this.#visibleEnd);
  }
  detach() { this.#generation++; this.#pending.clear(); this.#failed.clear(); this.loading = 0; this.#load = undefined; }
  async #fetch(offset: number) {
    if (!this.#load || this.#pending.has(offset) || this.#failed.has(offset)) return;
    const generation = this.#generation;
    this.#pending.add(offset);
    this.loading++;
    try {
      const mc = await this.#load(offset, PAGE);
      if (generation !== this.#generation) return;
      this.total = mc.totalSize ?? (offset + mc.Metadata.length);
      this.pages[offset] = mc.Metadata;
      this.#stale.delete(offset);
      // Bound memory as well as DOM size. Retain pages nearest the current viewport.
      const offsets = Object.keys(this.pages).map(Number).sort((a, b) => Math.abs(a - this.#visible) - Math.abs(b - this.#visible));
      for (const key of offsets.slice(12)) { delete this.pages[key]; this.#stale.delete(key); }
    } catch (e) {
      if (generation !== this.#generation) return;
      this.error = String(e);
      this.#failed.add(offset);
    } finally {
      if (generation === this.#generation) { this.#pending.delete(offset); this.loading--; }
    }
  }
}

export interface LibraryMemory {
  query: Omit<SavedView, "id" | "name" | "scope" | "library">;
  tab: "browse" | "recommended" | "collections" | "folders";
  trail: { key?: string; title: string }[];
  focus?: number;
  pages: LibraryPages;
}
export const libraryMemory = new Map<string, LibraryMemory>();
export function rememberLibrary(key: string, value: LibraryMemory) {
  libraryMemory.delete(key);
  libraryMemory.set(key, value);
  while (libraryMemory.size > 8) libraryMemory.delete(libraryMemory.keys().next().value!);
}
