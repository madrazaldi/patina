// Port of the "Item actions" section of desktop/commands/index.ts.

import Foundation
import PlexKit

extension Commands {
    /// Which artwork `artwork` and `setArtwork` mean.
    public enum ArtworkKind: String, Sendable {
        case poster, art
    }

    /// Mark an item (or a whole show, season or album) watched or unwatched; announces itself
    /// with a toast that offers Undo.
    public static func setWatched(_ app: AppState, server: String? = nil, ratingKey: String, watched: Bool) async throws {
        try await Undo.setWatched(app, server, [ratingKey], watched)
    }

    public static func setWatchedMany(_ app: AppState, ratingKeys: [String], watched: Bool) async throws {
        try await Undo.setWatched(app, nil, ratingKeys, watched)
    }

    public static func rate(_ app: AppState, ratingKey: String, rating: Double?) async throws {
        try await app.server().rate(ratingKey, rating: rating)
    }

    /// Choose the audio and/or subtitle stream of a part (0 turns subtitles off).
    public static func selectStreams(
        _ app: AppState, server: String? = nil, partId: Int, audio: Int? = nil, subtitle: Int? = nil
    ) async throws {
        try await Servers.serverFor(app, server).selectStreams(partId, audio: audio, subtitle: subtitle, allParts: true)
    }

    public static func refreshMetadata(_ app: AppState, ratingKey: String) async throws {
        try await app.server().refreshMetadata(ratingKey, force: true)
    }

    public static func analyzeMetadata(_ app: AppState, ratingKey: String) async throws {
        try await app.server().analyzeMetadata(ratingKey)
    }

    public static func removeFromContinueWatching(_ app: AppState, ratingKey: String) async throws {
        try await Undo.removeFromContinueWatching(app, ratingKey)
    }

    /// Forget the resume position without changing the watched state.
    public static func clearProgress(_ app: AppState, ratingKey: String) async throws {
        try await Undo.clearProgress(app, ratingKey)
    }

    public static func deleteItem(_ app: AppState, ratingKey: String) async throws {
        try await app.server().deleteMetadata(ratingKey)
    }

    /// Edit fields (locking them so refreshes keep the change) and tags.
    public static func editMetadata(
        _ app: AppState, ratingKey: String, fields: [(String, String)], tags: [Ops.Edit.TagEdit]
    ) async throws {
        try await Ops.Edit.editMetadata(app, ratingKey, fields, tags)
    }

    /// Poster or background choices.
    public static func artwork(_ app: AppState, ratingKey: String, kind: ArtworkKind) async throws -> [Metadata] {
        let s = try app.server()
        return kind == .art ? try await s.arts(ratingKey) : try await s.posters(ratingKey)
    }

    public static func setArtwork(_ app: AppState, ratingKey: String, kind: ArtworkKind, url: String) async throws {
        let s = try app.server()
        if kind == .art { try await s.setArt(ratingKey, url: url) } else { try await s.setPoster(ratingKey, url: url) }
    }

    public static func matches(_ app: AppState, ratingKey: String, title: String? = nil, year: Int? = nil) async throws
        -> [MatchResult]
    {
        let title = title.flatMap { $0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : $0 }
        return try await app.server().matches(ratingKey, title: title, year: year)
    }

    public static func applyMatch(_ app: AppState, ratingKey: String, guid: String, name: String? = nil) async throws {
        try await app.server().applyMatch(ratingKey, guid: guid, name: name)
    }

    public static func unmatch(_ app: AppState, ratingKey: String) async throws {
        try await app.server().unmatch(ratingKey)
    }

    public static func searchSubtitles(
        _ app: AppState, ratingKey: String, language: String, hearingImpaired: Bool = false, forced: Bool = false
    ) async throws -> [MediaStream] {
        try await app.server().searchSubtitles(ratingKey, language: language, hearingImpaired: hearingImpaired, forced: forced)
    }

    public static func downloadSubtitle(_ app: AppState, ratingKey: String, key: String) async throws {
        try await app.server().downloadSubtitle(ratingKey, key: key)
    }

    public static func deleteStream(_ app: AppState, id: Int) async throws {
        try await app.server().deleteStream(id)
    }
}
