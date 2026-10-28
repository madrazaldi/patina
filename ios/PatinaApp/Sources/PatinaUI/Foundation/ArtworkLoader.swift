// Where artwork comes from (the iOS counterpart of web/src/lib/images.ts and the desktop
// shell's image server). Everything goes through `ArtworkLoader.load`, so offline artwork
// and the shared disk cache can replace the transcoder fetch in one place.

import PatinaCore
import PlexKit
import UIKit

public enum ArtworkLoader {
    /// The app state the loader fetches with; the store sets it.
    static weak var app: AppState?

    private static let memory: NSCache<NSString, UIImage> = {
        let c = NSCache<NSString, UIImage>()
        c.totalCostLimit = 96 * 1024 * 1024
        return c
    }()

    /// Transcoded images on disk, under the cache directory (the system may purge it).
    private nonisolated static let session: URLSession = {
        let config = URLSessionConfiguration.default
        let dir = Paths.standard().cache.appendingPathComponent("artwork", isDirectory: true)
        config.urlCache = URLCache(memoryCapacity: 0, diskCapacity: 400 * 1024 * 1024, directory: dir)
        // Resized artwork never changes under the same URL (it carries the image's version).
        config.requestCachePolicy = .returnCacheDataElseLoad
        config.httpMaximumConnectionsPerHost = 6
        return URLSession(configuration: config)
    }()

    /// The desktop asks for sizes meant for a 2x screen; scale them for this one.
    static func scaled(_ n: Int) -> Int {
        let scale = UITraitCollection.current.displayScale
        return Int((Double(n) * max(1, scale / 2)).rounded(.up))
    }

    private static func key(_ source: String, _ w: Int, _ h: Int, _ server: String?) -> NSString {
        "\(server ?? "")|\(w)x\(h)|\(source)" as NSString
    }

    /// An image already in memory, to show without a flash.
    public static func cached(source: String, width: Int, height: Int, server: String? = nil) -> UIImage? {
        memory.object(forKey: key(source, width, height, server))
    }

    /// `source` (a server path or an absolute URL) resized to about `width`×`height` (the
    /// desktop's sizes). `server` is the machine id of another server of the account the
    /// image lives on. TODO(merge): `Images.data(...)` (downloads agent: offline artwork and
    /// the shared disk cache) replaces the fetch below.
    public static func load(source: String, width: Int, height: Int, server: String? = nil) async throws -> UIImage {
        let cacheKey = key(source, width, height, server)
        if let hit = memory.object(forKey: cacheKey) { return hit }
        let w = scaled(width)
        let h = scaled(height)
        let data: Data
        if let server, let app, !Servers.isCurrent(app, server) {
            guard let (bytes, _) = await Servers.image(app, server, source, w, h) else {
                throw CoreError("No artwork")
            }
            data = bytes
        } else {
            guard let current = Images.currentServer() else { throw CoreError("Not connected to a server.") }
            data = try await fetch(try current.imageUrl(source, width: w, height: h))
        }
        let image = try await decode(data)
        memory.setObject(image, forKey: cacheKey, cost: Int(image.size.width * image.size.height * image.scale * 4))
        return image
    }

    @concurrent
    private nonisolated static func fetch(_ url: URL) async throws -> Data {
        let (data, response) = try await session.data(from: url)
        if let http = response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) {
            throw CoreError("HTTP \(http.statusCode)")
        }
        return data
    }

    @concurrent
    private nonisolated static func decode(_ data: Data) async throws -> UIImage {
        guard let image = UIImage(data: data) else { throw CoreError("Not an image") }
        return await image.byPreparingForDisplay() ?? image
    }
}
