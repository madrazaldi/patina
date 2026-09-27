import SwiftUI

/// Tagline, summary, stream pickers and the facts list under the header.
struct ItemBody: View {
    let item: Metadata
    @Binding var mediaIndex: Int
    @Environment(AppModel.self) private var app
    @State private var expanded = false
    @State private var revealed = false

    var body: some View {
        let kind = item.kind
        let summary = item.summary ?? ""
        let spoiler = app.spoilers && kind == "episode" && !item.isWatched && !revealed
        let facts = ItemFacts(item: item, owned: app.owned)
        VStack(alignment: .leading, spacing: 0) {
            if let t = item.tagline, !t.isEmpty {
                Text(t).font(Theme.display(17).italic()).foregroundStyle(.secondary).padding(.bottom, 10)
            }
            if !summary.isEmpty {
                let long = summary.count > 420
                Text(summary)
                    .font(.system(size: 14))
                    .lineSpacing(4)
                    .lineLimit(long && !expanded ? 5 : nil)
                    .textSelection(.enabled)
                    .blur(radius: spoiler ? 5 : 0)
                    .frame(maxWidth: 640, alignment: .leading)
                    .contentShape(Rectangle())
                    .onTapGesture {
                        if spoiler { revealed = true } else if long { withAnimation(.easeOut(duration: 0.15)) { expanded.toggle() } }
                    }
                    .help(spoiler ? "Click to reveal the summary" : long ? (expanded ? "Show less" : "Show more") : "")
                if long && !spoiler {
                    Button(expanded ? "Less" : "More") { withAnimation(.easeOut(duration: 0.15)) { expanded.toggle() } }
                        .buttonStyle(.link)
                        .font(Theme.caption)
                        .padding(.top, 4)
                }
            }
            if ["movie", "episode", "clip", "track"].contains(kind) && !item.media.isEmpty {
                ItemStreamPickers(item: item, mediaIndex: $mediaIndex).padding(.top, 22)
            }
            if !facts.tags.isEmpty || !facts.plain.isEmpty {
                Hairline().padding(.top, 26).padding(.bottom, 16)
                factsGrid(facts)
            }
        }
        .frame(maxWidth: 880, alignment: .leading)
        .onChange(of: item.ratingKey) { _, _ in expanded = false; revealed = false }
    }

    private func factsGrid(_ facts: ItemFacts) -> some View {
        Grid(alignment: .leadingFirstTextBaseline, horizontalSpacing: 20, verticalSpacing: 7) {
            ForEach(facts.tags, id: \.0) { label, tags in
                GridRow {
                    factLabel(label)
                    ItemFlow(spacing: 0) {
                        ForEach(Array(tags.prefix(8).enumerated()), id: \.offset) { i, tag in
                            tagLink(tag, last: i == min(tags.count, 8) - 1)
                        }
                    }
                }
            }
            ForEach(facts.plain, id: \.0) { label, value in
                GridRow {
                    factLabel(label)
                    Text(value).textSelection(.enabled).foregroundStyle(.primary)
                }
            }
        }
        .font(.system(size: 12.5))
    }

    private func factLabel(_ s: String) -> some View {
        Text(s).foregroundStyle(.secondary).gridColumnAlignment(.leading)
    }

    @ViewBuilder private func tagLink(_ tag: Tag, last: Bool) -> some View {
        let name = (tag.tag ?? "") + (last ? "" : ", ")
        if let route = ItemFacts.route(item, tag) {
            Button(name) { app.open(route) }.buttonStyle(ItemTagLinkStyle())
        } else {
            Text(name)
        }
    }
}

/// Plain text that underlines on hover.
struct ItemTagLinkStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        ItemHoverUnderline(label: configuration.label, pressed: configuration.isPressed)
    }
}

private struct ItemHoverUnderline<Label: View>: View {
    let label: Label
    let pressed: Bool
    @State private var hovering = false
    var body: some View {
        label
            .underline(hovering)
            .opacity(pressed ? 0.6 : 1)
            .contentShape(Rectangle())
            .onHover { hovering = $0 }
    }
}

/// The key/value facts of an item.
struct ItemFacts {
    var tags: [(String, [Tag])] = []
    var plain: [(String, String)] = []

    init(item: Metadata, owned: Bool) {
        tags = [
            ("Directed by", item.directors), ("Written by", item.writers), ("Genres", item.genres),
            ("Collections", item.collections), ("Moods", item.moods), ("Styles", item.styles),
            ("Country", item.countries), ("Labels", item.labels),
        ].filter { !$0.1.isEmpty }
        if let s = item.studio { plain.append(("Studio", s)) }
        if let m = item.media.first {
            let p = m.parts.first
            let bits = [
                m.container?.uppercased(),
                m.width.flatMap { w in m.height.map { "\(w)×\($0)" } },
                m.bitrate.map(Fmt.bitrate),
                p?.size.map(Fmt.bytes),
            ].compactMap { $0 }
            if !bits.isEmpty { plain.append(("File", bits.joined(separator: " · "))) }
            if let v = m.videoCodec {
                plain.append(("Video", [v.uppercased(), m.videoResolution.map { Int($0) != nil ? "\($0)p" : $0.uppercased() }].compactMap { $0 }.joined(separator: " ")))
            }
            if let a = p?.selectedAudio {
                plain.append(("Audio", "\((a.codec ?? "").uppercased()) \(a.channels.map(Fmt.channels) ?? "")".trimmingCharacters(in: .whitespaces)))
            }
            if owned, let f = p?.file { plain.append(("Path", f)) }
        }
        if let a = item.addedAt { plain.append(("Added", Fmt.date(a))) }
        if let v = item.lastViewedAt {
            let n = item.viewCount ?? 0
            plain.append(("Last watched", "\(Fmt.ago(v)) · \(Fmt.plural(n, "play"))"))
        }
    }

    /// A tag's listing in the item's library (genre, director, actor, ...).
    static func route(_ item: Metadata, _ tag: Tag) -> Route? {
        guard let section = item.librarySectionID, let filter = tag.filter else { return nil }
        return .browse(path: "/library/sections/\(section)/all?\(filter)", title: tag.tag ?? "")
    }
}

/// Lays children out left to right, wrapping onto new lines.
struct ItemFlow: Layout {
    var spacing: CGFloat = 8
    var lineSpacing: CGFloat = 4

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let rows = arrange(proposal.width ?? .infinity, subviews)
        return CGSize(width: rows.map(\.width).max() ?? 0, height: rows.last.map { $0.y + $0.height } ?? 0)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        for row in arrange(bounds.width, subviews) {
            for (i, x) in zip(row.items, row.xs) {
                subviews[i].place(at: CGPoint(x: bounds.minX + x, y: bounds.minY + row.y), proposal: .unspecified)
            }
        }
    }

    private struct Row { var items: [Int] = []; var xs: [CGFloat] = []; var width: CGFloat = 0; var y: CGFloat = 0; var height: CGFloat = 0 }

    private func arrange(_ maxWidth: CGFloat, _ subviews: Subviews) -> [Row] {
        var rows: [Row] = [Row()]
        for (i, v) in subviews.enumerated() {
            let size = v.sizeThatFits(.unspecified)
            var row = rows.removeLast()
            if !row.items.isEmpty && row.width + spacing + size.width > maxWidth {
                let next = Row(y: row.y + row.height + lineSpacing)
                rows.append(row)
                row = next
            }
            let x = row.items.isEmpty ? 0 : row.width + spacing
            row.items.append(i)
            row.xs.append(x)
            row.width = x + size.width
            row.height = max(row.height, size.height)
            rows.append(row)
        }
        return rows
    }
}
