// Port of desktop/core/storage.ts.
// Atomic, private file replacement and JSON read-modify-write transactions.
//
// The desktop takes OS file locks on `<path>.lock` so the TUI and the desktop app exclude
// each other. An iOS app is one process, and these functions run on the main actor, so a
// transaction can't interleave with another: `lock` is gone, and `tryLock` is an in-process
// lease for work that spans awaits (the offline sync).

import Foundation

public enum Storage {
    /// A held lease; `release()` gives it back (also safe to call twice).
    public final class Lock {
        private let path: String
        private var held = true

        fileprivate init(path: String) {
            self.path = path
        }

        public func release() {
            guard held else { return }
            held = false
            Storage.leases.remove(path)
        }
    }

    fileprivate static var leases = Set<String>()

    /// Take the lease of `path` if nobody holds it; `nil` otherwise.
    public static func tryLock(_ path: URL) -> Lock? {
        let key = path.standardizedFileURL.path
        guard leases.insert(key).inserted else { return nil }
        return Lock(path: key)
    }

    // MARK: Atomic writes and JSON transactions

    /// Write `bytes` to a unique private (0600) temporary file next to `path` and move it
    /// into place, so readers see the old or the new file and never half of one.
    public nonisolated static func write(_ path: URL, _ bytes: Data) throws {
        let parent = path.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: parent, withIntermediateDirectories: true)
        let temp = parent.appendingPathComponent(".tmp\(UUID().uuidString.prefix(12).lowercased())")
        // 0o600 from creation, exclusive: never reuses another writer's file.
        let fd = open(temp.path, O_WRONLY | O_CREAT | O_EXCL, 0o600)
        guard fd >= 0 else { throw CoreError("saving \(path.path): \(String(cString: strerror(errno)))") }
        let ok = bytes.withUnsafeBytes { buf -> Bool in
            var off = 0
            while off < buf.count {
                let n = Foundation.write(fd, buf.baseAddress! + off, buf.count - off)
                if n <= 0 { return false }
                off += n
            }
            return fsync(fd) == 0
        }
        close(fd)
        guard ok else {
            unlink(temp.path)
            throw CoreError("saving \(path.path): \(String(cString: strerror(errno)))")
        }
        guard rename(temp.path, path.path) == 0 else {
            let reason = String(cString: strerror(errno))
            unlink(temp.path)
            throw CoreError("saving \(path.path): \(reason)")
        }
        let dir = open(parent.path, O_RDONLY)
        if dir >= 0 {
            fsync(dir)
            close(dir)
        }
    }

    /// The JSON file at `path`, or `def()` when it doesn't exist.
    public nonisolated static func readJson<T: Decodable>(_ path: URL, _ def: () -> T) throws -> T {
        let bytes: Data
        do {
            bytes = try Data(contentsOf: path)
        } catch let e as CocoaError where e.code == .fileReadNoSuchFile {
            return def()
        }
        do {
            return try JSONDecoder().decode(T.self, from: bytes)
        } catch {
            throw CoreError("reading \(path.path): \(decodeText(error))")
        }
    }

    /// Pretty JSON like `serde_json::to_vec_pretty` (keys sorted, so equal values compare equal).
    public nonisolated static func pretty<T: Encodable>(_ value: T) throws -> Data {
        let enc = JSONEncoder()
        enc.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        return try enc.encode(value)
    }

    /// A read-modify-write transaction. Corrupt state is never overwritten, and an update that
    /// changes nothing leaves the file alone.
    public static func updateJson<T: Codable, R>(_ path: URL, _ def: () -> T, _ update: (inout T) throws -> R) throws
        -> R
    {
        var value = try readJson(path, def)
        let before = try pretty(value)
        let result = try update(&value)
        let after = try pretty(value)
        // Polling workers often have nothing to claim or recover. Keep those transactions read-only.
        if before != after || !FileManager.default.fileExists(atPath: path.path) { try write(path, after) }
        return result
    }

    /// Like `updateJson` for values that are replaced rather than changed: `update` returns the new value.
    @discardableResult
    public static func replaceJson<T: Codable>(_ path: URL, _ def: () -> T, _ update: (T) throws -> T) throws -> T {
        let value = try readJson(path, def)
        let next = try update(value)
        let after = try pretty(next)
        if try pretty(value) != after || !FileManager.default.fileExists(atPath: path.path) { try write(path, after) }
        return next
    }
}

/// The message of a JSON decoding failure, without Foundation's wrapping.
nonisolated func decodeText(_ error: Error) -> String {
    switch error {
    case DecodingError.dataCorrupted(let c): return c.debugDescription
    case DecodingError.keyNotFound(let k, _): return "missing field `\(k.stringValue)`"
    case DecodingError.typeMismatch(_, let c): return c.debugDescription
    case DecodingError.valueNotFound(_, let c): return c.debugDescription
    default: return error.localizedDescription
    }
}
