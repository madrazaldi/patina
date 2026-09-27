import AppKit
import SwiftUI

/// Decoded artwork, shared across views.
@MainActor
final class ImageCache {
    static let shared = ImageCache()
    private let cache = NSCache<NSString, NSImage>()
    private var inflight: [String: Task<NSImage?, Never>] = [:]

    init() { cache.countLimit = 600 }

    func image(for key: String, load: @escaping () async -> Data?) async -> NSImage? {
        if let hit = cache.object(forKey: key as NSString) { return hit }
        if let task = inflight[key] { return await task.value }
        let task = Task<NSImage?, Never> {
            guard let data = await load() else { return nil }
            return NSImage(data: data)
        }
        inflight[key] = task
        let image = await task.value
        inflight[key] = nil
        if let image { cache.setObject(image, forKey: key as NSString) }
        return image
    }

    func cached(_ key: String) -> NSImage? { cache.object(forKey: key as NSString) }
}

/// Where a picture comes from.
enum ImageSource: Hashable {
    /// A Plex image path or URL, fetched through the backend's cache at a size.
    case plex(String, width: Int, height: Int)
    /// A download's saved artwork (works offline).
    case download(String)

    var key: String {
        switch self {
        case .plex(let path, let w, let h): "\(path)@\(w)x\(h)"
        case .download(let id): "download:\(id)"
        }
    }

    func load() async -> Data? {
        switch self {
        case .plex(let path, let w, let h): await Core.image(path, width: w, height: h)
        case .download(let id): await Core.downloadImage(id)
        }
    }
}

/// Artwork that fills its frame. Shows a neutral placeholder (or `fallback`
/// text) until it loads, and fades in.
struct Artwork: View {
    let source: ImageSource?
    var fallback: String? = nil
    var contentMode: ContentMode = .fill

    @State private var image: NSImage?

    init(_ source: ImageSource?, fallback: String? = nil, contentMode: ContentMode = .fill) {
        self.source = source
        self.fallback = fallback
        self.contentMode = contentMode
        _image = State(initialValue: source.flatMap { ImageCache.shared.cached($0.key) })
    }

    /// A Plex image path at a size, or nothing.
    init(_ path: String?, width: Int, height: Int, fallback: String? = nil, contentMode: ContentMode = .fill) {
        self.init(path.map { .plex($0, width: width, height: height) }, fallback: fallback, contentMode: contentMode)
    }

    var body: some View {
        ZStack {
            Rectangle().fill(Theme.placeholder)
            if let image {
                Image(nsImage: image)
                    .resizable()
                    .aspectRatio(contentMode: contentMode)
                    .transition(.opacity)
            } else if let fallback {
                Text(fallback)
                    .font(Theme.display(13))
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .padding(8)
            }
        }
        .clipped()
        .task(id: source) {
            guard let source else { image = nil; return }
            if let hit = ImageCache.shared.cached(source.key) { image = hit; return }
            // Don't keep showing the previous source's picture (grids reuse views by position).
            image = nil
            let loaded = await ImageCache.shared.image(for: source.key, load: source.load)
            // A newer source may have loaded meanwhile; loads don't observe cancellation.
            guard !Task.isCancelled else { return }
            withAnimation(.easeOut(duration: 0.2)) { image = loaded }
        }
    }
}

/// A round avatar: a user's picture or their initials.
struct Avatar: View {
    let path: String?
    let name: String
    var size: CGFloat = 28

    var body: some View {
        Group {
            if let path {
                Artwork(path, width: Int(size * 2), height: Int(size * 2))
            } else {
                ZStack {
                    Circle().fill(Theme.placeholder)
                    Text(Fmt.initials(name)).font(.system(size: size * 0.38, weight: .medium)).foregroundStyle(.secondary)
                }
            }
        }
        .frame(width: size, height: size)
        .clipShape(Circle())
    }
}
