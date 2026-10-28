// The media and photo routes of tools/mock_pms.py (`send_file`, `/photo/:/transcode`), with
// its `--download-delay`, `--drop-download-once` and `--resume-validator` options.

import Foundation
import PlexKit
import Synchronization

/// The file served for every `/library/parts/<id>/<n>/file.mkv`.
nonisolated final class MediaFile: Sendable {
    enum Validator: Sendable {
        /// Range answers carry the file's ETag.
        case normal
        /// Range answers carry no validator.
        case missing
        /// Range answers carry a different ETag.
        case changed
    }

    let bytes: Data
    let delay: Double
    let dropOnce: Bool
    let validator: Validator
    let etag: String
    private let dropped = Mutex(false)

    init(_ bytes: Data, delay: Double = 0, dropOnce: Bool = false, validator: Validator = .normal) {
        self.bytes = bytes
        self.delay = delay
        self.dropOnce = dropOnce
        self.validator = validator
        etag = "\"mock-\(bytes.count)-1700000000\""
    }

    /// Whether this transfer is the one that drops (only the first, once past 256 KiB).
    fileprivate func dropNow() -> Bool {
        dropOnce && dropped.withLock { d in
            defer { d = true }
            return !d
        }
    }
}

/// A tiny JPEG for artwork.
nonisolated let mockJpeg = Data([0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10, 0x4A, 0x46, 0x49, 0x46, 0, 1, 0xFF, 0xD9])

nonisolated extension MockPMS {
    /// Serve `file` for the media parts (`send_file`), and artwork for the photo transcoder.
    func serve(_ file: MediaFile) {
        on { mock, req in
            if req.path == "/photo/:/transcode" { return MockResponse(headers: ["Content-Type": "image/jpeg"], body: mockJpeg) }
            guard req.path.wholeMatch(of: #/\/library\/parts\/\d+\/\d+\/file\.mkv/#) != nil else { return nil }
            let size = file.bytes.count
            var rng = req.headers["Range"]
            if let ifRange = req.headers["If-Range"], ifRange != file.etag { rng = nil }
            var logged = req.query
            logged["range"] = [rng ?? "full"]
            mock.record("download", MockRequest(method: req.method, path: req.path, query: logged, headers: req.headers))
            var start = 0
            var end = size - 1
            var headers = ["Content-Type": "video/x-matroska", "Accept-Ranges": "bytes"]
            var status = 200
            if let rng {
                if let m = rng.wholeMatch(of: #/bytes=(\d*)-(\d*)/#) {
                    start = Int(m.1) ?? 0
                    end = Int(m.2) ?? size - 1
                }
                if start >= size {
                    return MockResponse(status: 416, headers: ["Content-Range": "bytes */\(size)"])
                }
                end = min(end, size - 1)
                status = 206
                headers["Content-Range"] = "bytes \(start)-\(end)/\(size)"
            }
            if rng == nil || file.validator != .missing {
                headers["ETag"] = rng != nil && file.validator == .changed ? "\"changed\"" : file.etag
            }
            headers["Content-Length"] = String(end - start + 1)
            let chunk = 1 << 16
            // `left < size - 262144` after a chunk: the first chunk boundary past 256 KiB of the file.
            let drop = file.dropNow() ? max(chunk, ((262_144 - (size - (end - start + 1))) / chunk + 1) * chunk) : nil
            return MockResponse(
                status: status, headers: headers, body: file.bytes.subdata(in: start..<(end + 1)), chunkSize: chunk,
                chunkDelay: file.delay, dropAfter: drop)
        }
    }
}
