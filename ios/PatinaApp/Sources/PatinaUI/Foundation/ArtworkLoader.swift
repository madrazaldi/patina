// Where artwork comes from (the iOS counterpart of web/src/lib/images.ts and the desktop
// shell's image server): the core's `Images` (its disk cache, other servers of the account,
// and the artwork downloads saved), decoded and kept in memory here.

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
    /// desktop's sizes), through the core's disk cache. `server` is the machine id of another
    /// server of the account the image lives on.
    public static func load(source: String, width: Int, height: Int, server: String? = nil) async throws -> UIImage {
        let cacheKey = key(source, width, height, server)
        if let hit = memory.object(forKey: cacheKey) { return hit }
        guard let app else { throw CoreError("Not connected to a server.") }
        let data = try await Images.data(app, source: source, width: scaled(width), height: scaled(height), server: server)
        return remember(try await decode(data), cacheKey)
    }

    /// The artwork a download saved, which works offline (the desktop's `/d?id=`).
    public static func load(download id: String) async throws -> UIImage {
        let cacheKey = "download|\(id)" as NSString
        if let hit = memory.object(forKey: cacheKey) { return hit }
        guard let app else { throw CoreError("No artwork") }
        return remember(try await decode(Images.downloadData(app, id: id)), cacheKey)
    }

    public static func cached(download id: String) -> UIImage? {
        memory.object(forKey: "download|\(id)" as NSString)
    }

    private static func remember(_ image: UIImage, _ key: NSString) -> UIImage {
        memory.setObject(image, forKey: key, cost: Int(image.size.width * image.size.height * image.scale * 4))
        return image
    }

    @concurrent
    private nonisolated static func decode(_ data: Data) async throws -> UIImage {
        guard let image = UIImage(data: data) else { throw CoreError("Not an image") }
        return await image.byPreparingForDisplay() ?? image
    }
}
