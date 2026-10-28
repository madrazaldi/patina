// Port of desktop/core/ops/server.ts.
// Server administration: activity, overview, settings, accounts, libraries, statistics and
// diagnostics.

import Foundation
import PlexKit

extension Ops {
    public enum Server {
        public nonisolated struct Tasks: Sendable, Equatable {
            public var activities: [Activity]
            public var transcodes: [TranscodeSession]
        }

        /// Background activities and transcoder sessions.
        public static func tasks(_ app: AppState) async throws -> Tasks {
            let server = try app.server()
            async let activities = (try? server.activities()) ?? []
            async let transcodes = (try? server.transcodeSessions()) ?? []
            return Tasks(activities: await activities, transcodes: await transcodes)
        }

        /// Stop a transcoder session by its key.
        public static func killTranscode(_ app: AppState, _ key: String) async throws {
            try await app.server().killTranscodeSession(Lists.lastSegment(key))
        }

        public nonisolated struct Overview: Sendable, Equatable {
            public var version: String?
            public var platform: String?
            public var owner: String?
            public var plexPass: Bool
            public var machineIdentifier: String?
            public var transcoder: Bool
            public var connectedVia: String
            public var hostCpu: Double
            public var plexCpu: Double
            public var hostMemory: Double
            public var plexMemory: Double
            public var release: Release?
            public var canInstall: Bool
        }

        public static func serverOverview(_ app: AppState) async throws -> Overview {
            let server = try app.server()
            async let update = try? server.updateStatus()
            async let resources = (try? server.statisticsResources(6)) ?? []
            let caps = try await server.capabilities()
            let latest = await resources.last
            let attr = { (k: String) in MediaContainer.attr(caps, k) }
            let flag = { (k: String) in attr(k) == "true" || attr(k) == "1" }
            let p = attr("platform")
            let v = attr("platformVersion")
            let platform = p.flatMap { p in v.map { "\(p) \($0)" } ?? p }
            let status = await update
            return Overview(
                version: attr("version"), platform: platform, owner: attr("myPlexUsername"),
                plexPass: flag("myPlexSubscription"), machineIdentifier: caps.machineIdentifier,
                transcoder: flag("transcoderVideo"), connectedVia: server.baseUrl.absoluteString,
                hostCpu: latest?.hostCpuUtilization ?? 0, plexCpu: latest?.processCpuUtilization ?? 0,
                hostMemory: latest?.hostMemoryUtilization ?? 0, plexMemory: latest?.processMemoryUtilization ?? 0,
                release: status?.release.first, canInstall: status?.canInstall ?? false)
        }

        /// A server or library setting.
        public nonisolated struct Pref: Sendable, Equatable {
            public struct Option: Sendable, Equatable {
                public var value: String
                public var label: String
            }

            public var id: String
            public var label: String
            public var summary: String?
            public var kind: String
            public var value: String
            public var options: [Option]
            public var advanced: Bool
            /// Plex's group id (`general`, `network`, `butler`, ...); empty for library settings.
            public var group: String

            public init(
                id: String, label: String, summary: String?, kind: String, value: String, options: [Option],
                advanced: Bool, group: String
            ) {
                self.id = id
                self.label = label
                self.summary = summary
                self.kind = kind
                self.value = value
                self.options = options
                self.advanced = advanced
                self.group = group
            }
        }

        /// A scheduled-task setting: the maintenance window or a task's switch.
        public nonisolated static func isButler(_ p: Pref) -> Bool {
            p.group == "butler" || p.id.hasPrefix("Butler")
        }

        /// `Pref::from(Setting)`.
        public nonisolated static func prefFrom(_ s: Setting) -> Pref {
            Pref(
                id: s.id ?? "", label: s.label ?? s.id ?? "", summary: s.summary, kind: s.type ?? "",
                value: Setting.valueString(s), options: Setting.enumOptions(s).map { Pref.Option(value: $0.0, label: $0.1) },
                advanced: s.advanced, group: s.group ?? "")
        }

        private nonisolated static func visible(_ list: [Setting]) -> [Pref] {
            list.filter { !$0.hidden }.map(prefFrom)
        }

        /// Server settings that belong together, as Plex groups them.
        public nonisolated struct PrefGroup: Sendable, Equatable {
            public var id: String
            public var label: String
            public var prefs: [Pref]
        }

        /// How a Plex settings group is titled.
        public nonisolated static func groupLabel(_ id: String) -> String {
            switch id {
            case "": return "Other"
            case "dlna": return "DLNA"
            case "butler": return "Scheduled tasks"
            default:
                let words = id.replacing(#/[_-]/#, with: " ")
                guard let first = words.first else { return "" }
                return first.uppercased() + words.dropFirst()
            }
        }

        /// Group settings in the order Plex lists them, ungrouped ones last.
        public nonisolated static func grouped(_ prefs: [Pref]) -> [PrefGroup] {
            var groups: [PrefGroup] = []
            for p in prefs {
                if let at = groups.firstIndex(where: { $0.id == p.group }) {
                    groups[at].prefs.append(p)
                } else {
                    groups.append(PrefGroup(id: p.group, label: groupLabel(p.group), prefs: [p]))
                }
            }
            // Stable: ungrouped (empty id) ones move last.
            return groups.filter { $0.id != "" } + groups.filter { $0.id == "" }
        }

        /// The server's settings by group, without hidden ones. Scheduled-task settings are
        /// left out; `maintenance` has them next to their tasks.
        public static func settings(_ app: AppState) async throws -> [PrefGroup] {
            let prefs = visible(try await app.server().prefs())
            return grouped(prefs.filter { !isButler($0) })
        }

        /// A butler task with the setting that turns its scheduled runs on and off.
        @dynamicMemberLookup
        public nonisolated struct ScheduledTask: Sendable, Equatable {
            public var task: ButlerTask
            public var pref: Pref?

            public subscript<T>(dynamicMember key: KeyPath<ButlerTask, T>) -> T {
                task[keyPath: key]
            }
        }

        public nonisolated struct Maintenance: Sendable, Equatable {
            /// Butler settings that aren't one task's switch, like the maintenance window.
            public var schedule: [Pref]
            public var tasks: [ScheduledTask]
        }

        /// The butler's tasks joined with their settings.
        public static func maintenance(_ app: AppState) async throws -> Maintenance {
            let server = try app.server()
            async let prefs = (try? server.prefs()) ?? []
            let tasks = try await server.butlerTasks()
            return joinButler(tasks, visible(await prefs))
        }

        /// Plex names a task's switch `ButlerTask<name>`.
        public nonisolated static func joinButler(_ tasks: [ButlerTask], _ prefs: [Pref]) -> Maintenance {
            var schedule = prefs.filter(isButler)
            let out = tasks.map { task -> ScheduledTask in
                let id = "ButlerTask\(task.name ?? "")"
                let at = schedule.firstIndex { $0.id == id && $0.kind == "bool" }
                return ScheduledTask(task: task, pref: at.map { schedule.remove(at: $0) })
            }
            return Maintenance(schedule: schedule, tasks: out)
        }

        /// A library's settings, without hidden ones.
        public static func libraryPrefs(_ app: AppState, _ id: String) async throws -> [Pref] {
            visible(try await app.server().sectionPrefs(id))
        }

        public nonisolated struct UsersDevices: Sendable, Equatable {
            public var accounts: [Account]
            public var devices: [Device]
            public var clients: [ServerDevice]
            public var friends: [Friend]
        }

        public static func usersDevices(_ app: AppState) async throws -> UsersDevices {
            let server = try app.server()
            let tv = app.tv()
            async let accounts = (try? server.accounts()) ?? []
            async let devices = (try? server.devices()) ?? []
            async let clients = (try? server.clients()) ?? []
            async let friends = tv == nil ? [] : ((try? tv!.friends()) ?? [])
            return UsersDevices(accounts: await accounts, devices: await devices, clients: await clients, friends: await friends)
        }

        // MARK: Libraries

        public nonisolated struct Choices: Sendable, Equatable {
            public struct Agent: Sendable, Equatable {
                public var identifier: String
                public var title: String
            }

            public var agents: [Agent]
            public var scanners: [String]
        }

        /// Agents and scanners for a new library of `kind` (`movie`, `show`, `artist`, `photo`).
        public static func libraryChoices(_ app: AppState, _ kind: String) async throws -> Choices {
            let server = try app.server()
            let id = Util.mediaTypeId(kind) ?? 1
            async let agents = try? server.agents(id)
            async let scanners = try? server.scanners(id)
            func pick(_ mc: MediaContainer?, _ key: String) -> [JSONValue] {
                switch mc?.extra[key] {
                case .array(let a)?: a
                case let v? where v.objectValue != nil: [v]
                default: []
                }
            }
            var out = Choices(agents: [], scanners: [])
            for a in pick(await agents, "Agent") {
                if let identifier = a["identifier"]?.stringValue {
                    out.agents.append(Choices.Agent(identifier: identifier, title: a["title"]?.stringValue ?? ""))
                }
            }
            for a in pick(await scanners, "Scanner") {
                if let name = a["name"]?.stringValue { out.scanners.append(name) }
            }
            return out
        }

        public nonisolated struct LibraryForm: Sendable, Equatable {
            public var name: String
            public var kind: String
            public var agent: String
            public var scanner: String
            public var language: String
            public var locations: [String]

            public init(name: String, kind: String, agent: String, scanner: String, language: String, locations: [String]) {
                self.name = name
                self.kind = kind
                self.agent = agent
                self.scanner = scanner
                self.language = language
                self.locations = locations
            }
        }

        private nonisolated static func pushLocations(_ p: inout Params, _ locations: [String]) {
            for l in locations where !l.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                p.push("location", l.trimmingCharacters(in: .whitespacesAndNewlines))
            }
        }

        public static func createLibrary(_ app: AppState, _ form: LibraryForm) async throws {
            var p = Params()
                .set("name", form.name.trimmingCharacters(in: .whitespacesAndNewlines))
                .set("type", form.kind)
                .set("agent", form.agent)
                .set("scanner", form.scanner)
                .set("language", form.language)
            pushLocations(&p, form.locations)
            _ = try await app.server().createSection(p)
            try await Session.reloadSections(app)
        }

        /// Rename a library and/or change its folders.
        public static func editLibrary(_ app: AppState, _ id: String, _ name: String, _ locations: [String]) async throws {
            let section = app.sections().first { $0.key == id }
            var p = Params()
                .set("name", name.trimmingCharacters(in: .whitespacesAndNewlines))
                .opt("agent", section?.agent)
                .opt("scanner", section?.scanner)
                .opt("language", section?.language)
            pushLocations(&p, locations)
            try await app.server().editSection(id, params: p)
            try await Session.reloadSections(app)
        }

        public static func deleteLibrary(_ app: AppState, _ id: String) async throws {
            try await app.server().deleteSection(id)
            try await Session.reloadSections(app)
        }

        // MARK: Statistics

        public nonisolated struct Bucket: Sendable, Equatable {
            public var at: Int
            public var lan: Int
            public var wan: Int
        }

        public nonisolated struct Usage: Sendable, Equatable {
            public var name: String
            public var bytes: Int
        }

        public nonisolated struct Stats: Sendable, Equatable {
            /// Bytes streamed per time bucket, oldest first.
            public var bandwidth: [Bucket]
            /// Bytes streamed per account, most first.
            public var accounts: [Usage]
            /// Bytes streamed per device, most first.
            public var devices: [Usage]
            public var plays: Int
            public var playSeconds: Int
        }

        /// Bandwidth and plays since `since` (unix seconds). `timespan` is Plex's bucket size
        /// code: 6 for hours, 4 for days.
        public static func serverStats(_ app: AppState, _ timespan: Int, _ since: Int) async throws -> Stats {
            let server = try app.server()
            async let media = (try? server.statisticsMedia(timespan))?.statisticsMedia ?? []
            let bandwidth = try await server.statisticsBandwidth(timespan)
            var accountNames: [Int: String] = [:]
            for a in bandwidth.account { if let id = a.id, let name = a.name { accountNames[id] = name } }
            var deviceNames: [Int: String] = [:]
            for d in bandwidth.device { if let id = d.id, let name = d.name { deviceNames[id] = name } }
            var buckets: [Int: Bucket] = [:]
            // Insertion order, for the stable sort by bytes below (a `Map` iterates in it).
            var accountOrder: [Int] = []
            var deviceOrder: [Int] = []
            var byAccount: [Int: Int] = [:]
            var byDevice: [Int: Int] = [:]
            for b in bandwidth.statisticsBandwidth where (b.at ?? 0) >= since {
                let at = b.at ?? 0
                let bytes = b.bytes ?? 0
                var e = buckets[at] ?? Bucket(at: at, lan: 0, wan: 0)
                if b.lan { e.lan += bytes } else { e.wan += bytes }
                buckets[at] = e
                let account = b.accountID ?? 0
                if byAccount[account] == nil { accountOrder.append(account) }
                byAccount[account, default: 0] += bytes
                let device = b.deviceID ?? 0
                if byDevice[device] == nil { deviceOrder.append(device) }
                byDevice[device, default: 0] += bytes
            }
            func rank(_ order: [Int], _ map: [Int: Int], _ names: [Int: String], _ unknown: String) -> [Usage] {
                let list = order.enumerated().map { i, id in (i, Usage(name: names[id] ?? unknown, bytes: map[id] ?? 0)) }
                return list.sorted { a, b in a.1.bytes != b.1.bytes ? a.1.bytes > b.1.bytes : a.0 < b.0 }
                    .prefix(8).map(\.1)
            }
            var plays = 0
            var playSeconds = 0
            for m in await media where (m.at ?? 0) >= since {
                plays += m.count ?? 0
                playSeconds += m.duration ?? 0
            }
            return Stats(
                bandwidth: buckets.values.sorted { $0.at < $1.at },
                accounts: rank(accountOrder, byAccount, accountNames, "Unknown user"),
                devices: rank(deviceOrder, byDevice, deviceNames, "Unknown device"), plays: plays,
                playSeconds: playSeconds)
        }

        /// Save the server's logs (`logs`) or database (`database`) as a zip in the downloads
        /// folder (Documents/Downloads, which the Files app shows). Returns the file, for the
        /// front end to offer in a share sheet.
        public static func saveDiagnostics(_ app: AppState, _ what: String) async throws -> URL {
            let server = try app.server()
            let bytes: Data
            switch what {
            case "logs": bytes = try await server.downloadLogs()
            case "database": bytes = try await server.downloadDatabase()
            default: throw CoreError("Unknown diagnostics \(what).")
            }
            let dir = app.paths.downloads
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
            let stamp = Int(Date().timeIntervalSince1970)
            let file = dir.appendingPathComponent("plex-\(what)-\(stamp).zip")
            try bytes.write(to: file, options: .atomic)
            return file
        }
    }
}
