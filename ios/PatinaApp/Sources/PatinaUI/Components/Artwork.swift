// Port of web/src/components/Img.svelte: artwork that fades in once loaded, over a
// placeholder. It fills the frame it is given (cropping), so callers size it.

import PlexKit
import SwiftUI

public struct Artwork: View {
    let source: String?
    let width: Int
    let height: Int
    var server: String?
    /// Hide what it shows (unwatched episodes when spoilers are hidden).
    var blur = false
    /// Shown while there is no image (`nil`: the plain placeholder).
    var fallback: String?

    @State private var image: UIImage?
    @State private var loadedKey: String?
    @State private var failed = false

    /// `width`×`height` are the transcoder sizes the desktop asks for (the loader scales them).
    public init(
        _ source: String?, width: Int, height: Int, server: String? = nil, blur: Bool = false, fallback: String? = nil
    ) {
        self.source = source
        self.width = width
        self.height = height
        self.server = server
        self.blur = blur
        self.fallback = fallback
        if let source, let hit = ArtworkLoader.cached(source: source, width: width, height: height, server: server) {
            _image = State(initialValue: hit)
            _loadedKey = State(initialValue: "\(server ?? "")|\(width)x\(height)|\(source)")
        }
    }

    private var key: String { "\(server ?? "")|\(width)x\(height)|\(source ?? "")" }

    public var body: some View {
        Rectangle()
            .fill(Theme.raised)
            .overlay {
                if let image {
                    Image(uiImage: image)
                        .resizable()
                        .scaledToFill()
                        .blur(radius: blur ? 14 : 0, opaque: true)
                        .brightness(blur ? -0.15 : 0)
                        .transition(.opacity)
                } else if let fallback, source == nil || failed {
                    Text(fallback)
                        .font(.footnote.weight(.medium))
                        .foregroundStyle(Theme.tertiaryText)
                        .multilineTextAlignment(.center)
                        .padding(8)
                }
            }
            .clipped()
            .task(id: key) {
                if loadedKey == key { return }
                failed = false
                guard let source else {
                    image = nil
                    loadedKey = key
                    return
                }
                if let hit = ArtworkLoader.cached(source: source, width: width, height: height, server: server) {
                    image = hit
                    loadedKey = key
                    return
                }
                image = nil
                do {
                    let loaded = try await ArtworkLoader.load(source: source, width: width, height: height, server: server)
                    withAnimation(.easeOut(duration: 0.25)) { image = loaded }
                    loadedKey = key
                } catch {
                    failed = true
                }
            }
            .accessibilityHidden(true)
    }
}

#Preview {
    Artwork(nil, width: 300, height: 450, fallback: "The Long Voyage")
        .aspectRatio(2 / 3, contentMode: .fit)
        .frame(width: 160)
        .clipShape(.rect(cornerRadius: Theme.cornerRadius))
        .padding()
        .preferredColorScheme(.dark)
}
