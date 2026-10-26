// Collections (port of crates/plex/src/server/collections.rs).

import { Params } from "../client";
import type { MediaContainer, Metadata } from "../models";
import { PlaylistsApi } from "./playlists";

export class CollectionsApi extends PlaylistsApi {
  async collections(sectionId: string): Promise<Metadata[]> {
    return (
      await this.get(`/library/sections/${sectionId}/collections`, new Params().set("includeCollections", 1))
    ).Metadata;
  }

  collectionItems(collectionId: string): Promise<MediaContainer> {
    return this.get(`/library/collections/${collectionId}/children`, new Params());
  }

  /** Create a collection. `kind` is the media type id of its items. */
  async createCollection(
    sectionId: string,
    title: string,
    kind: number,
    uri: string | null | undefined,
    smart: boolean,
  ): Promise<Metadata | null> {
    const p = new Params()
      .set("sectionId", sectionId)
      .set("title", title)
      .set("type", kind)
      .set("smart", smart ? 1 : 0)
      .opt("uri", uri);
    return (await this.container("POST", "/library/collections", p)).Metadata[0] ?? null;
  }

  addToCollection(collectionId: string, uri: string): Promise<void> {
    return this.put(`/library/collections/${collectionId}/items`, new Params().set("uri", uri));
  }

  removeFromCollection(collectionId: string, ratingKey: string): Promise<void> {
    return this.delete(`/library/collections/${collectionId}/items/${ratingKey}`, new Params());
  }

  /** Move an item inside a custom-sorted collection. */
  moveCollectionItem(collectionId: string, ratingKey: string, after?: string | null): Promise<void> {
    return this.put(`/library/collections/${collectionId}/items/${ratingKey}/move`, new Params().opt("after", after));
  }

  deleteCollection(collectionId: string): Promise<void> {
    return this.delete(`/library/collections/${collectionId}`, new Params());
  }

  /** Collection mode: -1 library default, 0 hide, 1 hide items, 2 show items. */
  setCollectionMode(collectionId: string, mode: number): Promise<void> {
    return this.setMetadataPrefs(collectionId, new Params().set("collectionMode", mode));
  }

  /** Collection sort: 0 release date, 1 alphabetical, 2 custom. */
  setCollectionSort(collectionId: string, sort: number): Promise<void> {
    return this.setMetadataPrefs(collectionId, new Params().set("collectionSort", sort));
  }
}
