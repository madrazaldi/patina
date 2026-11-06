// Port of desktop/core/images.ts (and of the request handling of desktop/shell/images-server.ts).
// Artwork: images are resolved through the current server's photo transcoder (so front ends
// never see the token) and the resized result is cached on disk, in `<cache>/images`.
//
// The cache is built for a grid of hundreds of posters scrolling past at once: simultaneous
// requests for one image share a single fetch, fetches are limited to MAX_FETCHES at a time,
// entries are written atomically (a reader never sees a half-written file), and the folder is
// kept under a size budget as well as an age limit.
//
// On the desktop the webview asks a loopback server for `/i?u=&w=&h=&s=` and `/d?id=`; on iOS
// the UI calls `Images.data` and `Images.downloadData` directly. Artwork of another server
// (`s`) goes into the same cache under that server's id; the desktop left it to the webview's
// HTTP cache. Disk work runs off the main actor.

import CryptoKit
import Foundation
import PlexKit

public enum Images {
    /// Images fetched from the server or the web at the same time.
    public nonisolated static let MAX_FETCHES = 8
    /// The most disk the cache keeps; the least recently used entries go first.
    public nonisolated static let BUDGET_BYTES = 512 * 1024 * 1024
    /// Entries unused for this long are dropped (30 days, in seconds).
    public nonisolated static let MAX_AGE: TimeInterval = 60 * 60 * 24 * 30
    /// Written bytes after which the budget is checked again.
    static let PRUNE_EVERY_BYTES = 32 * 1024 * 1024
    /// A hit older than this has its timestamp refreshed, so use keeps entries alive.
    nonisolated static let TOUCH_AFTER: TimeInterval = 60 * 60
    /// Direct (non-Plex) fetches give up after this long.
    static let HTTP_TIMEOUT: TimeInterval = 30
    /// Temporary files carry this between the entry's name and a random suffix.
    nonisolated static let TMP_MARK = ".tmp-"

    /// An image and its content type.
    public typealias Fetched = (Data, String)

    static var server: PlexServer?
    private static var written = 0
    private static var pruning = false
    private static let permits = Semaphore(MAX_FETCHES)
    private static let flights = SingleFlight<Fetched?>()

    /// The cache folder.
    nonisolated static func cacheRoot(_ paths: Paths) -> URL {
        paths.cache.appendingPathComponent("images")
    }

    /// Counting semaphore (tokio's `Semaphore`).
    final class Semaphore {
        private var free: Int
        private var waiters: [CheckedContinuation<Void, Never>] = []

        init(_ permits: Int) {
            free = permits
        }

        /// Wait for a permit; the returned function gives it back.
        func acquire() async -> () -> Void {
            if free > 0 {
                free -= 1
            } else {
                await withCheckedContinuation { waiters.append($0) }
            }
            var released = false
            return { [self] in
                if released { return }
                released = true
                if waiters.isEmpty {
                    free += 1
                } else {
                    waiters.removeFirst().resume()
                }
            }
        }
    }

    /// Runs one task per key and lets every caller for that key wait on it.
    ///
    /// The task is started, not driven by its callers: if all of them give up the work still
    /// finishes (and releases whatever it holds), and the key is forgotten the moment it does.
    final class SingleFlight<V: Sendable> {
        private var flights: [String: Task<V?, Never>] = [:]

        /// The result of the task for `key`, starting it with `make` unless one is already
        /// running. `nil` if the task failed.
        func run(_ key: String, _ make: @escaping () async throws -> V) async -> V? {
            if let existing = flights[key] { return await existing.value }
            let task = Task<V?, Never> {
                defer { flights[key] = nil }
                do {
                    return try await make()
                } catch {
                    return nil
                }
            }
            flights[key] = task
            return await task.value
        }

        var running: Int { flights.count }
    }

    /// The server images are fetched through; `nil` when signed out.
    public static func setServer(_ s: PlexServer?) {
        server = s
    }

    /// The current image server (tests).
    public static func currentServer() -> PlexServer? {
        server
    }

    // MARK: What the UI asks for

    /// A server path or absolute URL resized to fit `width` by `height`, from the cache when
    /// it can be (the desktop's `/i?u=&w=&h=&s=`). `server` names another server of the
    /// account the artwork comes from (`Metadata.serverId`); `nil` is the current one.
    public static func data(_ app: AppState, source: String, width: Int = 300, height: Int = 450, server: String? = nil)
        async throws -> Data
    {
        let found: Fetched?
        if let on = server, !on.isEmpty, !Servers.isCurrent(app, on) {
            found = await fetch(app.paths, scope: on, source, width, height) {
                await Servers.image(app, on, source, width, height)
            }
        } else {
            found = await fetch(app, source, width, height)
        }
        guard let found else { throw CoreError("Image not found.") }
        return found.0
    }

    /// Saved artwork of a download; works offline (the desktop's `/d?id=`).
    public static func downloadData(_ app: AppState, id: String) async throws -> Data {
        guard let found = await downloadPoster(app.paths, id) else { throw CoreError("Image not found.") }
        return found.0
    }

    /// Saved artwork of a download and its content type.
    @concurrent public nonisolated static func downloadPoster(_ paths: Paths, _ id: String) async -> Fetched? {
        guard let path = Downloads.poster(paths, id), let bytes = try? Data(contentsOf: URL(fileURLWithPath: path)) else {
            return nil
        }
        return (bytes, sniff(bytes))
    }

    // MARK: The cache

    public nonisolated static func cachePath(_ paths: Paths, _ s: PlexServer?, _ source: String, _ w: Int, _ h: Int) -> URL {
        cachePath(paths, scope: s?.machineIdentifier ?? "", source, w, h)
    }

    nonisolated static func cachePath(_ paths: Paths, scope: String, _ source: String, _ w: Int, _ h: Int) -> URL {
        let key = SHA256.hash(data: Data("\(scope)|\(source)|\(w)|\(h)".utf8)).map { String(format: "%02x", $0) }.joined()
        return cacheRoot(paths).appendingPathComponent(String(key.prefix(2))).appendingPathComponent(key)
    }

    /// A resized image as a file on disk, for things outside the front end (Now Playing, downloads).
    public static func cachedFile(_ app: AppState, _ source: String, _ w: Int, _ h: Int) async -> URL? {
        await cachedFile(app, server, source, w, h)
    }

    static func cachedFile(_ app: AppState, _ s: PlexServer?, _ source: String, _ w: Int, _ h: Int) async -> URL? {
        guard await fetch(app, s, source, w, h) != nil else { return nil }
        return cachePath(app.paths, s, source, w, h)
    }

    /// A server path or absolute URL resized to fit `w` by `h`, with its content type.
    public static func fetch(_ app: AppState, _ source: String, _ w: Int, _ h: Int) async -> Fetched? {
        await fetch(app, server, source, w, h)
    }

    static func fetch(_ app: AppState, _ s: PlexServer?, _ source: String, _ w: Int, _ h: Int) async -> Fetched? {
        let session = app.environment.urlSession ?? .shared
        return await fetch(app.paths, scope: s?.machineIdentifier ?? "", source, w, h) {
            await download(s, session, source, w, h)
        }
    }

    /// The cached entry of `scope|source|w|h`, else `download`'s result, stored.
    private static func fetch(
        _ paths: Paths, scope: String, _ source: String, _ w: Int, _ h: Int,
        _ download: @escaping () async -> Fetched?
    ) async -> Fetched? {
        if source.isEmpty { return nil }
        let path = cachePath(paths, scope: scope, source, w, h)
        if let hit = await readCached(path) { return hit }
        let shared = await flights.run(path.path) {
            // Another flight may have stored it between the miss and now.
            if let again = await readCached(path) { return again }
            let release = await permits.acquire()
            defer { release() }
            guard let got = await download() else { return nil }
            // The content type is re-sniffed from the bytes on cache hits.
            do {
                try await writeAtomic(path, got.0)
                noteWritten(paths, got.0.count)
            } catch {
                // A cache that can't be written still serves this request.
            }
            return got
        }
        return shared ?? nil
    }

    /// The cached bytes of `path`, if any (an empty file counts as a miss).
    @concurrent nonisolated static func readCached(_ path: URL) async -> Fetched? {
        guard let bytes = try? Data(contentsOf: path), !bytes.isEmpty else { return nil }
        touchIfStale(path)
        return (bytes, sniff(bytes))
    }

    /// Mark an old entry as used just now.
    nonisolated private static func touchIfStale(_ path: URL) {
        guard let m = try? FileManager.default.attributesOfItem(atPath: path.path)[.modificationDate] as? Date,
            Date().timeIntervalSince(m) > TOUCH_AFTER
        else { return }
        try? FileManager.default.setAttributes([.modificationDate: Date()], ofItemAtPath: path.path)
    }

    /// Ask the server (or the web) for the image.
    private static func download(_ s: PlexServer?, _ session: URLSession, _ source: String, _ w: Int, _ h: Int) async
        -> Fetched?
    {
        let isAbsolute = source.hasPrefix("http://") || source.hasPrefix("https://")
        var got: (Data, String)?
        if let s, !isAbsolute {
            got = (try? await s.image(source, width: w, height: h)).map { ($0.0, $0.1 ?? "") }
        } else if let s, isAbsolute, source.contains("plex.tv") || source.contains("plex.direct") {
            // Let the server resize external Plex artwork too, falling back to a direct fetch.
            if let r = try? await s.image(source, width: w, height: h) {
                got = (r.0, r.1 ?? "")
            } else {
                got = await direct(session, source)
            }
        } else if isAbsolute {
            got = await direct(session, source)
        }
        guard let (bytes, ctype) = got, !bytes.isEmpty else { return nil }
        return (bytes, ctype.hasPrefix("image/") ? ctype : "image/jpeg")
    }

    private static func direct(_ session: URLSession, _ url: String) async -> (Data, String)? {
        guard let u = URL(string: url) else { return nil }
        var req = URLRequest(url: u)
        req.timeoutInterval = HTTP_TIMEOUT
        guard let (data, resp) = try? await session.data(for: req), let http = resp as? HTTPURLResponse,
            (200..<300).contains(http.statusCode)
        else { return nil }
        return (data, http.value(forHTTPHeaderField: "Content-Type") ?? "")
    }

    /// Write to a temporary sibling, then rename over `path`, so readers see either the whole
    /// file or none of it.
    @concurrent nonisolated static func writeAtomic(_ path: URL, _ bytes: Data) async throws {
        let dir = path.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let suffix = UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased()
        let tmp = dir.appendingPathComponent("\(path.lastPathComponent)\(TMP_MARK)\(suffix)")
        do {
            try bytes.write(to: tmp)
            guard rename(tmp.path, path.path) == 0 else { throw CoreError(String(cString: strerror(errno))) }
        } catch {
            unlink(tmp.path)
            throw error
        }
    }

    /// Count written bytes and check the budget in the background once enough accumulate.
    private static func noteWritten(_ paths: Paths, _ bytes: Int) {
        written += bytes
        if written < PRUNE_EVERY_BYTES || pruning { return }
        pruning = true
        written = 0
        let root = cacheRoot(paths)
        Task {
            await Task.detached { _ = pruneDir(root, Date(), MAX_AGE, BUDGET_BYTES) }.value
            pruning = false
        }
    }

    /// The content type of an image, from its first bytes.
    public nonisolated static func sniff(_ head: Data) -> String {
        let b = [UInt8](head.prefix(8))
        if b.starts(with: [0x89, 0x50, 0x4E, 0x47]) { return "image/png" }
        if b.starts(with: Array("RIFF".utf8)) { return "image/webp" }
        if b.starts(with: Array("GIF".utf8)) { return "image/gif" }
        return "image/jpeg"
    }

    /// Drop cached images not used for 30 days, then the least recently used ones until the
    /// cache fits its size budget. The app runs this once at launch (as `main.ts` does).
    @discardableResult
    @concurrent public nonisolated static func pruneCache(_ paths: Paths) async -> Pruned {
        pruneDir(cacheRoot(paths), Date(), MAX_AGE, BUDGET_BYTES)
    }

    /// What a prune removed.
    public nonisolated struct Pruned: Sendable, Equatable {
        public var files = 0
        public var bytes = 0
    }

    /// Prune the two-level cache folder `dir`: leftover temporary files, entries last used
    /// before `now - maxAge`, then oldest-used entries until the total is at most 90% of
    /// `budget` (so a full cache is not trimmed on every write). An entry's age is the later
    /// of its modification and access times.
    nonisolated static func pruneDir(_ dir: URL, _ now: Date, _ maxAge: TimeInterval, _ budget: Int) -> Pruned {
        var pruned = Pruned()
        var live: [(used: Double, size: Int, path: String)] = []
        var total = 0
        let fm = FileManager.default
        guard let subs = try? fm.contentsOfDirectory(atPath: dir.path) else { return pruned }
        let nowSeconds = now.timeIntervalSince1970
        for sub in subs {
            let subdir = dir.appendingPathComponent(sub).path
            guard let files = try? fm.contentsOfDirectory(atPath: subdir) else { continue }
            for name in files {
                let path = subdir + "/" + name
                var st = stat()
                guard stat(path, &st) == 0, st.st_mode & S_IFMT == S_IFREG else { continue }
                let used = max(seconds(st.st_atimespec), seconds(st.st_mtimespec))
                let age = max(nowSeconds - used, 0)
                let size = Int(st.st_size)
                let temporary = name.contains(TMP_MARK)
                // A temporary file this old belongs to a write that died.
                let expired = temporary ? age > 3600 : age > maxAge
                if expired {
                    if unlink(path) == 0 {
                        pruned.files += 1
                        pruned.bytes += size
                    }
                } else if !temporary {
                    total += size
                    live.append((used, size, path))
                }
            }
        }
        if total > budget {
            let target = budget / 10 * 9
            live.sort { $0.used < $1.used }
            for entry in live {
                if total <= target { break }
                if unlink(entry.path) == 0 {
                    total -= entry.size
                    pruned.files += 1
                    pruned.bytes += entry.size
                }
            }
        }
        return pruned
    }

    nonisolated private static func seconds(_ t: timespec) -> Double {
        Double(t.tv_sec) + Double(t.tv_nsec) / 1e9
    }
}
