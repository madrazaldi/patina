// Port of web/src/pages/Settings.svelte: the account and its servers, playback (with the
// automatic audio, subtitle and version choices of PlaybackRulesForm.svelte), appearance,
// downloads, Seerr, diagnostics and About. The mpv and Discord settings have no iOS
// equivalent; the streaming cache stands in for mpv's demuxer cache.

import OSLog
import PatinaCore
import PatinaPlayer
import PlexKit
import SwiftUI

struct SettingsScreen: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        Form {
            AccountSettings()
            PlaybackSettings()
            StreamCacheSettings()
            AutomaticPlaybackSettings()
            AppearanceSettings()
            DownloadSettings()
            SeerrSettingsSection()
            DiagnosticsSettings()
            AboutSettings()
        }
        .navigationTitle("Settings")
    }
}

extension AppStore {
    /// Change some player settings: `patch` holds the changed keys (`["continuous": false]`).
    func updatePlayer(_ patch: [String: JSONValue]) {
        do {
            try Commands.setPlayerSettings(app, player: .object(patch))
        } catch {
            self.error("Could not save settings: \(errorText(error))")
        }
        reloadSettings()
    }
}

// MARK: Account

private struct AccountSettings: View {
    @Environment(AppStore.self) private var store
    @State private var confirmSignOut = false

    var body: some View {
        let session = store.session
        Section("Account") {
            if let user = session?.user {
                HStack(spacing: 14) {
                    Artwork(user.thumb, width: 96, height: 96, fallback: Format.initials(user.name))
                        .frame(width: 48, height: 48)
                        .clipShape(.circle)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(user.name).font(.headline)
                        Text([user.email ?? "", user.plexPass ? "Plex Pass" : ""].filter { !$0.isEmpty }.joined(separator: " · "))
                            .font(.subheadline)
                            .foregroundStyle(Theme.secondaryText)
                    }
                }
                if store.hasTv {
                    Button("Switch User…", systemImage: "person.2") { store.sheet = .switchUser }
                }
            } else {
                LabeledContent("Direct server connection", value: "Not signed in to plex.tv")
            }
            if session?.user != nil {
                Button("Sign Out", role: .destructive) { confirmSignOut = true }
            } else {
                Button("Sign In with Plex") { store.signOut() }
            }
        }
        .confirmationDialog("Sign out of Patina?", isPresented: $confirmSignOut, titleVisibility: .visible) {
            Button("Sign Out", role: .destructive) { store.signOut() }
        }
        if let session {
            Section {
                ForEach(session.servers, id: \.id) { s in
                    Button {
                        Haptics.selection()
                        store.switchServer(s.id)
                    } label: {
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(s.name).foregroundStyle(.primary)
                                Text(serverDetail(s)).font(.caption).foregroundStyle(Theme.secondaryText)
                            }
                            Spacer()
                            if s.id == session.serverId {
                                Image(systemName: "checkmark").foregroundStyle(store.accent)
                            }
                        }
                    }
                }
                if session.servers.isEmpty {
                    LabeledContent(session.serverName, value: session.baseUrl)
                }
            } header: {
                Text("Servers")
            } footer: {
                Text("Connected to \(session.serverName) at \(session.baseUrl).")
            }
        }
    }

    private func serverDetail(_ s: ServerView) -> String {
        var bits: [String] = []
        if s.owned { bits.append("Yours") } else if let from = s.sourceTitle { bits.append("Shared by \(from)") }
        if let v = s.version { bits.append(v) }
        if !s.presence { bits.append("Offline") }
        return bits.joined(separator: " · ")
    }
}

// MARK: Playback

private struct PlaybackSettings: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        if let p = store.settings?.player {
            Section {
                Picker("Streaming Quality", selection: bind("quality", p.quality) { .string($0) }) {
                    ForEach(Config.QUALITIES, id: \.id) { q in Text(q.label).tag(q.id) }
                }
                toggle("Autoplay Next Episode", p.continuous, "continuous")
                toggle("Sync Audio & Subtitle Tracks", p.syncStreams, "sync_streams")
                toggle("Skip Intros Automatically", p.autoSkipIntro, "auto_skip_intro")
                toggle("Skip Credits Automatically", p.autoSkipCredits, "auto_skip_credits")
                Picker("Mark as Watched After", selection: bind("watched_threshold", p.watchedThreshold) { .number($0) }) {
                    ForEach([0.8, 0.85, 0.9, 0.95], id: \.self) { t in Text("\(Int((t * 100).rounded()))%").tag(t) }
                }
                toggle("Lock Screen Controls", p.mediaControls, "media_controls")
            } header: {
                Text("Playback")
            } footer: {
                Text("Original plays the file directly; lower qualities ask the server to transcode, which helps on slow connections. Synced tracks start with what Plex has selected and remember what you switch to.")
            }
        }
    }

    private func toggle(_ title: String, _ value: Bool, _ key: String) -> some View {
        Toggle(title, isOn: bind(key, value) { .bool($0) })
    }

    /// A binding that saves `key` on change.
    private func bind<T>(_ key: String, _ value: T, _ json: @escaping (T) -> JSONValue) -> Binding<T> {
        Binding(get: { value }, set: { store.updatePlayer([key: json($0)]) })
    }
}

/// How much of a stream the player reads ahead and keeps (no desktop twin: mpv's
/// `demuxer-max-bytes` and `demuxer-max-back-bytes` in mpv.conf).
private struct StreamCacheSettings: View {
    @AppStorage(StreamCache.readAheadKey) private var readAhead = 0
    @AppStorage(StreamCache.sizeKey) private var size = 0

    var body: some View {
        Section {
            Picker("Read Ahead", selection: $readAhead) {
                Text("Automatic").tag(0)
                Text("2 Minutes").tag(120)
                Text("5 Minutes").tag(300)
                Text("10 Minutes").tag(600)
                Text("30 Minutes").tag(1800)
                Text("Whole Item").tag(Int.max)
            }
            Picker("Cache Size", selection: $size) {
                Text("Automatic").tag(0)
                ForEach([512, 1024, 2048, 4096, 8192], id: \.self) { mb in
                    Text(Format.bytes(Int64(mb) << 20)).tag(mb << 20)
                }
            }
        } header: {
            Text("Streaming Cache")
        } footer: {
            Text("How far ahead of the playhead the player downloads, and how much of the item it keeps on this \(UIDevice.current.model), for seeking back without downloading again. Automatic reads about 40 seconds ahead and keeps up to 2 GB; the cache never takes more than a quarter of the free space. Applies from the next item, and not to transcoded streams.")
        }
    }
}

/// Port of PlaybackRulesForm.svelte: the profile's automatic choices, and the shows that
/// have their own.
private struct AutomaticPlaybackSettings: View {
    @Environment(AppStore.self) private var store
    @State private var rules = Prefs.Rules()
    @State private var shows: [Prefs.ShowRules] = []
    @State private var error: String?
    @State private var audio = ""
    @State private var subtitleLangs = ""
    @State private var codec = ""

    var body: some View {
        Section {
            if let error {
                Text(error).foregroundStyle(Theme.danger)
            } else {
                LabeledContent("Audio Languages") {
                    TextField("e.g. English, Japanese", text: $audio)
                        .multilineTextAlignment(.trailing)
                        .onSubmit { save { $0.audioLanguages = list(audio) } }
                }
                Picker("Subtitles", selection: field(\.subtitles, .keep)) {
                    Text("As in Plex").tag(Prefs.SubtitleMode.keep)
                    Text("Off").tag(Prefs.SubtitleMode.off)
                    Text("Always").tag(Prefs.SubtitleMode.always)
                    Text("Foreign Dialogue").tag(Prefs.SubtitleMode.foreign)
                }
                LabeledContent("Subtitle Languages") {
                    TextField("Same as audio", text: $subtitleLangs)
                        .multilineTextAlignment(.trailing)
                        .onSubmit { save { $0.subtitleLanguages = list(subtitleLangs) } }
                }
                Picker("Forced Subtitles", selection: field(\.forced, .auto)) {
                    Text("Automatic").tag(Prefs.ForcedMode.auto)
                    Text("Prefer").tag(Prefs.ForcedMode.prefer)
                    Text("Avoid").tag(Prefs.ForcedMode.avoid)
                    Text("Only").tag(Prefs.ForcedMode.only)
                }
                Toggle("Skip Commentary Tracks", isOn: field(\.avoidCommentary, false))
                Picker("Resolution", selection: field(\.resolution, .any)) {
                    ForEach(Prefs.Resolution.allCases, id: \.self) { r in Text(r.label).tag(r) }
                }
                LabeledContent("Video Codec") {
                    TextField("Any", text: $codec)
                        .multilineTextAlignment(.trailing)
                        .textInputAutocapitalization(.never)
                        .onSubmit { save { $0.codec = codec.trimmingCharacters(in: .whitespaces).nonEmpty } }
                }
                Picker("HDR", selection: field(\.hdr, .any)) {
                    Text("Any").tag(Prefs.HdrMode.any)
                    Text("Prefer HDR").tag(Prefs.HdrMode.prefer)
                    Text("Prefer SDR").tag(Prefs.HdrMode.avoid)
                }
                if !rules.isEmpty {
                    Button("Clear All", role: .destructive) { save { $0 = Prefs.Rules() } }
                }
            }
        } header: {
            Text("Automatic Playback")
        } footer: {
            Text("Audio, subtitle and version choices made for you when something starts, for this profile on this server. Shows can override them from their page.")
        }
        .task(id: store.scope) { load() }
        if !shows.isEmpty {
            Section("Shows with Their Own Preferences") {
                ForEach(shows, id: \.ratingKey) { s in
                    Button {
                        store.navigate(to: .item(ratingKey: s.ratingKey))
                    } label: {
                        LabeledContent(s.title, value: "Open")
                    }
                    .swipeActions {
                        Button("Clear", role: .destructive) { clear(s) }
                    }
                }
            }
        }
    }

    private func load() {
        do {
            rules = try Commands.playbackPrefs(store.app).profile
            shows = try Commands.playbackPrefsShows(store.app)
            audio = (rules.audioLanguages ?? []).joined(separator: ", ")
            subtitleLangs = (rules.subtitleLanguages ?? []).joined(separator: ", ")
            codec = rules.codec ?? ""
            error = nil
        } catch {
            self.error = errorText(error)
        }
    }

    private func list(_ text: String) -> [String]? {
        let items = text.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
        return items.isEmpty ? nil : items
    }

    /// A rule as a binding, with the value an unset rule behaves as.
    private func field<T: Equatable>(_ key: WritableKeyPath<Prefs.Rules, T?>, _ unset: T) -> Binding<T> {
        Binding(
            get: { rules[keyPath: key] ?? unset },
            set: { v in save { $0[keyPath: key] = v == unset ? nil : v } })
    }

    private func save(_ change: (inout Prefs.Rules) -> Void) {
        var next = rules
        change(&next)
        do {
            try Commands.setPlaybackPrefs(store.app, rules: next)
            rules = next
        } catch {
            store.error(error)
        }
    }

    private func clear(_ s: Prefs.ShowRules) {
        do {
            try Commands.setPlaybackPrefs(store.app, show: s.ratingKey, title: s.title, rules: Prefs.Rules())
            store.toast("Cleared")
            load()
        } catch {
            store.error(error)
        }
    }
}

// MARK: Appearance

private struct AppearanceSettings: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        if let ui = store.settings?.ui {
            Section("Appearance") {
                VStack(alignment: .leading, spacing: 10) {
                    Text("Accent")
                    HStack(spacing: 12) {
                        ForEach(Config.ACCENTS, id: \.id) { a in
                            let on = ui.accent == a.id
                            Button {
                                Haptics.selection()
                                store.updateUi(["accent": .string(a.id)])
                            } label: {
                                Circle()
                                    .fill(Theme.color(hex: a.color) ?? .gray)
                                    .frame(width: 30, height: 30)
                                    .overlay(Circle().strokeBorder(.white, lineWidth: on ? 2.5 : 0).padding(-4))
                            }
                            .buttonStyle(.plain)
                            .accessibilityLabel(a.label)
                            .accessibilityAddTraits(on ? .isSelected : [])
                        }
                    }
                    .padding(.vertical, 4)
                    .padding(.leading, 4)
                }
                Toggle(
                    "Hide Spoilers",
                    isOn: Binding(get: { ui.hideSpoilers }, set: { store.updateUi(["hide_spoilers": .bool($0)]) }))
                Button("Customize Home…") { store.sheet = .customizeHome }
            }
        }
    }
}

// MARK: Downloads

private struct DownloadSettings: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let smart = Commands.smartDownloads(store.app)
        let used = store.downloads.reduce(0) { $0 + max($1.size, 0) }
        Section {
            LabeledContent("On This Device", value: Format.bytes(Int64(used)))
            LabeledContent("Storage Limit", value: smart.budget.map { Format.bytes(Int64($0)) } ?? "No limit")
            LabeledContent("Smart Downloads", value: smart.policies.isEmpty ? "None" : "\(smart.policies.count)")
            Button("Manage Downloads") { store.navigate(to: .downloads) }
            Button("Show in Files") {
                store.perform { try await Commands.openDownloadsFolder(store.app, store.host) }
            }
        } header: {
            Text("Downloads")
        } footer: {
            Text("Downloads are saved in On My \(UIDevice.current.model) › Patina › Downloads, which the Files app shows, and keep transferring in the background.")
        }
    }
}

// MARK: Seerr

private struct SeerrSettingsSection: View {
    @Environment(AppStore.self) private var store
    @State private var status: SeerrStatus?
    @State private var error: String?
    @State private var url = ""
    @State private var key = ""
    @State private var connecting = false

    var body: some View {
        let config = Commands.seerrConfig(store.app)
        Section {
            if let config {
                if let status {
                    LabeledContent("Connected as \(status.user.name)") {
                        Text([status.version.map { "Seerr \($0)" }, status.auth == .plex ? "Plex sign-in" : "API key"].compactMap { $0 }.joined(separator: " · "))
                    }
                } else if let error {
                    LabeledContent("Not connected") { Text(error).foregroundStyle(Theme.danger) }
                } else {
                    LabeledContent(config.url) { ProgressView() }
                }
            }
            TextField("Server address, e.g. http://seerr.local:5055", text: $url)
                .keyboardType(.URL)
                .textContentType(.URL)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
            SecureField(config?.auth == .apiKey ? "API key (saved; enter to replace)" : "API key (optional)", text: $key)
            HStack {
                Button(connecting ? "Connecting…" : config != nil ? "Reconnect" : "Connect") { connect(config) }
                    .disabled(connecting || url.trimmingCharacters(in: .whitespaces).isEmpty)
                if config != nil {
                    Spacer()
                    Button("Disconnect", role: .destructive) { disconnect() }
                }
            }
            .buttonStyle(.borderless)
            if config != nil, status != nil {
                Button("Open Requests") { store.navigate(to: .requests) }
            }
        } header: {
            Text("Seerr")
        } footer: {
            Text("Request what the server doesn't have through Seerr, Overseerr or Jellyseerr. Leave the API key empty to sign in with your Plex account.")
        }
        .task(id: "\(config?.url ?? "")|\(store.scope)") {
            status = nil
            error = nil
            guard let config else { return }
            url = config.url
            do {
                status = try await Commands.seerrStatus(store.app)
            } catch {
                self.error = errorText(error)
            }
        }
    }

    private func connect(_ config: SeerrConfig?) {
        connecting = true
        error = nil
        Task {
            do {
                // An empty field keeps a saved key; disconnect first to switch to the Plex sign-in.
                let apiKey: String? = key.trimmingCharacters(in: .whitespaces).isEmpty && config?.auth == .apiKey ? nil : key
                let s = try await Commands.seerrConnect(store.app, url: url, apiKey: apiKey)
                key = ""
                status = s
                store.reloadSettings()
                store.toast("Connected to Seerr as \(s.user.name)")
            } catch {
                self.error = errorText(error)
                store.error(error)
            }
            connecting = false
        }
    }

    private func disconnect() {
        do {
            try Commands.seerrDisconnect(store.app)
            url = ""
            status = nil
            store.reloadSettings()
            store.toast("Seerr disconnected")
        } catch {
            store.error(error)
        }
    }
}

// MARK: Diagnostics

private struct DiagnosticsSettings: View {
    @Environment(AppStore.self) private var store
    @State private var export: URL?
    @State private var collecting = false

    var body: some View {
        Section {
            LabeledContent("Connection", value: store.connected ? "Online" : "Reconnecting…")
            if let s = store.session {
                LabeledContent("Server", value: s.baseUrl)
                    .textSelection(.enabled)
            }
            LabeledContent("Player", value: store.engine != nil ? "AetherEngine" : "Unavailable")
            LabeledContent("Log Level", value: ProcessInfo.processInfo.environment["PATINA_LOG"] ?? "info")
            if let export {
                ShareLink("Share Logs", item: export)
            } else {
                Button(collecting ? "Collecting Logs…" : "Export Logs") { collect() }
                    .disabled(collecting)
            }
        } header: {
            Text("Diagnostics")
        } footer: {
            Text("The logs of this session, to attach to a bug report. They name your servers but hold no tokens.")
        }
    }

    /// The app's unified-log entries since launch, as a text file.
    private func collect() {
        collecting = true
        Task {
            export = await Self.writeLogs()
            if export == nil { store.error("Could not read the logs.") }
            collecting = false
        }
    }

    @concurrent
    private nonisolated static func writeLogs() async -> URL? {
        do {
            let store = try OSLogStore(scope: .currentProcessIdentifier)
            let entries = try store.getEntries(
                at: store.position(timeIntervalSinceLatestBoot: 0),
                matching: NSPredicate(format: "subsystem == %@", "io.github.madrazaldi.patina"))
            let format = ISO8601DateFormatter()
            var text = "Patina \(Config.VERSION) logs\n\n"
            for case let e as OSLogEntryLog in entries {
                text += "\(format.string(from: e.date)) [\(e.category)] \(e.composedMessage)\n"
            }
            let url = FileManager.default.temporaryDirectory.appendingPathComponent("Patina Logs.txt")
            try text.write(to: url, atomically: true, encoding: .utf8)
            return url
        } catch {
            return nil
        }
    }
}

// MARK: About

private struct AboutSettings: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let info = Commands.info(store.app)
        Section("About") {
            LabeledContent("Version", value: "Patina \(info.version)")
            LabeledContent("Settings File") {
                Text(info.configPath).font(.caption).textSelection(.enabled).lineLimit(2).truncationMode(.middle)
            }
            Link("Source Code", destination: URL(string: "https://github.com/madrazaldi/patina")!)
            Text("Plays with AetherEngine (FFmpeg over AVPlayer and VideoToolbox). Patina is not affiliated with Plex.")
                .font(.caption)
                .foregroundStyle(Theme.secondaryText)
        }
    }
}
