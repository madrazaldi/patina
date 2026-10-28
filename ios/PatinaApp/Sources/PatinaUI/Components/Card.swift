// Port of web/src/components/Card.svelte: the poster, landscape or square card of any
// item, with its watched mark or unwatched count, its progress, and the item menu on a
// long press. Tapping opens the item (or toggles it in Select mode).

import PatinaCore
import PlexKit
import SwiftUI

extension EnvironmentValues {
    /// The namespace of the navigation stack the view is in, for zoom transitions from a card
    /// to the item it opens.
    @Entry var zoomNamespace: Namespace.ID?
}

public struct Card: View {
    let item: Metadata
    var shape: CardShape?
    /// Continue-watching style: backdrop art, the show's name as the title.
    var useArt = false
    var selecting = false
    var selected = false
    var onToggle: (() -> Void)?

    @Environment(AppStore.self) private var store
    @Environment(\.zoomNamespace) private var zoom

    public init(
        _ item: Metadata, shape: CardShape? = nil, useArt: Bool = false, selecting: Bool = false, selected: Bool = false,
        onToggle: (() -> Void)? = nil
    ) {
        self.item = item
        self.shape = shape
        self.useArt = useArt
        self.selecting = selecting
        self.selected = selected
        self.onToggle = onToggle
    }

    private var resolvedShape: CardShape { shape ?? PlexUI.shape(item) }
    private var kind: String { Metadata.kind(item) }

    private var source: String? {
        switch resolvedShape {
        case .landscape where useArt && kind != "episode": Metadata.backdrop(item) ?? item.thumb
        case .landscape: item.thumb ?? Metadata.backdrop(item)
        default: Metadata.poster(item)
        }
    }

    private var lines: (String, String) {
        kind == "episode" && useArt
            ? (item.grandparentTitle ?? "", "\(Metadata.episodeCode(item)) · \(Metadata.title(item))")
            : (Metadata.title(item), PlexUI.subtitle(item))
    }

    public var body: some View {
        Group {
            if selecting {
                Button {
                    Haptics.selection()
                    onToggle?()
                } label: {
                    content
                }
            } else {
                NavigationLink(value: PlexUI.route(item)) { content }
            }
        }
        .buttonStyle(CardButtonStyle())
        .contextMenu {
            ItemMenu(item: item)
        }
        .accessibilityLabel(Metadata.fullTitle(item))
        .accessibilityAddTraits(selecting && selected ? .isSelected : [])
    }

    private var content: some View {
        VStack(alignment: .leading, spacing: 0) {
            CardArt(item: item, shape: resolvedShape, source: source, selecting: selecting, selected: selected)
                .modifier(ZoomSource(id: PlexUI.route(item), namespace: zoom))
            Text(lines.0)
                .font(.subheadline.weight(.medium))
                .foregroundStyle(.primary)
                .lineLimit(1)
                .padding(.top, 8)
            Text(lines.1.isEmpty ? " " : lines.1)
                .font(.caption)
                .foregroundStyle(Theme.secondaryText)
                .lineLimit(1)
                .padding(.top, 1)
        }
        .contentShape(.rect)
    }
}

/// The artwork of a card with its marks.
struct CardArt: View {
    let item: Metadata
    let shape: CardShape
    let source: String?
    var selecting = false
    var selected = false

    @Environment(AppStore.self) private var store

    private var kind: String { Metadata.kind(item) }
    private var unwatched: Int {
        kind == "show" || kind == "season" ? (Metadata.unwatchedLeaves(item) ?? 0) : 0
    }
    private var watched: Bool {
        PlexUI.hasWatchState(item) && unwatched == 0 && Metadata.isWatched(item)
    }

    var body: some View {
        let (w, h) = shape.imageSize
        let shapeRect = RoundedRectangle(cornerRadius: Theme.cornerRadius, style: .continuous)
        Color.clear
            .aspectRatio(shape.aspect, contentMode: .fit)
            .overlay {
                Artwork(
                    source, width: w, height: h, server: Metadata.serverId(item),
                    blur: store.spoilers && kind == "episode" && !Metadata.isWatched(item), fallback: Metadata.title(item))
            }
            .overlay(alignment: .topTrailing) {
                if unwatched > 0 {
                    Text("\(unwatched)")
                        .font(.caption2.weight(.bold).monospacedDigit())
                        .padding(.horizontal, 6)
                        .frame(minWidth: 20, minHeight: 20)
                        .background(.ultraThinMaterial, in: .capsule)
                        .padding(6)
                } else if watched {
                    Image(systemName: "checkmark")
                        .font(.system(size: 9, weight: .heavy))
                        .foregroundStyle(store.accent)
                        .frame(width: 20, height: 20)
                        .background(.ultraThinMaterial, in: .circle)
                        .padding(6)
                }
            }
            .overlay(alignment: .bottom) {
                if let p = Metadata.progress(item) {
                    ProgressStrip(value: p)
                }
            }
            .overlay(alignment: .topLeading) {
                if selecting {
                    Image(systemName: selected ? "checkmark.circle.fill" : "circle")
                        .font(.title3)
                        .symbolRenderingMode(.palette)
                        .foregroundStyle(selected ? Color.black : .white, selected ? store.accent : .black.opacity(0.35))
                        .padding(6)
                        .shadow(radius: 2)
                }
            }
            .clipShape(shapeRect)
            .overlay {
                shapeRect.strokeBorder(selecting && selected ? store.accent : Theme.hairline, lineWidth: selecting && selected ? 2.5 : 1)
            }
    }
}

/// The resume bar at the bottom of artwork.
struct ProgressStrip: View {
    var value: Double
    var height: CGFloat = 3.5
    @Environment(AppStore.self) private var store

    var body: some View {
        GeometryReader { proxy in
            ZStack(alignment: .leading) {
                Rectangle().fill(.black.opacity(0.55))
                Rectangle().fill(store.accent).frame(width: proxy.size.width * value)
            }
        }
        .frame(height: height)
    }
}

/// Cards dim and settle a little while pressed.
struct CardButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .scaleEffect(configuration.isPressed ? 0.97 : 1)
            .opacity(configuration.isPressed ? 0.85 : 1)
            .animation(.spring(duration: 0.25), value: configuration.isPressed)
    }
}

/// Marks a view as the source of a zoom transition to `id`.
struct ZoomSource: ViewModifier {
    let id: Route
    let namespace: Namespace.ID?

    func body(content: Content) -> some View {
        if let namespace {
            content.matchedTransitionSource(id: id, in: namespace)
        } else {
            content
        }
    }
}

#Preview("Cards") {
    let store = AppStore.preview()
    NavigationStack {
        ScrollView(.horizontal) {
            HStack(alignment: .top, spacing: 14) {
                Card(Fixtures.movie(progress: 0.4)).frame(width: 130)
                Card(Fixtures.show()).frame(width: 130)
                Card(Fixtures.episode(watched: true), useArt: true).frame(width: 230)
                Card(Fixtures.album()).frame(width: 150)
                Card(Fixtures.movie("2", "Selected"), selecting: true, selected: true).frame(width: 130)
            }
            .padding()
        }
    }
    .environment(store)
    .preferredColorScheme(.dark)
}
