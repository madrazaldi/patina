// Editing items: fields, tags and collections.

import type { AppState } from "../state";
import { mediaTypeId } from "./util";

export interface TagEdit {
  /** `genre`, `label`, `collection`, `mood`, ... */
  tag: string;
  add: string[];
  remove: string[];
}

/** Edit fields (locking them so refreshes keep the change) and tags. */
export async function editMetadata(
  app: AppState,
  ratingKey: string,
  fields: readonly (readonly [string, string])[],
  tags: readonly TagEdit[],
): Promise<void> {
  const server = app.server();
  const item = await server.metadata(ratingKey);
  if (!item) throw new Error("Item not found.");
  const kind = mediaTypeId(item.type ?? "");
  if (kind === undefined) throw new Error("This item cannot be edited.");
  if (fields.length > 0) {
    await server.editMetadata(ratingKey, item.librarySectionID ?? undefined, kind, fields.map(([k, v]) => [k, v] as [string, string]), true);
  }
  for (const t of tags.filter((t) => t.add.length > 0 || t.remove.length > 0)) {
    await server.editTags(ratingKey, kind, t.tag, t.add, t.remove);
  }
}

/** Add an item to a collection, or to a new one called `title`. */
export async function addToCollection(
  app: AppState,
  ratingKey: string,
  collection?: string | null,
  title?: string | null,
): Promise<void> {
  const server = app.server();
  const uri = server.itemUri([ratingKey]);
  if (collection != null) {
    await server.addToCollection(collection, uri);
  } else if (title != null) {
    const item = await server.metadata(ratingKey);
    if (!item) throw new Error("Item not found.");
    const section = item.librarySectionID;
    if (section == null) throw new Error("The item is not in a library.");
    const kind = mediaTypeId(item.type ?? "") ?? 1;
    await server.createCollection(String(section), title, kind, uri, false);
  } else {
    throw new Error("No collection given.");
  }
}
