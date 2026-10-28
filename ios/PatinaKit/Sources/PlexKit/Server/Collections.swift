// Port of desktop/plex/server/collections.ts.
// Collections (port of crates/plex/src/server/collections.rs).

import Foundation

extension PlexServer {
    public func collections(_ sectionId: String) async throws -> [Metadata] {
        try await get("/library/sections/\(sectionId)/collections", Params().set("includeCollections", 1)).metadata
    }

    public func collectionItems(_ collectionId: String) async throws -> MediaContainer {
        try await get("/library/collections/\(collectionId)/children")
    }

    /// Create a collection. `kind` is the media type id of its items.
    public func createCollection(_ sectionId: String, title: String, kind: Int, uri: String?, smart: Bool)
        async throws -> Metadata?
    {
        let p = Params()
            .set("sectionId", sectionId)
            .set("title", title)
            .set("type", kind)
            .set("smart", smart ? 1 : 0)
            .opt("uri", uri)
        return try await container("POST", "/library/collections", p).metadata.first
    }

    public func addToCollection(_ collectionId: String, uri: String) async throws {
        try await put("/library/collections/\(collectionId)/items", Params().set("uri", uri))
    }

    public func removeFromCollection(_ collectionId: String, ratingKey: String) async throws {
        try await delete("/library/collections/\(collectionId)/items/\(ratingKey)")
    }

    /// Move an item inside a custom-sorted collection.
    public func moveCollectionItem(_ collectionId: String, ratingKey: String, after: String? = nil) async throws {
        try await put("/library/collections/\(collectionId)/items/\(ratingKey)/move", Params().opt("after", after))
    }

    public func deleteCollection(_ collectionId: String) async throws {
        try await delete("/library/collections/\(collectionId)")
    }

    /// Collection mode: -1 library default, 0 hide, 1 hide items, 2 show items.
    public func setCollectionMode(_ collectionId: String, mode: Int) async throws {
        try await setMetadataPrefs(collectionId, params: Params().set("collectionMode", mode))
    }

    /// Collection sort: 0 release date, 1 alphabetical, 2 custom.
    public func setCollectionSort(_ collectionId: String, sort: Int) async throws {
        try await setMetadataPrefs(collectionId, params: Params().set("collectionSort", sort))
    }
}
