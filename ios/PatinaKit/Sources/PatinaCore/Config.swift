// Port of desktop/core/config.ts.
// Persistent settings, stored as JSON in `config.json` in the config dir.
//
// The keys are those of the desktop's `config.toml` (the serde field names, snake_case), so
// the document has the same shape; the Swift properties are their camelCase. Tokens
// (`auth.token`, `server.manual_token`, `seerr.api_key`) live in the `SecretStore` (the
// Keychain) and never in the file. Keys this version does not know, in known sections or
// as sections of their own, survive a rewrite, as on the desktop. There is one process, so
// there is no lock; the file is still re-read before each change, which is cheap.

import Foundation
import PlexKit

nonisolated private let log = logger("config")

public nonisolated enum Config {
    public nonisolated static let PRODUCT = "Patina"
    /// Workspace version of the Rust crates (`[workspace.package] version`); tools/release.sh bumps both.
    public nonisolated static let VERSION = "0.2.0"
}

/// A settings struct as read from and written to its JSON section: `init(json:)` reads it
/// strictly, like serde (a missing key takes the default, a wrongly typed one fails the
/// whole struct); `json` writes it back with every key (nil as null).
public nonisolated protocol SettingsSection: Sendable, Equatable {
    init()
    init(json: JSONValue) throws
    var json: JSONValue { get }
}

public nonisolated struct AuthSettings: SettingsSection {
    public var accountId: String? = nil
    /// plex.tv account token (or the active Plex Home user's token). Kept in the Keychain.
    public var token: String? = nil

    public init() {}

    public init(json v: JSONValue) throws {
        let t = try Read.table(v, "auth")
        accountId = try Read.field(t, "account_id", nil, Read.optStr)
        token = try Read.field(t, "token", nil, Read.optStr)
    }

    public var json: JSONValue {
        ["account_id": JSONValue(accountId), "token": JSONValue(token)]
    }
}

public nonisolated struct ServerSettings: SettingsSection {
    /// `clientIdentifier` of the preferred server.
    public var id: String? = nil
    /// Last connection URI that worked, tried first on startup.
    public var lastUri: String? = nil
    /// Manually configured server (no plex.tv account).
    public var manualUrl: String? = nil
    /// Kept in the Keychain.
    public var manualToken: String? = nil

    public init() {}

    public init(json v: JSONValue) throws {
        let t = try Read.table(v, "server")
        id = try Read.field(t, "id", nil, Read.optStr)
        lastUri = try Read.field(t, "last_uri", nil, Read.optStr)
        manualUrl = try Read.field(t, "manual_url", nil, Read.optStr)
        manualToken = try Read.field(t, "manual_token", nil, Read.optStr)
    }

    public var json: JSONValue {
        [
            "id": JSONValue(id), "last_uri": JSONValue(lastUri), "manual_url": JSONValue(manualUrl),
            "manual_token": JSONValue(manualToken),
        ]
    }
}

public nonisolated struct PlayerSettings: SettingsSection {
    /// mpv executable; auto-detected when empty. Unused on iOS.
    public var mpvPath = ""
    /// Extra mpv arguments, whitespace separated. Unused on iOS.
    public var extraArgs = ""
    /// Quality preset id (see `QUALITIES`).
    public var quality = "original"
    public var autoSkipIntro = false
    public var autoSkipCredits = false
    /// mpv key that skips the current intro/credits marker. Unused on iOS.
    public var skipKey = "TAB"
    /// Apply the audio/subtitle tracks selected in Plex, and save changes made in the player back to Plex.
    public var syncStreams = true
    /// Fraction of the runtime after which an item counts as watched.
    public var watchedThreshold = 0.9
    /// Autoplay following episodes.
    public var continuous = true
    /// Show what's playing in the OS media controls (Now Playing).
    public var mediaControls = true
    /// Discord Rich Presence. Unused on iOS.
    public var discord = false
    /// Discord application id. Unused on iOS.
    public var discordAppId = ""
    /// Where downloads go on the desktop. Unused on iOS: downloads go to `Paths.downloads`.
    public var downloadsDir = ""

    public init() {}

    public init(json v: JSONValue) throws {
        let t = try Read.table(v, "player")
        let d = PlayerSettings()
        mpvPath = try Read.field(t, "mpv_path", d.mpvPath, Read.str)
        extraArgs = try Read.field(t, "extra_args", d.extraArgs, Read.str)
        quality = try Read.field(t, "quality", d.quality, Read.str)
        autoSkipIntro = try Read.field(t, "auto_skip_intro", d.autoSkipIntro, Read.bool)
        autoSkipCredits = try Read.field(t, "auto_skip_credits", d.autoSkipCredits, Read.bool)
        skipKey = try Read.field(t, "skip_key", d.skipKey, Read.str)
        syncStreams = try Read.field(t, "sync_streams", d.syncStreams, Read.bool)
        watchedThreshold = try Read.field(t, "watched_threshold", d.watchedThreshold, Read.num)
        continuous = try Read.field(t, "continuous", d.continuous, Read.bool)
        mediaControls = try Read.field(t, "media_controls", d.mediaControls, Read.bool)
        discord = try Read.field(t, "discord", d.discord, Read.bool)
        discordAppId = try Read.field(t, "discord_app_id", d.discordAppId, Read.str)
        downloadsDir = try Read.field(t, "downloads_dir", d.downloadsDir, Read.str)
    }

    public var json: JSONValue {
        [
            "mpv_path": .string(mpvPath), "extra_args": .string(extraArgs), "quality": .string(quality),
            "auto_skip_intro": .bool(autoSkipIntro), "auto_skip_credits": .bool(autoSkipCredits),
            "skip_key": .string(skipKey), "sync_streams": .bool(syncStreams),
            "watched_threshold": .number(watchedThreshold), "continuous": .bool(continuous),
            "media_controls": .bool(mediaControls), "discord": .bool(discord), "discord_app_id": .string(discordAppId),
            "downloads_dir": .string(downloadsDir),
        ]
    }

    /// `PlayerSettings::quality()`: the selected preset, original when unknown.
    public var qualityPreset: Quality {
        Config.quality(quality) ?? Config.QUALITIES[0]
    }
}

public nonisolated struct HomeLayout: SettingsSection {
    public var order: [String] = []
    public var hidden: [String] = []
    /// Hubs that are hidden by default (the legacy On Deck) but turned on.
    public var shown: [String] = []
    public var compactContinue = false

    public init() {}

    public init(order: [String] = [], hidden: [String] = [], shown: [String] = [], compactContinue: Bool = false) {
        self.order = order
        self.hidden = hidden
        self.shown = shown
        self.compactContinue = compactContinue
    }

    public init(json v: JSONValue) throws {
        let t = try Read.table(v, "home_layouts")
        order = try Read.field(t, "order", [], Read.strings)
        hidden = try Read.field(t, "hidden", [], Read.strings)
        shown = try Read.field(t, "shown", [], Read.strings)
        compactContinue = try Read.field(t, "compact_continue", false, Read.bool)
    }

    public var json: JSONValue {
        [
            "order": .array(order.map { .string($0) }), "hidden": .array(hidden.map { .string($0) }),
            "shown": .array(shown.map { .string($0) }), "compact_continue": .bool(compactContinue),
        ]
    }
}

public nonisolated struct UiSettings: SettingsSection {
    /// Named library queries and home layouts, scoped by account and server in the GUI.
    public var savedViews: [JSONValue] = []
    public var homeLayouts: [String: HomeLayout] = [:]
    public var accent = "paper"
    /// Poster width in points.
    public var posterSize = 160
    /// Blur thumbnails and summaries of unwatched episodes.
    public var hideSpoilers = false

    public init() {}

    public init(json v: JSONValue) throws {
        let t = try Read.table(v, "ui")
        let d = UiSettings()
        savedViews = try Read.field(t, "saved_views", d.savedViews) { v, name in
            guard case .array(let a) = v else { throw Read.invalid("invalid type for `\(name)`: expected a sequence") }
            return a
        }
        homeLayouts = try Read.field(t, "home_layouts", d.homeLayouts) { v, name in
            try Read.table(v, name).mapValues { try HomeLayout(json: $0) }
        }
        accent = try Read.field(t, "accent", d.accent, Read.str)
        posterSize = try Read.field(t, "poster_size", d.posterSize, Read.uint)
        hideSpoilers = try Read.field(t, "hide_spoilers", d.hideSpoilers, Read.bool)
    }

    public var json: JSONValue {
        [
            "saved_views": .array(savedViews), "home_layouts": .object(homeLayouts.mapValues(\.json)),
            "accent": .string(accent), "poster_size": .number(Double(posterSize)), "hide_spoilers": .bool(hideSpoilers),
        ]
    }
}

/// The terminal client's appearance; kept so the format stays the same.
public nonisolated struct TuiSettings: SettingsSection {
    public var showInspector = true
    public var showWatchStatus = true
    public var showLibraryCounts = true
    public var hideWatched = false
    /// Accent colour; empty means the theme's.
    public var accent = ""

    public init() {}

    public init(json v: JSONValue) throws {
        let t = try Read.table(v, "tui")
        let d = TuiSettings()
        showInspector = try Read.field(t, "show_inspector", d.showInspector, Read.bool)
        showWatchStatus = try Read.field(t, "show_watch_status", d.showWatchStatus, Read.bool)
        showLibraryCounts = try Read.field(t, "show_library_counts", d.showLibraryCounts, Read.bool)
        hideWatched = try Read.field(t, "hide_watched", d.hideWatched, Read.bool)
        accent = try Read.field(t, "accent", d.accent, Read.str)
    }

    public var json: JSONValue {
        [
            "show_inspector": .bool(showInspector), "show_watch_status": .bool(showWatchStatus),
            "show_library_counts": .bool(showLibraryCounts), "hide_watched": .bool(hideWatched),
            "accent": .string(accent),
        ]
    }
}

/// A Seerr (Overseerr/Jellyseerr) instance for requesting titles the server doesn't have.
public nonisolated struct SeerrSettings: SettingsSection {
    /// Base URL such as `http://seerr.local:5055`; empty when not set up.
    public var url = ""
    /// API key (Settings → General in Seerr); empty signs in with the Plex account instead.
    /// Kept in the Keychain.
    public var apiKey = ""

    public init() {}

    public init(url: String, apiKey: String) {
        self.url = url
        self.apiKey = apiKey
    }

    public init(json v: JSONValue) throws {
        let t = try Read.table(v, "seerr")
        url = try Read.field(t, "url", "", Read.str)
        apiKey = try Read.field(t, "api_key", "", Read.str)
    }

    public var json: JSONValue {
        ["url": .string(url), "api_key": .string(apiKey)]
    }
}

public nonisolated struct Settings: SettingsSection {
    /// Stable per-install identifier sent as `X-Plex-Client-Identifier`.
    public var clientId: String
    public var auth = AuthSettings()
    public var server = ServerSettings()
    public var player = PlayerSettings()
    /// The GUI's appearance.
    public var ui = UiSettings()
    /// The terminal client's appearance and behaviour (not used on iOS).
    public var tui = TuiSettings()
    public var seerr = SeerrSettings()

    /// `Settings::default()`: a fresh client id.
    public init() {
        clientId = UUID().uuidString.lowercased()
    }

    public init(json v: JSONValue) throws {
        let t = try Read.table(v, "settings")
        self.init()
        clientId = try Read.field(t, "client_id", clientId, Read.str)
        if let v = t["auth"] { auth = try AuthSettings(json: v) }
        if let v = t["server"] { server = try ServerSettings(json: v) }
        if let v = t["player"] { player = try PlayerSettings(json: v) }
        if let v = t["ui"] { ui = try UiSettings(json: v) }
        if let v = t["tui"] { tui = try TuiSettings(json: v) }
        if let v = t["seerr"] { seerr = try SeerrSettings(json: v) }
    }

    public var json: JSONValue {
        [
            "client_id": .string(clientId), "auth": auth.json, "server": server.json, "player": player.json,
            "ui": ui.json, "tui": tui.json, "seerr": seerr.json,
        ]
    }
}

public nonisolated struct Accent: Sendable, Hashable {
    public var id: String
    public var label: String
    public var color: String
}

public nonisolated struct Quality: Sendable, Hashable {
    public var id: String
    public var label: String
    /// `nil` means direct play.
    public var kbps: Int?
    public var resolution: String
}

extension Config {
    /// Accents are used sparingly (progress, watched marks, focus), so they are
    /// kept muted. Ids are stable; they are stored in the config file.
    public nonisolated static let ACCENTS: [Accent] = [
        Accent(id: "paper", label: "Paper", color: "#e6e1d6"),
        Accent(id: "amber", label: "Amber", color: "#d6a44c"),
        Accent(id: "copper", label: "Copper", color: "#c98a5e"),
        Accent(id: "verdigris", label: "Verdigris", color: "#7fb3a0"),
        Accent(id: "sky", label: "Sky", color: "#7ea6cc"),
        Accent(id: "iris", label: "Iris", color: "#9d98d4"),
        Accent(id: "rose", label: "Rose", color: "#cf8a98"),
    ]

    public nonisolated static let QUALITIES: [Quality] = [
        Quality(id: "original", label: "Original (direct play)", kbps: nil, resolution: ""),
        Quality(id: "40m-4k", label: "4K · 40 Mbps", kbps: 40000, resolution: "3840x2160"),
        Quality(id: "20m-1080", label: "1080p · 20 Mbps", kbps: 20000, resolution: "1920x1080"),
        Quality(id: "12m-1080", label: "1080p · 12 Mbps", kbps: 12000, resolution: "1920x1080"),
        Quality(id: "8m-1080", label: "1080p · 8 Mbps", kbps: 8000, resolution: "1920x1080"),
        Quality(id: "4m-720", label: "720p · 4 Mbps", kbps: 4000, resolution: "1280x720"),
        Quality(id: "2m-720", label: "720p · 2 Mbps", kbps: 2000, resolution: "1280x720"),
        Quality(id: "1m-480", label: "480p · 1 Mbps", kbps: 1000, resolution: "854x480"),
    ]

    /// A quality preset by id or label.
    public nonisolated static func quality(_ name: String) -> Quality? {
        QUALITIES.first { $0.id == name || $0.label == name }
    }

    /// `playerQuality(p)`.
    public nonisolated static func playerQuality(_ p: PlayerSettings) -> Quality {
        p.qualityPreset
    }

    // MARK: Reading and writing

    /// The settings keys whose values are secrets: (section, key), stored as "section.key".
    nonisolated static let SECRETS: [(String, String)] = [("auth", "token"), ("server", "manual_token"), ("seerr", "api_key")]

    public nonisolated static func configFile(_ paths: Paths) -> URL {
        paths.config.appendingPathComponent("config.json")
    }

    /// Parse `config.json` text into settings (secrets aside). Throws if unreadable.
    public nonisolated static func parseSettings(_ text: String) throws -> Settings {
        let doc = text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? JSONValue.object([:]) : try JSONValue.parse(text)
        return try Settings(json: doc)
    }

    private nonisolated struct Loaded {
        var settings: Settings
        var clientIdSaved: Bool
    }

    /// The file and the secrets, or `nil` when the file is missing or unreadable.
    private nonisolated static func read(_ env: AppEnvironment) -> Loaded? {
        let path = configFile(env.paths)
        guard let bytes = try? Data(contentsOf: path) else { return nil }
        let doc: JSONValue
        do {
            doc = try JSONValue.parse(bytes)
        } catch {
            log.warn("ignoring unreadable \(path.path): \(decodeText(error))")
            return nil
        }
        do {
            var settings = try Settings(json: doc)
            withSecrets(&settings, env.secrets)
            return Loaded(settings: settings, clientIdSaved: doc["client_id"] != nil)
        } catch {
            log.warn("ignoring unreadable \(path.path): \(errorText(error))")
            return nil
        }
    }

    private nonisolated static func withSecrets(_ s: inout Settings, _ secrets: any SecretStore) {
        s.auth.token = secrets.get("auth.token")
        s.server.manualToken = secrets.get("server.manual_token")
        s.seerr.apiKey = secrets.get("seerr.api_key") ?? ""
    }

    public nonisolated static func loadSettings(_ env: AppEnvironment) -> Settings {
        let exists = FileManager.default.fileExists(atPath: configFile(env.paths).path)
        let loaded = read(env)
        let settings = loaded?.settings ?? Settings()
        // Persist so the generated client id stays stable. Saving fresh defaults also clears
        // tokens a previous install left in the Keychain (it outlives the app's files).
        if loaded.map({ !$0.clientIdSaved }) ?? !exists {
            do {
                try saveSettings(env, settings)
            } catch {
                log.error("could not save settings: \(errorText(error))")
            }
        }
        return settings
    }

    /// The file as saved now, keeping this client id if the file has none. `current` when it
    /// can't be read.
    public nonisolated static func reloadSettings(_ env: AppEnvironment, _ current: Settings) -> Settings {
        guard var loaded = read(env) else { return current }
        if !loaded.clientIdSaved { loaded.settings.clientId = current.clientId }
        return loaded.settings
    }

    /// The settings as a JSON document without the secrets, merged over `existing` so
    /// unknown keys and sections survive.
    public nonisolated static func settingsToJson(_ s: Settings, _ existing: JSONValue? = nil) -> JSONValue {
        guard case .object(var doc) = s.json else { return s.json }
        let old = existing?.objectValue ?? [:]
        for (name, value) in doc {
            guard case .object(var section) = value else { continue }
            for (section2, key) in SECRETS where section2 == name { section.removeValue(forKey: key) }
            // `None` fields are absent from the file.
            if name == "auth" || name == "server" { section = section.filter { !$0.value.isNull } }
            // Keep keys this version does not know inside known sections.
            if case .object(let before)? = old[name] {
                let known = s.json[name]?.objectValue ?? [:]
                for (k, v) in before where known[k] == nil && section[k] == nil { section[k] = v }
            }
            doc[name] = .object(section)
        }
        for (k, v) in old where doc[k] == nil { doc[k] = v }
        return .object(doc)
    }

    /// Save the secrets to the secret store and the rest to `config.json`.
    public nonisolated static func saveSettings(_ env: AppEnvironment, _ s: Settings) throws {
        let secrets: [(String, String?)] = [
            ("auth.token", s.auth.token), ("server.manual_token", s.server.manualToken),
            ("seerr.api_key", s.seerr.apiKey.isEmpty ? nil : s.seerr.apiKey),
        ]
        for (key, value) in secrets where env.secrets.get(key) != value { try env.secrets.set(key, value) }
        let path = configFile(env.paths)
        let existing = (try? Data(contentsOf: path)).flatMap { try? JSONValue.parse($0) }
        try Storage.write(path, try Storage.pretty(settingsToJson(s, existing)))
    }

    /// The read-modify-write transaction of a settings change: the file is re-read first.
    public nonisolated static func updateSettings(_ env: AppEnvironment, _ current: Settings, _ f: (inout Settings) throws -> Void)
        throws -> Settings
    {
        // Never overwrite an existing unreadable file with defaults.
        var latest: Settings
        if FileManager.default.fileExists(atPath: configFile(env.paths).path) {
            guard let loaded = read(env) else {
                throw CoreError("Settings could not be read; the existing file was preserved.")
            }
            latest = loaded.settings
        } else {
            latest = current
        }
        try f(&latest)
        try saveSettings(env, latest)
        return latest
    }

    /// Apply only the fields a front end changed, preserving the others. Throws (leaving
    /// `value` unchanged) on an unknown key or a value of the wrong type.
    public nonisolated static func patch<T: SettingsSection>(_ value: inout T, _ patch: JSONValue) throws {
        guard case .object(let changes) = patch else { throw CoreError("Settings patch must be an object") }
        guard case .object(var json) = value.json else { return }
        for (key, v) in changes {
            guard json.keys.contains(key) else { throw CoreError("Unknown setting: \(key)") }
            json[key] = v
        }
        value = try T(json: .object(json))
    }
}

/// The strict readers of `init(json:)`: the wrong type fails the whole struct, like serde.
nonisolated enum Read {
    nonisolated static func invalid(_ message: String) -> CoreError {
        CoreError(message)
    }

    nonisolated static func table(_ v: JSONValue, _ name: String) throws -> [String: JSONValue] {
        guard case .object(let o) = v else { throw invalid("invalid type for `\(name)`: expected a table") }
        return o
    }

    /// A missing key is the default (`#[serde(default)]`).
    nonisolated static func field<T>(_ t: [String: JSONValue], _ key: String, _ def: T, _ read: (JSONValue, String) throws -> T)
        throws -> T
    {
        guard let v = t[key] else { return def }
        return try read(v, key)
    }

    nonisolated static func str(_ v: JSONValue, _ name: String) throws -> String {
        guard case .string(let s) = v else { throw invalid("invalid type for `\(name)`: expected a string") }
        return s
    }

    /// JSON can say null where TOML leaves the key out.
    nonisolated static func optStr(_ v: JSONValue, _ name: String) throws -> String? {
        v.isNull ? nil : try str(v, name)
    }

    nonisolated static func bool(_ v: JSONValue, _ name: String) throws -> Bool {
        guard case .bool(let b) = v else { throw invalid("invalid type for `\(name)`: expected a boolean") }
        return b
    }

    nonisolated static func num(_ v: JSONValue, _ name: String) throws -> Double {
        guard case .number(let n) = v else { throw invalid("invalid type for `\(name)`: expected a number") }
        return n
    }

    nonisolated static func uint(_ v: JSONValue, _ name: String) throws -> Int {
        let n = try num(v, name)
        guard n.rounded() == n, n >= 0, n <= 0xffff_ffff else {
            throw invalid("invalid value for `\(name)`: expected a u32")
        }
        return Int(n)
    }

    nonisolated static func strings(_ v: JSONValue, _ name: String) throws -> [String] {
        guard case .array(let a) = v else { throw invalid("invalid type for `\(name)`: expected a sequence") }
        return try a.map { try str($0, name) }
    }
}
