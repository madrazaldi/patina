import Foundation

// Typed wrappers around the backend commands (crates/ffi/src/dispatch.rs),
// named as in web/src/lib/api.ts. Errors throw `CoreError` with the message.

struct PlayOptions {
    var resume = true
    var shuffle = false
    var mediaIndex = 0
    /// Play the item's radio station (music).
    var radio = false
}

enum Control: Encodable {
    case togglePause, play, pause, next, previous, stop
    case seekRelative(Double)
    case seekTo(Double)

    private enum Keys: String, CodingKey { case kind, seconds }
    func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: Keys.self)
        switch self {
        case .togglePause: try c.encode("togglePause", forKey: .kind)
        case .play: try c.encode("play", forKey: .kind)
        case .pause: try c.encode("pause", forKey: .kind)
        case .next: try c.encode("next", forKey: .kind)
        case .previous: try c.encode("previous", forKey: .kind)
        case .stop: try c.encode("stop", forKey: .kind)
        case .seekRelative(let s): try c.encode("seekRelative", forKey: .kind); try c.encode(s, forKey: .seconds)
        case .seekTo(let s): try c.encode("seekTo", forKey: .kind); try c.encode(s, forKey: .seconds)
        }
    }
}

/// Edits to the running queue; entries are addressed by `QueueItem.uid`.
enum QueueCommand: Encodable {
    case add(Metadata, next: Bool)
    case remove(UInt64)
    case move(UInt64, before: UInt64?)
    case jump(UInt64)
    case shuffle(Bool)
    case stopAfter(Bool)

    private enum Keys: String, CodingKey { case kind, item, next, uid, before, on }
    func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: Keys.self)
        switch self {
        case .add(let item, let next): try c.encode("add", forKey: .kind); try c.encode(item, forKey: .item); try c.encode(next, forKey: .next)
        case .remove(let uid): try c.encode("remove", forKey: .kind); try c.encode(uid, forKey: .uid)
        case .move(let uid, let before): try c.encode("move", forKey: .kind); try c.encode(uid, forKey: .uid); try c.encode(before, forKey: .before)
        case .jump(let uid): try c.encode("jump", forKey: .kind); try c.encode(uid, forKey: .uid)
        case .shuffle(let on): try c.encode("shuffle", forKey: .kind); try c.encode(on, forKey: .on)
        case .stopAfter(let on): try c.encode("stopAfter", forKey: .kind); try c.encode(on, forKey: .on)
        }
    }
}

struct TagEdit: Encodable {
    /// "genre", "label", "collection", "mood", ...
    var tag: String
    var add: [String]
    var remove: [String]
}

struct TrackQuery: Encodable {
    var show: String
    var season: Int?
    var reference: String?
    var audio: Int64?
    /// "keep", "off" or a stream id.
    var subtitle: String
}

/// Encodes `[a, b]` pairs, which the backend reads as tuples.
struct Pair: Encodable {
    var a: String
    var b: String
    init(_ a: String, _ b: String) { self.a = a; self.b = b }
    func encode(to encoder: Encoder) throws {
        var c = encoder.unkeyedContainer()
        try c.encode(a)
        try c.encode(b)
    }
}

enum API {
    // App
    static func snapshot() async throws -> Snapshot { try await Core.call("snapshot") }
    static func info() async throws -> Info { try await Core.call("info") }
    static func settings() async throws -> Settings { try await Core.call("settings") }
    /// `patch` holds snake_case `PlayerSettings` fields.
    static func setPlayerSettings(_ patch: [String: JSON]) async throws { try await Core.run("set_player_settings", ["player": patch]) }
    /// `patch` holds snake_case `UiSettings` fields.
    static func setUiSettings(_ patch: [String: JSON]) async throws { try await Core.run("set_ui_settings", ["ui": patch]) }
    static func probeMpv() async throws -> String { try await Core.call("probe_mpv") }
    static func openUrl(_ url: String) async throws { try await Core.run("open_url", ["url": url]) }
    static func openMpvConfig() async throws { try await Core.run("open_mpv_config") }

    // Session
    static func retry() async throws { try await Core.run("retry") }
    static func startPin(strong: Bool) async throws -> PinView { try await Core.call("start_pin", ["strong": strong]) }
    static func completePin(id: Int64, attempt: UInt64) async throws { try await Core.run("complete_pin", ["id": id, "attempt": attempt]) }
    static func cancelPin() async throws { try await Core.run("cancel_pin") }
    static func passwordSignIn(login: String, password: String, code: String?) async throws {
        try await Core.run("password_sign_in", ["login": login, "password": password, "code": code])
    }
    static func signInManual(url: String, token: String) async throws { try await Core.run("sign_in_manual", ["url": url, "token": token]) }
    static func pickServer(_ id: String) async throws { try await Core.run("pick_server", ["id": id]) }
    static func switchServer(_ id: String) async throws { try await Core.run("switch_server", ["id": id]) }
    static func switchUser(_ uuid: String, pin: String? = nil) async throws { try await Core.run("switch_user", ["uuid": uuid, "pin": pin]) }
    static func signOut() async throws { try await Core.run("sign_out") }
    static func reloadSections() async throws { try await Core.run("reload_sections") }
    static func homeUsers() async throws -> [HomeUser] { try await Core.call("home_users") }

    // Browsing
    static func homeHubs() async throws -> [Hub] { try await Core.call("home_hubs") }
    static func sectionHubs(_ id: String) async throws -> [Hub] { try await Core.call("section_hubs", ["id": id]) }
    static func sectionSorts(_ id: String) async throws -> [Directory] { try await Core.call("section_sorts", ["id": id]) }
    static func sectionFilterValues(_ id: String, filter: String, kind: Int?) async throws -> [Directory] {
        try await Core.call("section_filter_values", ["id": id, "filter": filter, "kind": kind])
    }
    /// `filters` are (filter, value) pairs, e.g. ("genre", "12"), ("year", "1999"), ("hdr", "1").
    static func sectionItems(_ id: String, kind: Int?, sort: String?, unwatched: Bool, filters: [(String, String)], start: Int, size: Int) async throws -> MediaContainer {
        try await Core.call("section_items", [
            "id": id, "kind": kind, "sort": sort, "unwatched": unwatched,
            "filters": filters.map { Pair($0.0, $0.1) }, "start": start, "size": size,
        ])
    }
    static func sectionFilters(_ id: String, kind: Int?) async throws -> [FilterView] { try await Core.call("section_filters", ["id": id, "kind": kind]) }
    static func firstCharacters(_ id: String, kind: Int?) async throws -> [Directory] { try await Core.call("first_characters", ["id": id, "kind": kind]) }
    /// Folder view; `key` is a directory key from a previous call, or nil for the top.
    static func sectionFolders(_ id: String, key: String?) async throws -> FolderView { try await Core.call("section_folders", ["id": id, "key": key]) }
    /// More hubs for a detail page (related, sonically similar, similar artists).
    static func itemHubs(_ ratingKey: String, kind: String) async throws -> [Hub] { try await Core.call("item_hubs", ["ratingKey": ratingKey, "kind": kind]) }
    static func collections(_ id: String) async throws -> [Metadata] { try await Core.call("collections", ["id": id]) }
    static func listing(_ path: String, start: Int, size: Int) async throws -> MediaContainer {
        try await Core.call("listing", ["path": path, "start": start, "size": size])
    }
    static func search(_ query: String) async throws -> [Hub] { try await Core.call("search", ["query": query]) }
    static func metadata(_ ratingKey: String) async throws -> Metadata? { try await Core.call("metadata", ["ratingKey": ratingKey]) }
    static func children(_ ratingKey: String, collection: Bool = false) async throws -> [Metadata] {
        try await Core.call("children", ["ratingKey": ratingKey, "collection": collection])
    }
    static func findByGuid(_ guid: String) async throws -> String? { try await Core.call("find_by_guid", ["guid": guid]) }

    // Item actions
    static func setWatched(_ ratingKey: String, _ watched: Bool) async throws { try await Core.run("set_watched", ["ratingKey": ratingKey, "watched": watched]) }
    static func rate(_ ratingKey: String, _ rating: Double?) async throws { try await Core.run("rate", ["ratingKey": ratingKey, "rating": rating]) }
    /// Choose the audio and/or subtitle stream of a part (subtitle 0 turns subtitles off).
    static func selectStreams(partId: Int, audio: Int?, subtitle: Int?) async throws {
        try await Core.run("select_streams", ["partId": partId, "audio": audio, "subtitle": subtitle])
    }
    static func refreshMetadata(_ ratingKey: String) async throws { try await Core.run("refresh_metadata", ["ratingKey": ratingKey]) }
    static func analyzeMetadata(_ ratingKey: String) async throws { try await Core.run("analyze_metadata", ["ratingKey": ratingKey]) }
    static func removeFromContinueWatching(_ ratingKey: String) async throws { try await Core.run("remove_from_continue_watching", ["ratingKey": ratingKey]) }
    static func clearProgress(_ ratingKey: String) async throws { try await Core.run("clear_progress", ["ratingKey": ratingKey]) }
    /// Deletes the item and its files (the server must allow media deletion).
    static func deleteItem(_ ratingKey: String) async throws { try await Core.run("delete_item", ["ratingKey": ratingKey]) }
    /// `fields` are (field, value) pairs: title, titleSort, originalTitle, summary, tagline, studio,
    /// contentRating, year, originallyAvailableAt. Edited fields are locked.
    static func editMetadata(_ ratingKey: String, fields: [(String, String)], tags: [TagEdit] = []) async throws {
        try await Core.run("edit_metadata", ["ratingKey": ratingKey, "fields": fields.map { Pair($0.0, $0.1) }, "tags": tags])
    }
    /// `kind` is "poster" or "art".
    static func artwork(_ ratingKey: String, kind: String) async throws -> [ArtworkChoice] { try await Core.call("artwork", ["ratingKey": ratingKey, "kind": kind]) }
    static func setArtwork(_ ratingKey: String, kind: String, url: String) async throws {
        try await Core.run("set_artwork", ["ratingKey": ratingKey, "kind": kind, "url": url])
    }
    static func matches(_ ratingKey: String, title: String?, year: Int?) async throws -> [MatchResult] {
        try await Core.call("matches", ["ratingKey": ratingKey, "title": title, "year": year])
    }
    static func applyMatch(_ ratingKey: String, guid: String, name: String?) async throws {
        try await Core.run("apply_match", ["ratingKey": ratingKey, "guid": guid, "name": name])
    }
    static func unmatch(_ ratingKey: String) async throws { try await Core.run("unmatch", ["ratingKey": ratingKey]) }
    static func searchSubtitles(_ ratingKey: String, language: String, hearingImpaired: Bool = false, forced: Bool = false) async throws -> [Stream] {
        try await Core.call("search_subtitles", ["ratingKey": ratingKey, "language": language, "hearingImpaired": hearingImpaired, "forced": forced])
    }
    static func downloadSubtitle(_ ratingKey: String, key: String) async throws { try await Core.run("download_subtitle", ["ratingKey": ratingKey, "key": key]) }
    static func deleteStream(_ id: Int) async throws { try await Core.run("delete_stream", ["id": id]) }

    // Collections
    /// Add to an existing collection, or to a new one called `title`.
    static func addToCollection(_ ratingKey: String, collection: String?, title: String?) async throws {
        try await Core.run("add_to_collection", ["ratingKey": ratingKey, "collection": collection, "title": title])
    }
    static func removeFromCollection(_ id: String, ratingKey: String) async throws { try await Core.run("remove_from_collection", ["id": id, "ratingKey": ratingKey]) }
    /// Custom-sorted collections only; `after` nil moves to the top.
    static func moveCollectionItem(_ id: String, ratingKey: String, after: String?) async throws {
        try await Core.run("move_collection_item", ["id": id, "ratingKey": ratingKey, "after": after])
    }
    static func deleteCollection(_ id: String) async throws { try await Core.run("delete_collection", ["id": id]) }
    /// mode: -1 library default, 0 hide collection, 1 hide items, 2 show both. sort: 0 release date, 1 title, 2 custom.
    static func collectionSettings(_ id: String, mode: Int? = nil, sort: Int? = nil) async throws {
        try await Core.run("collection_settings", ["id": id, "mode": mode, "sort": sort])
    }

    // Watchlist
    static func watchlist() async throws -> [Metadata] { try await Core.call("watchlist") }
    static func watchlisted(_ discoverId: String) async throws -> Bool { try await Core.call("watchlisted", ["discoverId": discoverId]) }
    static func setWatchlisted(_ discoverId: String, _ on: Bool) async throws { try await Core.run("set_watchlisted", ["discoverId": discoverId, "on": on]) }

    // Playlists
    /// All playlists, or the non-smart ones of one kind (`video`, `audio`, `photo`).
    static func playlists(kind: String? = nil) async throws -> [Metadata] { try await Core.call("playlists", ["kind": kind]) }
    static func playlist(_ id: String) async throws -> PlaylistView? { try await Core.call("playlist", ["id": id]) }
    /// Add to an existing playlist, or to a new one called `title` of `kind`.
    static func addToPlaylist(_ ratingKey: String, kind: String, playlist: String?, title: String?) async throws {
        try await Core.run("add_to_playlist", ["ratingKey": ratingKey, "kind": kind, "playlist": playlist, "title": title])
    }
    static func removeFromPlaylist(_ id: String, itemId: Int) async throws { try await Core.run("remove_from_playlist", ["id": id, "itemId": itemId]) }
    static func deletePlaylist(_ id: String) async throws { try await Core.run("delete_playlist", ["id": id]) }
    /// `after` nil moves to the top.
    static func movePlaylistItem(_ id: String, itemId: Int, after: Int?) async throws {
        try await Core.run("move_playlist_item", ["id": id, "itemId": itemId, "after": after])
    }
    static func updatePlaylist(_ id: String, title: String, summary: String) async throws {
        try await Core.run("update_playlist", ["id": id, "title": title, "summary": summary])
    }
    static func clearPlaylist(_ id: String) async throws { try await Core.run("clear_playlist", ["id": id]) }

    // Playback
    static func play(_ item: Metadata, _ o: PlayOptions = PlayOptions()) async throws {
        struct Request: Encodable { var item: Metadata; var resume: Bool; var shuffle: Bool; var mediaIndex: Int; var radio: Bool }
        try await Core.run("play", ["request": Request(item: item, resume: o.resume, shuffle: o.shuffle, mediaIndex: o.mediaIndex, radio: o.radio)])
    }
    static func control(_ command: Control) async throws { try await Core.run("control", ["command": command]) }
    /// Adding while nothing plays starts playback.
    static func queueItems(_ items: [Metadata]) async throws { try await Core.run("queue_items", ["items": items]) }
    static func queue(_ command: QueueCommand) async throws { try await Core.run("queue", ["command": command]) }
    static func lyrics(_ ratingKey: String) async throws -> Lyrics? { try await Core.call("lyrics", ["ratingKey": ratingKey]) }

    // Downloads
    static func downloads() async throws -> [Download] { try await Core.call("downloads") }
    /// Queue an item (or every episode/track below it); returns how many were added.
    static func download(_ ratingKey: String) async throws -> Int { try await Core.call("download", ["ratingKey": ratingKey]) }
    static func removeDownload(_ id: String) async throws { try await Core.run("remove_download", ["id": id]) }
    static func pauseDownload(_ id: String) async throws { try await Core.run("pause_download", ["id": id]) }
    static func retryDownload(_ id: String) async throws { try await Core.run("retry_download", ["id": id]) }
    static func playDownloads(_ ids: [String]) async throws { try await Core.run("play_downloads", ["ids": ids]) }
    static func openDownloadsFolder() async throws { try await Core.run("open_downloads_folder") }

    // Live TV
    static func livetvAvailable() async throws -> Bool { try await Core.call("livetv_available") }
    static func livetvGuide(hours: Int) async throws -> [LiveChannel] { try await Core.call("livetv_guide", ["hours": hours]) }
    static func livetvWatch(dvr: String, channel: String, title: String) async throws {
        try await Core.run("livetv_watch", ["dvr": dvr, "channel": channel, "title": title])
    }
    static func recordings() async throws -> Recordings { try await Core.call("recordings") }
    static func record(_ guid: String) async throws { try await Core.run("record", ["guid": guid]) }
    static func deleteRecordingRule(_ id: String) async throws { try await Core.run("delete_recording_rule", ["id": id]) }

    // Bulk audio & subtitles. Progress arrives as `tracks-progress` events ([done, total]).
    static func tracksLoad(show: String) async throws { try await Core.run("tracks_load", ["show": show]) }
    static func tracksView(_ query: TrackQuery) async throws -> TrackView { try await Core.call("tracks_view", ["query": query]) }
    static func tracksApply(_ changes: [PartChange]) async throws { try await Core.run("tracks_apply", ["changes": changes]) }

    // Activity
    static func sessions() async throws -> [Metadata] { try await Core.call("sessions") }
    static func terminateSession(_ id: String) async throws { try await Core.run("terminate_session", ["id": id]) }
    static func history(page: Int) async throws -> HistoryPage { try await Core.call("history", ["page": page]) }
    static func deleteHistory(_ key: String) async throws { try await Core.run("delete_history", ["key": key]) }
    static func tasks() async throws -> Tasks { try await Core.call("tasks") }
    static func cancelActivity(_ uuid: String) async throws { try await Core.run("cancel_activity", ["uuid": uuid]) }
    static func killTranscode(_ key: String) async throws { try await Core.run("kill_transcode", ["key": key]) }

    // Server administration
    static func serverOverview() async throws -> Overview { try await Core.call("server_overview") }
    static func checkForUpdates() async throws { try await Core.run("check_for_updates") }
    static func applyUpdate(tonight: Bool) async throws { try await Core.run("apply_update", ["tonight": tonight]) }
    /// `action`: "scan", "refresh", "analyze" or "emptyTrash".
    static func sectionAction(_ id: String, _ action: String) async throws { try await Core.run("section_action", ["id": id, "action": action]) }
    /// `action`: "scanAll", "cleanBundles" or "optimizeDatabase".
    static func serverAction(_ action: String) async throws { try await Core.run("server_action", ["action": action]) }
    static func butlerTasks() async throws -> [ButlerTask] { try await Core.call("butler_tasks") }
    static func butlerTask(_ name: String, start: Bool) async throws { try await Core.run("butler_task", ["name": name, "start": start]) }
    static func butlerAll(start: Bool) async throws { try await Core.run("butler_all", ["start": start]) }
    static func prefs() async throws -> [Pref] { try await Core.call("prefs") }
    static func setPref(_ id: String, _ value: String) async throws { try await Core.run("set_pref", ["id": id, "value": value]) }
    static func usersDevices() async throws -> UsersDevices { try await Core.call("users_devices") }
    static func libraryChoices(kind: String) async throws -> LibraryChoices { try await Core.call("library_choices", ["kind": kind]) }
    static func createLibrary(_ form: LibraryForm) async throws { try await Core.run("create_library", ["form": form]) }
    static func editLibrary(_ id: String, name: String, locations: [String]) async throws {
        try await Core.run("edit_library", ["id": id, "name": name, "locations": locations])
    }
    static func deleteLibrary(_ id: String) async throws { try await Core.run("delete_library", ["id": id]) }
    static func cancelScan(_ id: String) async throws { try await Core.run("cancel_scan", ["id": id]) }
    static func libraryPrefs(_ id: String) async throws -> [Pref] { try await Core.call("library_prefs", ["id": id]) }
    static func setLibraryPref(_ id: String, pref: String, value: String) async throws {
        try await Core.run("set_library_pref", ["id": id, "pref": pref, "value": value])
    }
    /// timespan: 6 = hourly buckets, 4 = daily. `since` in unix seconds.
    static func serverStats(timespan: Int, since: Int) async throws -> ServerStats {
        try await Core.call("server_stats", ["timespan": timespan, "since": since])
    }
    /// Saves a zip to the Downloads folder and reveals it; returns the path.
    static func saveDiagnostics(_ what: String) async throws -> String { try await Core.call("save_diagnostics", ["what": what]) }
}
