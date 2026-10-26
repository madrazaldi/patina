// Saved library views: named queries (type, sort, filters) of one library,
// kept per account and server in the shared `config.toml` under
// `ui.saved_views`. The desktop app reads and writes the same entries, so a
// view saved in either front end is there in both.
//
// The entries are stored as the JSON the desktop app writes
// (`web/src/lib/types.ts`, `SavedView`). Fields this code doesn't know are
// kept as they are when an entry is rewritten, and entries that don't parse
// are left alone.

import * as offline from "../offline";
import type { AppState, SessionView } from "../state";

/** A value filter ("Genre: Drama"). */
export interface Picked {
  filter: string;
  title: string;
  value: string;
  label: string;
}

/** What only the terminal client adds to a view; the desktop app ignores it. */
export interface TuiExtras {
  hideWatched: boolean;
  /** Text typed into the column's `/` filter. */
  filter: string;
}

/** Fields written by other versions or front ends are kept as extra properties (serde `flatten`). */
export interface SavedView {
  id: string;
  name: string;
  /** `account:server`, see `scope`. */
  scope: string;
  /** The library's section id. */
  library: string;
  /** Plex media type listed; 0 for the library's default. */
  mediaType: number;
  /** Plex sort key, e.g. `titleSort`; `desc` reverses it. */
  sort: string;
  desc: boolean;
  unwatched: boolean;
  /** Boolean filters that are on, by key. */
  toggles: string[];
  picked: Picked[];
  tui?: TuiExtras;
  [extra: string]: unknown;
}

const KNOWN = ["id", "name", "scope", "library", "mediaType", "sort", "desc", "unwatched", "toggles", "picked", "tui"];

/** `SavedView::default()`. */
function defaultView(): SavedView {
  return {
    id: "",
    name: "",
    scope: "",
    library: "",
    mediaType: 0,
    sort: "",
    desc: false,
    unwatched: false,
    toggles: [],
    picked: [],
  };
}

/** A new view of `library` for `scope`, with a fresh id. */
export function newSavedView(scope: string, library: string, name: string): SavedView {
  return { ...defaultView(), id: crypto.randomUUID(), name: name.trim(), scope, library, sort: "titleSort" };
}

/** The scope views are saved under: the same `accountId:serverId` the desktop app uses. */
export function scopeOf(session: Pick<SessionView, "accountId" | "serverId">): string {
  return `${session.accountId}:${session.serverId}`;
}

/** The current account and server's scope; `null` when not connected. */
export function scope(app: AppState): string | null {
  const s = app.session();
  return s ? `${offline.account(s)}:${s.serverId}` : null;
}

/** The views of `library` on the connected server, in the order saved. */
export function list(app: AppState, library: string): SavedView[] {
  const sc = scope(app);
  if (sc === null) return [];
  return matching(app.settings().ui.saved_views, sc, library);
}

/** Save `view`. A view of the same library and name (in the same scope) is replaced. */
export function save(app: AppState, view: SavedView): void {
  if (view.name.trim() === "") throw new Error("A saved view needs a name.");
  const value = toJson(view);
  app.tryUpdateSettings((s) => {
    upsert(s.ui.saved_views, view, value);
  });
}

/** Delete the view with this id. */
export function remove(app: AppState, id: string): void {
  app.tryUpdateSettings((s) => {
    forget(s.ui.saved_views, id);
  });
}

type Obj = Record<string, unknown>;

function isObj(v: unknown): v is Obj {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

/** `serde_json::to_value(&view)`: the known fields, `tui` only when present, then the extras. */
export function toJson(view: SavedView): Obj {
  const out: Obj = {
    id: view.id,
    name: view.name,
    scope: view.scope,
    library: view.library,
    mediaType: view.mediaType,
    sort: view.sort,
    desc: view.desc,
    unwatched: view.unwatched,
    toggles: view.toggles,
    picked: view.picked,
  };
  if (view.tui !== undefined) out.tui = { hideWatched: view.tui.hideWatched, filter: view.tui.filter };
  for (const [k, v] of Object.entries(view)) if (!KNOWN.includes(k)) out[k] = v;
  return structuredClone(out);
}

function str(o: Obj, key: string, fallback: string): string | null {
  const v = o[key];
  if (v === undefined) return fallback;
  return typeof v === "string" ? v : null;
}

function bool(o: Obj, key: string): boolean | null {
  const v = o[key];
  if (v === undefined) return false;
  return typeof v === "boolean" ? v : null;
}

/** `serde_json::from_value::<SavedView>` (every field defaulted), `null` when it does not fit. */
function fromJson(v: unknown): SavedView | null {
  if (!isObj(v)) return null;
  const out = defaultView();
  const id = str(v, "id", "");
  const name = str(v, "name", "");
  const scopeText = str(v, "scope", "");
  const library = str(v, "library", "");
  const sort = str(v, "sort", "");
  const desc = bool(v, "desc");
  const unwatched = bool(v, "unwatched");
  if (id === null || name === null || scopeText === null || library === null || sort === null || desc === null || unwatched === null) {
    return null;
  }
  Object.assign(out, { id, name, scope: scopeText, library, sort, desc, unwatched });
  if (v.mediaType !== undefined) {
    if (typeof v.mediaType !== "number" || !Number.isInteger(v.mediaType)) return null;
    out.mediaType = v.mediaType;
  }
  if (v.toggles !== undefined) {
    if (!Array.isArray(v.toggles) || !v.toggles.every((t) => typeof t === "string")) return null;
    out.toggles = [...(v.toggles as string[])];
  }
  if (v.picked !== undefined) {
    if (!Array.isArray(v.picked)) return null;
    const picked: Picked[] = [];
    for (const p of v.picked) {
      // `Picked` has no defaults: all four fields are required.
      if (!isObj(p) || ![p.filter, p.title, p.value, p.label].every((x) => typeof x === "string")) return null;
      picked.push({ filter: p.filter as string, title: p.title as string, value: p.value as string, label: p.label as string });
    }
    out.picked = picked;
  }
  if (v.tui !== undefined && v.tui !== null) {
    if (!isObj(v.tui)) return null;
    const hideWatched = bool(v.tui, "hideWatched");
    const filter = str(v.tui, "filter", "");
    if (hideWatched === null || filter === null) return null;
    out.tui = { hideWatched, filter };
  }
  for (const [k, val] of Object.entries(v)) if (!KNOWN.includes(k)) out[k] = structuredClone(val);
  return out;
}

export function parse(v: unknown): SavedView | null {
  const view = fromJson(v);
  return view !== null && view.id !== "" ? view : null;
}

export function matching(stored: readonly unknown[], scopeText: string, library: string): SavedView[] {
  const out: SavedView[] = [];
  for (const v of stored) {
    const view = parse(v);
    if (view !== null && view.scope === scopeText && view.library === library) out.push(view);
  }
  return out;
}

export function upsert(stored: unknown[], view: SavedView, value: Obj): void {
  const same = (v: unknown) => {
    const old = parse(v);
    return old !== null && old.scope === view.scope && old.library === view.library && asciiLower(old.name) === asciiLower(view.name);
  };
  const at = stored.findIndex(same);
  if (at >= 0) {
    // Keep the id so anything pointing at the view still does.
    const old = stored[at];
    if (isObj(old) && old.id !== undefined) value.id = old.id;
    stored[at] = value;
  } else {
    stored.push(value);
  }
}

/** `eq_ignore_ascii_case`: only ASCII letters fold. */
function asciiLower(s: string): string {
  return s.replace(/[A-Z]/g, (c) => c.toLowerCase());
}

export function forget(stored: unknown[], id: string): void {
  for (let i = stored.length - 1; i >= 0; i--) {
    const v = stored[i];
    if (isObj(v) && v.id === id) stored.splice(i, 1);
  }
}
