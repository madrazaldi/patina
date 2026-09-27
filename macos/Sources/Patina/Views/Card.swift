import SwiftUI

/// Poster / thumbnail card for any item. Click opens it, the context menu has
/// every item action, and hovering shows a play button.
struct Card: View {
    let item: Metadata
    var shape: CardShape? = nil
    /// Continue-watching style: backdrop art, show name as the title.
    var useArt = false
    /// Width when shown in a row; grids size cards themselves.
    var width: CGFloat? = nil
    /// When set, a click toggles selection instead of opening.
    var selection: Binding<Bool>? = nil

    @Environment(AppModel.self) private var app
    @State private var hovering = false

    private var resolvedShape: CardShape { shape ?? item.shape }

    private var source: ImageSource? {
        let (w, h): (Int, Int) = switch resolvedShape {
        case .poster: (300, 450)
        case .landscape: (560, 315)
        case .square: (360, 360)
        }
        let path: String? = switch resolvedShape {
        case .landscape where useArt && item.kind != "episode": item.backdropPath ?? item.thumb
        case .landscape: item.thumb ?? item.backdropPath
        default: item.posterPath
        }
        return path.map { .plex($0, width: w, height: h) }
    }

    private var lines: (String, String) {
        item.kind == "episode" && useArt
            ? (item.grandparentTitle ?? "", "\(item.episodeCode) · \(item.name)")
            : (item.name, item.subtitle)
    }

    private var unwatched: Int {
        item.kind == "show" || item.kind == "season" ? (item.unwatchedLeaves ?? 0) : 0
    }

    private var spoiler: Bool { app.spoilers && item.kind == "episode" && !item.isWatched }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            art
            VStack(alignment: .leading, spacing: 1) {
                Text(lines.0).font(.system(size: 12, weight: .medium)).lineLimit(1)
                if !lines.1.isEmpty {
                    Text(lines.1).font(Theme.caption).foregroundStyle(.secondary).lineLimit(1)
                }
            }
        }
        .frame(width: width)
        .contentShape(Rectangle())
        .onTapGesture { open() }
        .onHover { hovering = $0 }
        .contextMenu { ItemMenu(item: item) }
        .help(item.fullTitle)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        .accessibilityAction { open() }
    }

    private var art: some View {
        Color.clear
            .aspectRatio(1 / resolvedShape.ratio, contentMode: .fit)
            .overlay {
                Artwork(source, fallback: item.name)
                    .blur(radius: spoiler ? 14 : 0)
            }
            .overlay(alignment: .topTrailing) { badge.padding(6) }
            .overlay(alignment: .bottom) {
                if let p = item.progress { ProgressLine(value: p) }
            }
            .overlay {
                if hovering && selection == nil && item.canPlay {
                    ZStack {
                        Color.black.opacity(0.25)
                        Button { Actions.play(item) } label: {
                            Image(systemName: "play.fill")
                                .font(.system(size: 15))
                                .foregroundStyle(.black)
                                .frame(width: 38, height: 38)
                                .background(Circle().fill(.white.opacity(0.92)))
                        }
                        .buttonStyle(.plain)
                        .help(item.progress != nil ? "Resume" : "Play")
                    }
                }
            }
            .overlay(alignment: .topLeading) {
                if let selection {
                    Image(systemName: selection.wrappedValue ? "checkmark.circle.fill" : "circle")
                        .font(.system(size: 17))
                        .foregroundStyle(selection.wrappedValue ? app.accent : .white)
                        .shadow(radius: 2)
                        .padding(6)
                }
            }
            .clipShape(RoundedRectangle(cornerRadius: Theme.radius))
            .overlay(RoundedRectangle(cornerRadius: Theme.radius).strokeBorder(Theme.hairline))
    }

    @ViewBuilder private var badge: some View {
        if unwatched > 0 {
            Text("\(unwatched)")
                .font(.system(size: 10, weight: .semibold).monospacedDigit())
                .foregroundStyle(app.onAccent)
                .padding(.horizontal, 5).padding(.vertical, 1)
                .background(Capsule().fill(app.accent))
        } else if item.hasWatchState && item.isWatched {
            Image(systemName: "checkmark")
                .font(.system(size: 9, weight: .bold))
                .foregroundStyle(app.onAccent)
                .frame(width: 17, height: 17)
                .background(Circle().fill(app.accent))
        }
    }

    private func open() {
        if let selection { selection.wrappedValue.toggle() } else { app.open(item) }
    }
}

/// A thin progress bar along the bottom of artwork.
struct ProgressLine: View {
    let value: Double
    @Environment(AppModel.self) private var app

    var body: some View {
        GeometryReader { g in
            ZStack(alignment: .leading) {
                Rectangle().fill(.black.opacity(0.5))
                Rectangle().fill(app.accent).frame(width: g.size.width * value)
            }
        }
        .frame(height: 3)
    }
}

/// A titled horizontal row of cards, as on Home and detail pages.
struct HubRow: View {
    let title: String
    let items: [Metadata]
    /// "See all" destination, if the hub has more.
    var more: Route? = nil
    var useArt = false

    @Environment(AppModel.self) private var app

    private var shape: CardShape {
        if useArt { return .landscape }
        return items.first?.shape ?? .poster
    }

    private var cardWidth: CGFloat {
        switch shape {
        case .landscape: app.posterWidth * 1.75
        case .square: app.posterWidth * 1.05
        case .poster: app.posterWidth
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .firstTextBaseline) {
                Text(title).font(Theme.sectionTitle)
                Spacer()
                if let more {
                    Button("See all") { app.open(more) }
                        .buttonStyle(.link)
                        .font(Theme.caption)
                        .foregroundStyle(.secondary)
                }
            }
            .padding(.horizontal, Theme.gutter)
            ScrollView(.horizontal, showsIndicators: false) {
                LazyHStack(alignment: .top, spacing: 14) {
                    ForEach(Array(items.enumerated()), id: \.offset) { _, item in
                        Card(item: item, shape: useArt ? .landscape : nil, useArt: useArt, width: cardWidth)
                    }
                }
                .padding(.horizontal, Theme.gutter)
            }
        }
    }
}

extension HubRow {
    /// A server hub; "See all" opens its key.
    init(hub: Hub, useArt: Bool = false) {
        self.title = hub.title ?? ""
        self.items = hub.metadata
        self.more = hub.more ? hub.key.map { Route.browse(path: $0, title: hub.title ?? "") } : nil
        self.useArt = useArt
    }
}

/// A responsive grid of cards.
struct CardGrid<Footer: View>: View {
    let items: [Metadata]
    var shape: CardShape? = nil
    /// Called when a card appears, e.g. to load the next page.
    var onAppear: ((Int) -> Void)? = nil
    var selected: Binding<Set<String>>? = nil
    @ViewBuilder var footer: () -> Footer

    @Environment(AppModel.self) private var app

    private var minWidth: CGFloat {
        switch shape ?? items.first?.shape ?? .poster {
        case .landscape: app.posterWidth * 1.75
        case .square: app.posterWidth * 1.05
        case .poster: app.posterWidth
        }
    }

    var body: some View {
        LazyVGrid(columns: [GridItem(.adaptive(minimum: minWidth, maximum: minWidth * 1.4), spacing: 18, alignment: .top)], alignment: .leading, spacing: 22) {
            ForEach(Array(items.enumerated()), id: \.offset) { i, item in
                Card(item: item, shape: shape, selection: selectionBinding(item))
                    .onAppear { onAppear?(i) }
            }
        }
        .padding(.horizontal, Theme.gutter)
        footer()
    }

    private func selectionBinding(_ item: Metadata) -> Binding<Bool>? {
        guard let selected else { return nil }
        return Binding(
            get: { selected.wrappedValue.contains(item.id) },
            set: { on in
                if on { selected.wrappedValue.insert(item.id) } else { selected.wrappedValue.remove(item.id) }
            }
        )
    }
}

extension CardGrid where Footer == EmptyView {
    init(items: [Metadata], shape: CardShape? = nil, onAppear: ((Int) -> Void)? = nil, selected: Binding<Set<String>>? = nil) {
        self.init(items: items, shape: shape, onAppear: onAppear, selected: selected, footer: { EmptyView() })
    }
}
