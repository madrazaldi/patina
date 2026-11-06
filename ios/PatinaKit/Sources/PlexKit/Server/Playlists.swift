// Port of desktop/plex/server/playlists.ts.
// Playlists (port of crates/plex/src/server/playlists.rs).

import Foundation

extension PlexServer {
    /// All playlists; `kind` is `video`, `audio` or `photo`.
    public func playlists(_ kind: String? = nil, smart: Bool? = nil) async throws -> [Metadata] {
        let p = Params().opt("playlistType", kind).opt("smart", smart.map { $0 ? 1 : 0 })
        return try await get("/playlists", p).metadata
    }

    public func playlist(_ id: String) async throws -> Metadata? {
        try await get("/playlists/\(id)").metadata.first
    }

    public func playlistItems(_ id: String, start: Int, size: Int) async throws -> MediaContainer {
        let p = Params().set("X-Plex-Container-Start", start).set("X-Plex-Container-Size", size)
        return try await get("/playlists/\(id)/items", p)
    }

    /// Create a playlist from item URIs (see `itemUri`).
    /// For smart playlists pass a library URI with filters and `smart = true`.
    public func createPlaylist(_ title: String, kind: String, uri: String, smart: Bool) async throws -> Metadata? {
        let p = Params().set("title", title).set("type", kind).set("uri", uri).set("smart", smart ? 1 : 0)
        return try await container("POST", "/playlists", p).metadata.first
    }

    public func addToPlaylist(_ id: String, uri: String) async throws {
        try await put("/playlists/\(id)/items", Params().set("uri", uri))
    }

    public func removeFromPlaylist(_ id: String, playlistItemId: Int) async throws {
        try await delete("/playlists/\(id)/items/\(playlistItemId)")
    }

    /// Move an item after another one; `after = nil` moves it to the top.
    public func movePlaylistItem(_ id: String, playlistItemId: Int, after: Int? = nil) async throws {
        try await put("/playlists/\(id)/items/\(playlistItemId)/move", Params().opt("after", after))
    }

    public func clearPlaylist(_ id: String) async throws {
        try await delete("/playlists/\(id)/items")
    }

    public func updatePlaylist(_ id: String, title: String? = nil, summary: String? = nil) async throws {
        try await put("/playlists/\(id)", Params().opt("title", title).opt("summary", summary))
    }

    public func deletePlaylist(_ id: String) async throws {
        try await delete("/playlists/\(id)")
    }

    /// Import an .m3u file that lives on the server's filesystem.
    public func uploadPlaylist(_ sectionId: String, path: String, force: Bool) async throws {
        try await post(
            "/playlists/upload", Params().set("sectionID", sectionId).set("path", path).flag("force", force))
    }
}
