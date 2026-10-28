// Port of desktop/core/downloads.ts.
// Durable downloads. One worker fetches the queue in order; the manifest (`downloads.json`)
// is how pause, remove and progress reach it.
//
// What iOS changes: the file goes to `Paths.downloads`, and the bytes move through a
// `Transfer` (Downloads+Transfer.swift): a background `URLSession` in the app, which the
// system continues while the app is suspended, or a foreground stream (tests, fallback).
// There is one process, so the desktop's cross-process worker lease is an in-process one
// (`Storage.tryLock`), which keeps two `AppState`s on the same files (tests) from both
// running a worker. The manifest keeps absolute paths, as on the desktop, but iOS moves the
// app's container on updates, so stored paths are re-anchored under the current folders.

import Foundation
import PlexKit

nonisolated private let log = logger("downloads")

public nonisolated struct Download: Sendable, Equatable {
    public var id: String
    public var serverId: String
    public var serverName: String
    public var item: Metadata
    public var file: String?
    public var poster: String?
    public var size: Int
    public var received: Int
    public var state: Downloads.State
    public var error: String?
    public var addedAt: Int
    /// Account that queued this file. Legacy downloads remain playable but never sync to an inferred user.
    public var account: String
    public var validator: String?
    public var sourceKey: String?
    /// Smart-download policy that fetched this file and may remove it again (see Smart.swift);
    /// `nil` for downloads someone asked for by hand.
    public var policy: String?

    public init(
        id: String, serverId: String, serverName: String, item: Metadata, file: String? = nil, poster: String? = nil,
        size: Int = 0, received: Int = 0, state: Downloads.State = .queued, error: String? = nil, addedAt: Int = 0,
        account: String = "", validator: String? = nil, sourceKey: String? = nil, policy: String? = nil
    ) {
        self.id = id
        self.serverId = serverId
        self.serverName = serverName
        self.item = item
        self.file = file
        self.poster = poster
        self.size = size
        self.received = received
        self.state = state
        self.error = error
        self.addedAt = addedAt
        self.account = account
        self.validator = validator
        self.sourceKey = sourceKey
        self.policy = policy
    }
}

extension Download: Codable {
    /// `normalizeDownload`: a `Download` as serde would read it, `#[serde(default)]` fields
    /// filled, the item normalised.
    public init(from decoder: Decoder) throws {
        self = Downloads.normalize(try JSONValue(from: decoder))
    }

    public func encode(to encoder: Encoder) throws {
        let o: [String: JSONValue] = [
            "id": .string(id), "serverId": .string(serverId), "serverName": .string(serverName), "item": item.json,
            "file": JSONValue(file), "poster": JSONValue(poster), "size": JSONValue(size),
            "received": JSONValue(received), "state": .string(state.rawValue), "error": JSONValue(error),
            "addedAt": JSONValue(addedAt), "account": .string(account), "validator": JSONValue(validator),
            "sourceKey": JSONValue(sourceKey), "policy": JSONValue(policy),
        ]
        try JSONValue.object(o).encode(to: encoder)
    }
}

/// A failure while reading or writing the transfer; the same kinds reqwest retried (timeout, connect, body).
nonisolated struct TransferError: Error, LocalizedError, Sendable {
    let message: String
    let retry: Bool

    init(_ message: String, retry: Bool) {
        self.message = message
        self.retry = retry
    }

    var errorDescription: String? { message }
}

public enum Downloads {
    public nonisolated enum State: String, Sendable, Codable {
        case queued, downloading, paused, done, failed
    }

    /// The running worker of an app (the TypeScript `running` flag).
    final class Worker {
        var running = false
        var task: Task<Void?, Never>?
        /// The transfer moving the current file has handed it to the system (background refresh can end).
        var handedOff = false
    }

    static func worker(_ app: AppState) -> Worker {
        app.local(Worker.self) { Worker() }
    }

    nonisolated static func manifest(_ paths: Paths) -> URL {
        paths.config.appendingPathComponent("downloads.json")
    }

    nonisolated static func artworkDir(_ paths: Paths) -> URL {
        paths.data.appendingPathComponent("artwork")
    }

    // MARK: Reading the manifest

    nonisolated static func normalize(_ v: JSONValue) -> Download {
        let o = v.objectValue ?? [:]
        func text(_ v: JSONValue?) -> String {
            switch v {
            case .string(let s): s
            case .number(let n): n == n.rounded() && abs(n) < 1e15 ? String(Int(n)) : String(n)
            case .bool(let b): String(b)
            default: ""
            }
        }
        func str(_ v: JSONValue?) -> String? { v?.stringValue }
        func int(_ v: JSONValue?) -> Int {
            guard let n = v?.doubleValue, n.isFinite, abs(n) < 9e18 else { return 0 }
            return Int(n)
        }
        let item = (try? Metadata(json: o["item"] ?? .object([:]))) ?? Metadata()
        return Download(
            id: text(o["id"]), serverId: text(o["serverId"]), serverName: text(o["serverName"]), item: item,
            file: str(o["file"]), poster: str(o["poster"]), size: int(o["size"]), received: int(o["received"]),
            state: str(o["state"]).flatMap(State.init(rawValue:)) ?? .failed, error: str(o["error"]),
            addedAt: int(o["addedAt"]), account: str(o["account"]) ?? "", validator: str(o["validator"]),
            sourceKey: str(o["sourceKey"]), policy: str(o["policy"]))
    }

    /// A stored absolute path, moved under `dir` when it points into an older container of
    /// this app (iOS moves the container on updates).
    nonisolated static func relocate(_ stored: String?, _ dir: URL) -> String? {
        guard let stored else { return nil }
        let base = dir.standardizedFileURL.path
        if stored.hasPrefix(base + "/") { return stored }
        let anchor = "/\(dir.deletingLastPathComponent().lastPathComponent)/\(dir.lastPathComponent)/"
        guard let r = stored.range(of: anchor) else { return stored }
        return base + "/" + stored[r.upperBound...]
    }

    nonisolated static func relocate(_ list: inout [Download], _ paths: Paths) {
        for i in list.indices {
            list[i].file = relocate(list[i].file, paths.downloads)
            list[i].poster = relocate(list[i].poster, paths.data)
        }
    }

    nonisolated static func readRaw(_ paths: Paths) throws -> [Download] {
        var list = try Storage.readJson(manifest(paths)) { [Download]() }
        relocate(&list, paths)
        return list
    }

    static func update<R>(_ paths: Paths, _ f: (inout [Download]) throws -> R) throws -> R {
        try Storage.updateJson(manifest(paths), { [Download]() }) { list in
            relocate(&list, paths)
            return try f(&list)
        }
    }

    /// The manifest (`downloads.json` in the config dir).
    public nonisolated static func list(_ paths: Paths) -> [Download] {
        do {
            return try readRaw(paths)
        } catch {
            log.error("downloads: \(errorText(error))")
            return []
        }
    }

    nonisolated static func entry(_ paths: Paths, _ id: String) -> Download? {
        do {
            return try readRaw(paths).first { $0.id == id }
        } catch {
            log.error("downloads: \(errorText(error))")
            return nil
        }
    }

    static func emit(_ app: AppState) {
        app.emit(.downloads(list(app.paths)))
    }

    private static func change(_ app: AppState, _ f: (inout [Download]) -> Void) {
        do {
            try update(app.paths, f)
        } catch {
            app.error("Could not save downloads: \(errorText(error))")
        }
        emit(app)
    }

    /// The saved artwork of a download.
    public nonisolated static func poster(_ paths: Paths, _ id: String) -> String? {
        entry(paths, id)?.poster
    }

    /// Identity of a download: one file per server, account and item.
    public nonisolated static func idFor(_ serverId: String, _ account: String, _ ratingKey: String) -> String {
        "\(serverId):\(account):\(ratingKey)"
    }

    // MARK: Queueing and controlling

    /// Queue an item, or every playable item of a show, season, album or playlist. Returns how many were added.
    public static func enqueue(_ app: AppState, ratingKey: String) async throws -> Int {
        guard let session = app.session() else { throw CoreError("Not connected.") }
        let server = session.server
        guard let item = try await server.metadata(ratingKey) else { throw CoreError("Item not found.") }
        let items = Metadata.isPlayable(item) ? [item] : try await server.allLeaves(ratingKey).metadata
        return add(app, session, items, nil)
    }

    /// Queue the playable items for the session's server and account; those already
    /// downloaded are skipped. `policy` marks them as managed by a smart download policy.
    @discardableResult
    public static func add(_ app: AppState, _ session: Session, _ items: [Metadata], _ policy: String? = nil) -> Int {
        let account = Offline.account(session)
        var added = 0
        do {
            added = try update(app.paths) { list in
                var added = 0
                for item in items where Metadata.isPlayable(item) && !item.media.isEmpty {
                    let id = idFor(session.serverId, account, Metadata.ratingKey(item))
                    if list.contains(where: { $0.id == id }) { continue }
                    list.append(
                        Download(
                            id: id, serverId: session.serverId, serverName: session.serverName, item: item,
                            size: item.media.first?.part.first?.size ?? 0, state: .queued, addedAt: Offline.now(),
                            account: account, policy: policy))
                    added += 1
                }
                return added
            }
        } catch {
            app.error("Could not save downloads: \(errorText(error))")
        }
        emit(app)
        resume(app)
        return added
    }

    /// Change which policy owns downloads: `(download id, policy id or nil)`.
    public static func assign(_ paths: Paths, _ changes: [(String, String?)]) {
        do {
            try update(paths) { list in
                for (id, policy) in changes {
                    if let i = list.firstIndex(where: { $0.id == id }) { list[i].policy = policy }
                }
            }
        } catch {
            log.error("downloads: \(errorText(error))")
        }
    }

    public static func remove(_ app: AppState, _ id: String) {
        do {
            let removed = try update(app.paths) { list -> Download? in
                guard let i = list.firstIndex(where: { $0.id == id }) else { return nil }
                return list.remove(at: i)
            }
            if let removed { cleanup(app.paths, removed) }
        } catch {
            app.error("Could not remove download: \(errorText(error))")
        }
        emit(app)
    }

    /// Delete a download's file, artwork and whatever a transfer left behind.
    nonisolated static func cleanup(_ paths: Paths, _ d: Download) {
        for f in [d.file, d.poster].compactMap({ $0 }) {
            unlink(f)
            unlink(f + ".part")
        }
        stage(paths, d.id).clear()
    }

    public static func pause(_ app: AppState, _ id: String) {
        change(app) { list in
            if let i = list.firstIndex(where: { $0.id == id && ($0.state == .queued || $0.state == .downloading) }) {
                list[i].state = .paused
            }
        }
    }

    public static func retry(_ app: AppState, _ id: String) {
        change(app) { list in
            if let i = list.firstIndex(where: { $0.id == id && ($0.state == .failed || $0.state == .paused) }) {
                list[i].state = .queued
                list[i].error = nil
            }
        }
        resume(app)
    }

    /// Preserve server/account identity all the way into offline playback.
    public nonisolated static func files(_ paths: Paths, _ ids: [String]) -> [Download] {
        let all = list(paths)
        return ids.compactMap { id in
            all.first { $0.id == id && $0.state == .done }.flatMap { d in
                d.file.map { FileManager.default.fileExists(atPath: $0) } == true ? d : nil
            }
        }
    }

    // MARK: The worker

    /// Start (or continue) the transfer worker.
    public static func resume(_ app: AppState) {
        let w = worker(app)
        if w.running { return }
        w.running = true
        w.task = app.spawn {
            do {
                try await work(app)
            } catch is CancellationError {
            } catch {
                app.error("Downloads: \(errorText(error))")
            }
            w.running = false
            w.handedOff = false
        }
    }

    /// Stop the worker where it is, as if the process exited (tests).
    static func stop(_ app: AppState) async {
        let w = worker(app)
        w.task?.cancel()
        _ = await w.task?.value
    }

    private static func work(_ app: AppState) async throws {
        let paths = app.paths
        // One worker per set of files; another `AppState` on them leaves it alone.
        guard let lease = Storage.tryLock(paths.config.appendingPathComponent("download-worker")) else { return }
        defer { lease.release() }
        // With the lease held, any downloading entries were interrupted by a previous run.
        try update(paths) { list in
            for i in list.indices where list[i].state == .downloading { list[i].state = .queued }
        }
        while true {
            guard let session = app.session() else { break }
            let account = Offline.account(session)
            let next = try update(paths) { list -> Download? in
                guard
                    let i = list.firstIndex(where: {
                        $0.state == .queued && $0.serverId == session.serverId && ($0.account.isEmpty || $0.account == account)
                    })
                else { return nil }
                list[i].state = .downloading
                list[i].error = nil
                return list[i]
            }
            guard let d = next else { break }
            emit(app)
            var result: Result<Bool, any Error> = .success(false)
            for attempt in 0..<3 {
                do {
                    result = .success(try await fetchOne(app, session.server, d.id))
                } catch is CancellationError {
                    throw CancellationError()
                } catch {
                    result = .failure(error)
                }
                guard case .failure(let e) = result, isActive(paths, d.id), retryable(e) else { break }
                if attempt < 2 { try await Task.sleep(for: .milliseconds(1000 << attempt)) }
            }
            let outcome = result
            try update(paths) { list in
                guard let i = list.firstIndex(where: { $0.id == d.id && $0.state == .downloading }) else { return }
                switch outcome {
                case .success(true):
                    list[i].state = .done
                    list[i].error = nil
                case .success(false):
                    list[i].state = .paused
                case .failure(let e):
                    list[i].state = .failed
                    list[i].error = errorText(e)
                }
            }
            if entry(paths, d.id) == nil { cleanup(paths, d) }
            emit(app)
        }
    }

    nonisolated static func retryable(_ error: any Error) -> Bool {
        if let e = error as? TransferError { return e.retry }
        if let e = error as? PlexError {
            // `Http` wraps transport errors (timeouts, connection failures); `Status` retries only transient codes.
            if e.kind == .http { return true }
            return e.kind == .status && [429, 500, 502, 503, 504].contains(e.status ?? 0)
        }
        return false
    }

    nonisolated static func isActive(_ paths: Paths, _ id: String) -> Bool {
        entry(paths, id)?.state == .downloading
    }

    // MARK: File names

    public nonisolated static func clean(_ name: String) -> String {
        let bad: Set<Character> = [":", "/", "\\", "*", "?", "\"", "<", ">", "|"]
        let s = String(
            name.map { c -> Character in
                bad.contains(c) || c.unicodeScalars.contains(where: { $0.properties.generalCategory == .control })
                    ? " " : c
            })
        let words = s.split(whereSeparator: \.isWhitespace).joined(separator: " ")
        let trimmed = words.drop(while: { $0 == "." }).reversed().drop(while: { $0 == "." }).reversed()
        return trimmed.isEmpty ? "Untitled" : String(trimmed)
    }

    public nonisolated static func target(_ dir: URL, _ item: Metadata, _ ext: String) -> URL {
        let folder: URL
        let name: String
        switch Metadata.kind(item) {
        case "episode":
            folder = dir.appendingPathComponent(clean(item.grandparentTitle ?? "Shows"))
                .appendingPathComponent(clean(item.parentTitle ?? "Season"))
            name = "\(Metadata.episodeCode(item)) \(Metadata.title(item))"
        case "track":
            folder = dir.appendingPathComponent(clean(item.grandparentTitle ?? "Music"))
                .appendingPathComponent(clean(item.parentTitle ?? "Album"))
            name = "\(String(format: "%02d", item.index ?? 0)) \(Metadata.title(item))"
        default:
            folder = dir
            name = Metadata.fullTitle(item)
        }
        return folder.appendingPathComponent("\(clean(name)) [\(clean(Metadata.ratingKey(item)))].\(ext)")
    }

    // MARK: Responses

    /// Rust `str::parse::<u64>()` with its error texts.
    nonisolated static func parseU64(_ s: Substring) throws -> Int {
        if s.isEmpty { throw CoreError("cannot parse integer from empty string") }
        let digits = s.hasPrefix("+") ? s.dropFirst() : s
        guard !digits.isEmpty, digits.allSatisfy({ $0.isASCII && $0.isNumber }) else {
            throw CoreError("invalid digit found in string")
        }
        // Int is 64-bit but signed; offsets past Int.max can't be real anyway.
        guard let n = Int(digits) else { throw CoreError("number too large to fit in target type") }
        return n
    }

    /// Validate a resumed response before appending even one byte. Returns `(offset, total)`.
    public nonisolated static func responseOffset(_ status: Int, _ range: String?, _ requested: Int) throws -> (Int, Int?) {
        if status == 200 { return (0, nil) }
        if status != 206 { throw CoreError("Unexpected download response: \(status)") }
        guard let range, range.hasPrefix("bytes ") else { throw CoreError("Missing Content-Range") }
        let spec = range.dropFirst("bytes ".count)
        guard let slash = spec.firstIndex(of: "/") else { throw CoreError("Invalid Content-Range") }
        let span = spec[..<slash]
        let totalText = spec[spec.index(after: slash)...]
        guard let dash = span.firstIndex(of: "-") else { throw CoreError("Invalid byte range") }
        let start = try parseU64(span[..<dash])
        let end = try parseU64(span[span.index(after: dash)...])
        let total = try parseU64(totalText)
        guard start == requested && end >= start && end < total else {
            throw CoreError("Server returned the wrong byte range")
        }
        return (start, total)
    }

    /// Only a strong ETag or modification date can identify bytes in a saved partial file.
    public nonisolated static func responseValidator(_ resp: HTTPURLResponse) -> String? {
        if let etag = resp.value(forHTTPHeaderField: "ETag"), !etag.hasPrefix("W/") { return etag }
        return resp.value(forHTTPHeaderField: "Last-Modified")
    }

    /// What to make of the server's answer to a transfer that asked for `requested` bytes on
    /// with validator `saved`: start over, or append from `offset`.
    nonisolated enum Answer: Sendable, Equatable {
        case restart
        case accept(offset: Int, total: Int?, validator: String?)
    }

    /// The checks of `fetchOne` between the request and the first byte, for a response that is
    /// already in (the background transfer only sees it then). Non-2xx answers become the
    /// errors `Http` would have thrown.
    nonisolated static func judge(_ resp: HTTPURLResponse, requested: Int, saved: String?, partSize: Int?, body: () -> Data)
        throws -> Answer
    {
        let status = resp.statusCode
        if status == 416 && requested > 0 { return .restart }
        if status == 401 { throw PlexError.unauthorized() }
        if !(200..<300).contains(status) {
            let path = resp.url.map { URLComponents(url: $0, resolvingAgainstBaseURL: true)?.path ?? $0.path } ?? ""
            throw PlexError.statusError("GET", path, status, String(decoding: body().prefix(64 * 1024), as: UTF8.self))
        }
        // A correctly aligned range alone cannot prove that the source is unchanged.
        if requested > 0 && status == 206 && responseValidator(resp) != saved { return .restart }
        let (offset, rangeTotal) = try responseOffset(status, resp.value(forHTTPHeaderField: "Content-Range"), requested)
        let contentLength = resp.value(forHTTPHeaderField: "Content-Length")
        let length = contentLength.flatMap { $0.allSatisfy(\.isNumber) ? Int($0) : nil }
        let total = rangeTotal ?? length.map { offset + $0 } ?? partSize.flatMap { $0 >= 0 ? $0 : nil }
        return .accept(offset: offset, total: total, validator: responseValidator(resp))
    }

    /// Bytes free for this app's important data on the volume holding `url`.
    nonisolated static func availableSpace(_ url: URL) -> Int? {
        #if os(iOS) || os(macOS)
            if let v = try? url.resourceValues(forKeys: [.volumeAvailableCapacityForImportantUsageKey]),
                let n = v.volumeAvailableCapacityForImportantUsage
            {
                return Int(n)
            }
        #endif
        return (try? url.resourceValues(forKeys: [.volumeAvailableCapacityKey]))?.volumeAvailableCapacity
    }

    nonisolated static func checkSpace(_ dir: URL, _ needed: Int) throws {
        guard let available = availableSpace(dir) else { return }
        if available < max(needed, 0) { throw CoreError("Not enough free disk space for this download.") }
    }

    // MARK: One file

    private static func fetchOne(_ app: AppState, _ server: PlexServer, _ id: String) async throws -> Bool {
        let paths = app.paths
        guard let d = entry(paths, id) else { return false }
        guard let item = try await server.metadata(d.item.ratingKey ?? "") else {
            throw CoreError("The item no longer exists on the server.")
        }
        guard let part = item.media.first?.part.first, let key = part.key else { throw CoreError("No file to download.") }
        let ext =
            part.container.flatMap { c in
                c.count <= 12 && c.allSatisfy { $0.isASCII && ($0.isLetter || $0.isNumber) } ? c : nil
            } ?? "mkv"
        // Account/server ids separate files with identical titles and versions.
        let scope = Offline.digest("\(d.serverId):\(d.account)")
        let dir = paths.downloads.appendingPathComponent("\(clean(d.serverName)) - \(scope.prefix(8))")
        let path = d.file.map { URL(fileURLWithPath: $0) } ?? target(dir, item, ext)
        let parent = path.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: parent, withIntermediateDirectories: true)
        excludeFromBackup(paths.downloads)
        let partial = URL(fileURLWithPath: path.path + ".part")
        let existing = (try? FileManager.default.attributesOfItem(atPath: partial.path)[.size] as? Int) ?? 0
        let savedValidator = d.validator
        // Without a validator a partial file cannot safely be matched to a changed source.
        let sameSource = d.sourceKey == key
        let requested = sameSource && savedValidator != nil ? existing : 0
        if !sameSource { unlink(stage(paths, id).resumeData.path) }
        // The system holds a background transfer's bytes, so check before it starts too.
        if let size = part.size, size > 0 { try checkSpace(parent, size - requested) }

        let thumb = item.thumb ?? item.parentThumb ?? item.grandparentThumb
        var last = ContinuousClock.now
        let job = Job(
            id: id, server: server, key: key, path: path, requested: requested, validator: savedValidator,
            partSize: part.size)
        let hooks = Hooks(
            ready: { offset, total, validator in
                if let total { try checkSpace(parent, total - offset) }
                var posterPath = d.poster
                if posterPath == nil, let thumb, let cached = await Images.cachedFile(app, server, thumb, 480, 720) {
                    let art = artworkDir(paths)
                    let dest = art.appendingPathComponent("\(Offline.digest(id)).img")
                    // The artwork is optional.
                    if (try? FileManager.default.createDirectory(at: art, withIntermediateDirectories: true)) != nil {
                        unlink(dest.path)
                        if (try? FileManager.default.copyItem(at: cached, to: dest)) != nil { posterPath = dest.path }
                    }
                }
                try update(paths) { list in
                    guard let i = list.firstIndex(where: { $0.id == id }) else { return }
                    list[i].item = item
                    list[i].file = path.path
                    list[i].poster = posterPath
                    list[i].size = total ?? 0
                    list[i].received = offset
                    list[i].validator = validator
                    list[i].sourceKey = key
                }
                return isActive(paths, id)
            },
            progress: { received in
                guard ContinuousClock.now - last >= .milliseconds(500) else { return }
                last = ContinuousClock.now
                try? update(paths) { list in
                    if let i = list.firstIndex(where: { $0.id == id }) { list[i].received = received }
                }
                emit(app)
            },
            active: { isActive(paths, id) },
            handedOff: { worker(app).handedOff = $0 })

        guard let received = try await transfer(app).fetch(job, hooks) else {
            if entry(paths, id) == nil {
                unlink(path.path + ".part")
                unlink(path.path)
                stage(paths, id).clear()
            }
            return false
        }
        guard isActive(paths, id) else { return false }
        do {
            if FileManager.default.fileExists(atPath: path.path) { try FileManager.default.removeItem(at: path) }
            try FileManager.default.moveItem(at: partial, to: path)
        } catch {
            throw CoreError("saving \(path.path): \(error.localizedDescription)")
        }
        stage(paths, id).clear()
        try update(paths) { list in
            guard let i = list.firstIndex(where: { $0.id == id }) else { return }
            list[i].received = received
            list[i].size = received
            list[i].state = .done
        }
        return true
    }

    /// Downloads can be fetched again, so they stay out of iCloud backups.
    nonisolated static func excludeFromBackup(_ dir: URL) {
        var url = dir
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        try? url.setResourceValues(values)
    }
}
