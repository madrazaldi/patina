import Foundation

// App-level shapes from patina-core (camelCase), matching web/src/lib/types.ts.

struct UserView: Codable, Hashable {
    var uuid: String?
    var name: String
    var email: String?
    var thumb: String?
    var plexPass: Bool
}

struct ServerView: Codable, Hashable, Identifiable {
    var id: String
    var name: String
    var owned: Bool
    var sourceTitle: String?
    var platform: String?
    var version: String?
    var presence: Bool
}

enum Stage: Decodable, Equatable {
    case booting
    case login
    case pickServer(servers: [ServerView], user: UserView)
    case connecting(name: String)
    case ready
    case failed(message: String)

    private enum Keys: String, CodingKey { case kind, servers, user, name, message }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        switch try c.decode(String.self, forKey: .kind) {
        case "login": self = .login
        case "pickServer":
            self = .pickServer(servers: try c.decode([ServerView].self, forKey: .servers), user: try c.decode(UserView.self, forKey: .user))
        case "connecting": self = .connecting(name: try c.decode(String.self, forKey: .name))
        case "ready": self = .ready
        case "failed": self = .failed(message: try c.decode(String.self, forKey: .message))
        default: self = .booting
        }
    }
}

struct SessionView: Decodable, Equatable {
    var accountId: String
    var serverName: String
    var serverId: String
    var machineIdentifier: String?
    var baseUrl: String
    var owned: Bool
    var hasTv: Bool
    var user: UserView?
    var servers: [ServerView]
}

struct NowPlaying: Decodable, Equatable {
    var item: Metadata
    var positionMs: Int
    var durationMs: Int
    var paused: Bool
    var transcoding: Bool
    var queueLen: Int
    var queuePos: Int
    var uid: UInt64
    var ending: Bool
    var stopAfter: Bool
    var live: Bool
    var local: Bool
}

struct QueueItem: Decodable, Equatable, Identifiable {
    var uid: UInt64
    var item: Metadata
    var id: UInt64 { uid }
}

/// The running play queue in play order. Absent when nothing plays (or for offline files).
struct QueueView: Decodable, Equatable {
    var items: [QueueItem]
    var current: UInt64?
    var shuffled: Bool
    var stopAfter: Bool
}

/// A background task on the server (scan, analysis, ...), live from notifications.
struct ServerActivity: Decodable, Equatable, Identifiable {
    var uuid: String
    var kind: String
    var title: String
    var subtitle: String?
    var progress: Int
    var cancellable: Bool
    var sectionId: String?
    var id: String { uuid }
}

struct Snapshot: Decodable {
    var connected: Bool
    var stage: Stage
    var session: SessionView?
    var sections: [Directory]
    var nowPlaying: NowPlaying?
    var queue: QueueView?
    var activities: [ServerActivity]
    var revision: UInt64
}

struct Choice: Decodable, Hashable, Identifiable {
    var id: String
    var label: String
    var color: String?
}

struct Info: Decodable {
    var version: String
    var configPath: String
    var mpvConfigDir: String
    var mpvConfExists: Bool
    var qualities: [Choice]
    var accents: [Choice]
}

// Stored as TOML, so these keep snake_case.
struct PlayerSettings: Codable, Equatable {
    var mpv_path: String
    var extra_args: String
    var quality: String
    var auto_skip_intro: Bool
    var auto_skip_credits: Bool
    var skip_key: String
    var sync_streams: Bool
    var watched_threshold: Double
    var continuous: Bool
    var media_controls: Bool
    var discord: Bool
    var discord_app_id: String
    /// Empty means the default folder.
    var downloads_dir: String
}

struct SavedView: Codable, Equatable, Identifiable {
    struct Picked: Codable, Equatable { var filter: String; var title: String; var value: String; var label: String }
    var id: String
    var name: String
    var scope: String
    var library: String
    var mediaType: Int
    var sort: String
    var desc: Bool
    var unwatched: Bool
    var toggles: [String]
    var picked: [Picked]
}

struct HomeLayout: Codable, Equatable {
    var order: [String] = []
    var hidden: [String] = []
    var shown: [String] = []
    var compact_continue: Bool = false

    /// Hubs hidden until turned on: Plex's legacy On Deck, which Continue Watching replaces.
    static let hiddenByDefault: Set<String> = ["home.ondeck"]

    func isHidden(_ key: String) -> Bool {
        hidden.contains(key) || (Self.hiddenByDefault.contains(key) && !shown.contains(key))
    }
}

struct UiSettings: Codable, Equatable {
    var saved_views: [SavedView]
    var home_layouts: [String: HomeLayout]
    var accent: String
    var poster_size: Int
    var hide_spoilers: Bool

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        // Saved views are free-form JSON on the Rust side; skip any that don't fit.
        let views = (try? c.decode([JSON].self, forKey: "saved_views")) ?? []
        saved_views = views.compactMap { v in
            (try? JSONEncoder.patina.encode(v)).flatMap { try? JSONDecoder.patina.decode(SavedView.self, from: $0) }
        }
        home_layouts = (try? c.decode([String: HomeLayout].self, forKey: "home_layouts")) ?? [:]
        accent = (try? c.decode(String.self, forKey: "accent")) ?? "paper"
        poster_size = (try? c.decode(Int.self, forKey: "poster_size")) ?? 160
        hide_spoilers = (try? c.decode(Bool.self, forKey: "hide_spoilers")) ?? false
    }
}

struct Settings: Decodable, Equatable {
    var player: PlayerSettings
    var ui: UiSettings
}

struct PinView: Decodable {
    var id: Int64
    var code: String
    var url: String
    var attempt: UInt64
}

struct PlaylistView: Decodable {
    var playlist: Metadata
    var items: [Metadata]
}

struct HistoryPage: Decodable {
    var items: [Metadata]
    var hasMore: Bool
    var accounts: [String: String]
}

struct Activity: Decodable, Identifiable {
    var uuid: String?
    var cancellable: Bool
    var title: String?
    var subtitle: String?
    var progress: Double?
    var id: String { uuid ?? title ?? "" }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        uuid = c.lenientString("uuid")
        cancellable = c.lenientBool("cancellable")
        title = c.lenientString("title")
        subtitle = c.lenientString("subtitle")
        progress = c.lenientDouble("progress")
    }
}

struct Tasks: Decodable {
    var activities: [Activity]
    var transcodes: [TranscodeSession]
}

struct Release: Decodable {
    var version: String?
    var added: String?
    var fixed: String?
    var downloadUrl: String?

    private enum CodingKeys: String, CodingKey { case version, added, fixed, downloadUrl = "downloadURL" }
}

struct Overview: Decodable {
    var version: String?
    var platform: String?
    var owner: String?
    var plexPass: Bool
    var machineIdentifier: String?
    var transcoder: Bool
    var connectedVia: String
    var hostCpu: Double
    var plexCpu: Double
    var hostMemory: Double
    var plexMemory: Double
    var release: Release?
    var canInstall: Bool
}

struct Pref: Decodable, Identifiable {
    var id: String
    var label: String
    var summary: String?
    var kind: String
    var value: String
    /// [value, label]
    var options: [[String]]
    var advanced: Bool
}

struct Account: Decodable, Identifiable {
    var accountID: Int?
    var name: String?
    var defaultAudioLanguage: String?
    var defaultSubtitleLanguage: String?
    var id: Int { accountID ?? 0 }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        accountID = c.lenientInt("id")
        name = c.lenientString("name")
        defaultAudioLanguage = c.lenientString("defaultAudioLanguage")
        defaultSubtitleLanguage = c.lenientString("defaultSubtitleLanguage")
    }
}

struct Device: Decodable, Identifiable {
    var deviceID: Int?
    var name: String?
    var platform: String?
    var createdAt: Int?
    var id: Int { deviceID ?? 0 }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        deviceID = c.lenientInt("id")
        name = c.lenientString("name")
        platform = c.lenientString("platform")
        createdAt = c.lenientInt("createdAt")
    }
}

struct ServerDevice: Decodable, Identifiable {
    var name: String?
    var address: String?
    var machineIdentifier: String?
    var product: String?
    var id: String { machineIdentifier ?? name ?? "" }
}

struct Friend: Decodable, Identifiable {
    var friendID: Int?
    var title: String?
    var username: String?
    var email: String?
    var thumb: String?
    var restricted: Bool
    var home: Bool
    var status: String?
    var id: Int { friendID ?? 0 }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        friendID = c.lenientInt("id")
        title = c.lenientString("title")
        username = c.lenientString("username")
        email = c.lenientString("email")
        thumb = c.lenientString("thumb")
        restricted = c.lenientBool("restricted")
        home = c.lenientBool("home")
        status = c.lenientString("status")
    }
}

struct UsersDevices: Decodable {
    var accounts: [Account]
    var devices: [Device]
    var clients: [ServerDevice]
    var friends: [Friend]
}

struct PartChange: Codable, Equatable {
    var partId: Int64
    var audio: Int64?
    var subtitle: Int64?
}

struct TrackView: Decodable {
    struct Season: Decodable, Identifiable { var index: Int; var title: String; var count: Int; var id: Int { index } }
    struct Episode: Decodable, Identifiable { var ratingKey: String; var label: String; var id: String { ratingKey } }
    struct Track: Decodable, Identifiable { var id: Int64; var label: String; var external: Bool }
    struct Plan: Decodable { var changes: [PartChange]; var episodes: Int; var noAudio: [String]; var noSubtitle: [String] }
    var total: Int
    var seasons: [Season]
    var scope: [Episode]
    var reference: String?
    var audio: [Track]
    var subtitles: [Track]
    var audioName: String?
    var subtitleName: String?
    var nothingChosen: Bool
    var plan: Plan
}

struct Lyrics: Decodable {
    struct Line: Decodable { var timeMs: Int?; var text: String }
    var synced: Bool
    var lines: [Line]
}

struct FilterView: Decodable, Hashable, Identifiable {
    /// Query key, e.g. "genre", "year", "unwatched", "hdr".
    var filter: String
    var title: String
    /// "string", "integer", "tag" or "boolean". Boolean filters take "1".
    var kind: String
    var id: String { filter }
}

struct FolderView: Decodable {
    var title: String?
    var directories: [Directory]
    var items: [Metadata]
}

enum DownloadState: String, Codable {
    case queued, downloading, paused, done, failed
}

struct Download: Decodable, Identifiable, Equatable {
    var account: String
    /// "<server id>:<ratingKey>"
    var id: String
    var serverId: String
    var serverName: String
    var item: Metadata
    var file: String?
    var poster: String?
    var size: Int64
    var received: Int64
    var state: DownloadState
    var error: String?
    var addedAt: Int64
}

struct LiveProgram: Decodable, Hashable {
    var title: String
    var subtitle: String?
    var guid: String?
    var summary: String?
    var thumb: String?
    /// Unix seconds.
    var beginsAt: Int64
    var endsAt: Int64
}

struct LiveChannel: Decodable, Identifiable {
    var dvr: String
    var id: String
    var number: String?
    var title: String
    var thumb: String?
    /// From now on, in order; the first may be on air.
    var programs: [LiveProgram]
}

struct Recordings: Decodable {
    struct Upcoming: Decodable, Identifiable { var id: String; var title: String; var subtitle: String?; var beginsAt: Int64?; var channel: String?; var status: String? }
    struct Rule: Decodable, Identifiable { var id: String; var title: String; var kind: String? }
    var upcoming: [Upcoming]
    var rules: [Rule]
}

struct LibraryChoices: Decodable {
    /// [identifier, title]
    var agents: [[String]]
    var scanners: [String]
}

struct LibraryForm: Encodable {
    var name: String
    /// "movie", "show", "artist" or "photo".
    var kind: String
    var agent: String
    var scanner: String
    var language: String
    var locations: [String]
}

struct ServerStats: Decodable {
    struct Bucket: Decodable, Identifiable { var at: Int64; var lan: Double; var wan: Double; var id: Int64 { at } }
    struct Usage: Decodable, Identifiable { var name: String; var bytes: Double; var id: String { name } }
    var bandwidth: [Bucket]
    var accounts: [Usage]
    var devices: [Usage]
    var plays: Int
    var playSeconds: Double
}

struct Toast: Decodable, Identifiable, Equatable {
    var id = UUID()
    var text: String
    var error: Bool

    private enum Keys: String, CodingKey { case text, error }
    init(text: String, error: Bool) { self.text = text; self.error = error }
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        text = try c.decode(String.self, forKey: .text)
        error = try c.decode(Bool.self, forKey: .error)
    }
}
