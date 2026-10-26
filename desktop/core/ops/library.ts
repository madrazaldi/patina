// Library browsing beyond plain listings: filters, folders, related hubs.

import { type Directory, type Hub, type LibraryMetaType, type Metadata, type PlexServer, defaultHub } from "../../plex";
import type { AppState } from "../state";
import { mediaTypeId } from "./util";

export interface FilterView {
  filter: string;
  title: string;
  /** `string`, `integer`, `tag` or `boolean`. */
  kind: string;
}

/** Filters for a section and media type (genre, year, resolution, unwatched, ...). */
export async function sectionFilters(app: AppState, id: string, kind?: number | null): Promise<FilterView[]> {
  const server = app.server();
  const want = kind ?? undefined;
  const view = (d: Directory): FilterView | null => {
    if (d.filter == null) return null;
    return { filter: d.filter, title: d.title ?? "", kind: d.filterType ?? "string" };
  };
  const views = (list: Directory[]) => list.map(view).filter((v): v is FilterView => v !== null);
  const meta = await server.sectionMeta(id);
  if (meta) {
    const ofKind = (t: LibraryMetaType) => (t.type != null ? mediaTypeId(t.type) : undefined) === want;
    const found = meta.Type.find(ofKind) ?? meta.Type[0];
    if (found) return views(found.Filter);
  }
  return views(await server.sectionFilters(id));
}

export interface FolderView {
  title: string | null;
  directories: Directory[];
  items: Metadata[];
}

/** A section's folder tree: the root, or the folder at `key`. */
export async function sectionFolders(app: AppState, id: string, key?: string | null): Promise<FolderView> {
  const mc = await app.server().sectionFolders(id, key ?? undefined);
  return { title: mc.title2 ?? mc.title1 ?? null, directories: mc.Directory, items: mc.Metadata };
}

/** Extra hubs for a detail page, plus sonically similar music. */
export async function itemHubs(app: AppState, ratingKey: string, kind: string): Promise<Hub[]> {
  return itemHubsOn(app.server(), ratingKey, kind);
}

/** `itemHubs` on any server (an item found on another server of the account). */
export async function itemHubsOn(server: PlexServer, ratingKey: string, kind: string): Promise<Hub[]> {
  const hubs: Hub[] = await server.metadataHubs(ratingKey, 12).catch(() => []);
  let similar: [string, Metadata[]] | null = null;
  if (kind === "track" || kind === "album") {
    similar = await server.nearest(ratingKey).then((m): [string, Metadata[]] => ["Sonically similar", m], () => null);
  } else if (kind === "artist") {
    similar = await server.similar(ratingKey).then((m): [string, Metadata[]] => ["Similar artists", m], () => null);
  }
  if (similar && similar[1].length > 0) {
    hubs.push(
      defaultHub({
        title: similar[0],
        hubIdentifier: `patina.${kind}.similar`,
        Metadata: similar[1].filter((m) => m.ratingKey !== ratingKey),
      }),
    );
  }
  return hubs;
}
