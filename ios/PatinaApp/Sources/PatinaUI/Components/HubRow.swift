// Port of web/src/components/HubRow.svelte and Row.svelte: a titled, horizontally
// scrolling row of cards that snaps to them, with "See all" when the server has more.

import PatinaCore
import PlexKit
import SwiftUI

/// The width of a card in a row, by shape and size class.
func cardWidth(_ shape: CardShape, _ sizeClass: UserInterfaceSizeClass?) -> CGFloat {
    let regular = sizeClass == .regular
    switch shape {
    case .poster: return regular ? 152 : 118
    case .landscape: return regular ? 290 : 232
    case .square: return regular ? 168 : 136
    }
}

/// A row heading: the title, and an optional "See all".
struct SectionHeader<Trailing: View>: View {
    var title: String
    var more: Route?
    @ViewBuilder var trailing: Trailing
    @Environment(\.horizontalSizeClass) private var sizeClass

    init(_ title: String, more: Route? = nil, @ViewBuilder trailing: () -> Trailing = { EmptyView() }) {
        self.title = title
        self.more = more
        self.trailing = trailing()
    }

    var body: some View {
        HStack(alignment: .firstTextBaseline) {
            if let more {
                NavigationLink(value: more) {
                    HStack(spacing: 4) {
                        Text(title)
                        Image(systemName: "chevron.right")
                            .font(.subheadline.weight(.bold))
                            .foregroundStyle(Theme.tertiaryText)
                    }
                }
                .buttonStyle(.plain)
                .accessibilityHint("See all")
            } else {
                Text(title)
            }
            Spacer()
            trailing
        }
        .font(.title3.weight(.bold))
        .padding(.horizontal, Theme.margin(sizeClass))
    }
}

/// A row of cards.
struct CardRow: View {
    var title: String
    var items: [Metadata]
    var shape: CardShape?
    var useArt = false
    var more: Route?

    @Environment(\.horizontalSizeClass) private var sizeClass

    var body: some View {
        let rowShape = shape ?? PlexUI.commonShape(items)
        if !items.isEmpty {
            VStack(alignment: .leading, spacing: 12) {
                SectionHeader(title, more: more)
                ScrollView(.horizontal) {
                    LazyHStack(alignment: .top, spacing: sizeClass == .regular ? 18 : 12) {
                        ForEach(Array(items.enumerated()), id: \.offset) { _, item in
                            Card(item, shape: rowShape, useArt: useArt)
                                .frame(width: cardWidth(rowShape, sizeClass))
                        }
                    }
                    .scrollTargetLayout()
                }
                .scrollIndicators(.hidden)
                .scrollTargetBehavior(.viewAligned)
                .contentMargins(.horizontal, Theme.margin(sizeClass), for: .scrollContent)
            }
        }
    }
}

/// A Plex hub (Continue Watching, Recently Added, ...).
struct HubRow: View {
    var hub: Hub
    /// Continue Watching as a compact list (Customize Home).
    var compact = false

    @Environment(AppStore.self) private var store
    @Environment(\.horizontalSizeClass) private var sizeClass

    private var ident: String { (hub.hubIdentifier ?? "").lowercased() }
    private var continueStyle: Bool { ["continue", "ondeck", "inprogress"].contains { ident.contains($0) } }
    private var landscape: Bool { continueStyle || hub.type == "clip" || hub.type == "episode" }
    private var more: Route? {
        guard hub.more, let key = hub.key else { return nil }
        return .browse(path: key, title: hub.title ?? "")
    }

    var body: some View {
        if compact && continueStyle {
            VStack(alignment: .leading, spacing: 12) {
                SectionHeader(hub.title ?? "")
                VStack(spacing: 0) {
                    ForEach(Array(hub.metadata.enumerated()), id: \.offset) { i, item in
                        if i > 0 { Divider().overlay(Theme.hairline) }
                        Button {
                            store.perform { try await PendingCommands.play(store.app, item: item) }
                        } label: {
                            HStack(spacing: 12) {
                                Text(item.grandparentTitle.map { "\($0) · \(Metadata.title(item))" } ?? Metadata.title(item))
                                    .font(.subheadline.weight(.medium))
                                    .lineLimit(1)
                                Spacer()
                                if let left = Format.remaining(
                                    durationMs: item.duration.map(Int64.init), viewOffsetMs: item.viewOffset.map(Int64.init))
                                {
                                    Text(left).font(.caption.monospacedDigit()).foregroundStyle(Theme.secondaryText)
                                }
                                Image(systemName: "play.fill").font(.caption).foregroundStyle(store.accent)
                            }
                            .padding(.horizontal, 14)
                            .padding(.vertical, 12)
                            .contentShape(.rect)
                        }
                        .buttonStyle(.plain)
                        .contextMenu { ItemMenu(item: item) }
                    }
                }
                .panel()
                .padding(.horizontal, Theme.margin(sizeClass))
            }
        } else {
            CardRow(
                title: hub.title ?? "", items: hub.metadata, shape: landscape ? .landscape : nil, useArt: continueStyle,
                more: more)
        }
    }
}

#Preview("Hub row") {
    NavigationStack {
        ScrollView {
            HubRow(hub: Fixtures.hub)
        }
    }
    .environment(AppStore.preview())
    .preferredColorScheme(.dark)
}
