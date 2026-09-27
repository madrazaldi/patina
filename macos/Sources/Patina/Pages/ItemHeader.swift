import SwiftUI

/// Poster, breadcrumbs, title, meta line and the action row of a detail page.
struct ItemHeader: View {
    let item: Metadata
    @Binding var mediaIndex: Int
    @Environment(AppModel.self) private var app

    private var shape: CardShape {
        let k = item.kind
        if k == "episode" || k == "clip" { return .landscape }
        if ["album", "artist", "playlist", "photoalbum", "track"].contains(k) { return .square }
        return .poster
    }

    var body: some View {
        HStack(alignment: .bottom, spacing: 32) {
            poster
            VStack(alignment: .leading, spacing: 0) {
                crumbs
                Text(item.name)
                    .font(Theme.display(40))
                    .lineLimit(3)
                    .textSelection(.enabled)
                if let o = item.originalTitle, o != item.title {
                    Text(o).font(Theme.display(17)).foregroundStyle(.secondary).padding(.top, 4)
                }
                meta.padding(.top, 12)
                ItemActions(item: item, mediaIndex: mediaIndex).padding(.top, 20)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private var poster: some View {
        let (w, source): (CGFloat, ImageSource?) = switch shape {
        case .landscape: (340, item.thumb.map { .plex($0, width: 840, height: 472) })
        case .square: (220, (item.thumb ?? item.parentThumb).map { .plex($0, width: 600, height: 600) })
        case .poster: (200, item.thumb.map { .plex($0, width: 500, height: 750) })
        }
        let spoiler = app.spoilers && item.kind == "episode" && !item.isWatched
        return Color.clear
            .frame(width: w, height: w * shape.ratio)
            .overlay { Artwork(source, fallback: item.name).blur(radius: spoiler ? 16 : 0) }
            .overlay(alignment: .bottom) { if let p = item.progress { ProgressLine(value: p) } }
            .clipShape(RoundedRectangle(cornerRadius: Theme.radius))
            .overlay(RoundedRectangle(cornerRadius: Theme.radius).strokeBorder(Theme.hairline))
            .shadow(color: .black.opacity(0.18), radius: 10, y: 4)
    }

    /// Show / season or artist / album links above the title.
    @ViewBuilder private var crumbs: some View {
        let k = item.kind
        let links: [(String, String)] = {
            var out: [(String, String)] = []
            if (k == "episode" || k == "track"), let g = item.grandparentRatingKey { out.append((item.grandparentTitle ?? "", g)) }
            if ["episode", "track", "season", "album"].contains(k), let p = item.parentRatingKey { out.append((item.parentTitle ?? "", p)) }
            return out
        }()
        if !links.isEmpty {
            HStack(spacing: 6) {
                ForEach(Array(links.enumerated()), id: \.offset) { i, link in
                    if i > 0 { Image(systemName: "chevron.right").font(.system(size: 9)).foregroundStyle(.tertiary) }
                    Button(link.0) { app.open(.item(link.1)) }
                        .buttonStyle(.link)
                        .foregroundStyle(.secondary)
                }
            }
            .font(.system(size: 12.5))
            .padding(.bottom, 8)
        }
    }

    private var meta: some View {
        let parts = ratings + metaLine
        let media = item.media.indices.contains(mediaIndex) ? item.media[mediaIndex] : item.media.first
        return HStack(spacing: 8) {
            if !parts.isEmpty {
                Text(parts.joined(separator: "  ·  ")).foregroundStyle(.secondary)
            }
            if let r = item.contentRating { ItemBadge(text: r) }
            if let label = media?.label, !label.isEmpty { ItemBadge(text: label) }
        }
        .font(.system(size: 13))
        .lineLimit(1)
    }

    private var metaLine: [String] {
        let k = item.kind
        var out: [String] = []
        if k == "episode" {
            if !item.episodeCode.isEmpty { out.append(item.episodeCode) }
            if let d = item.originallyAvailableAt { out.append(Fmt.airDate(d)) }
        } else if let y = item.year { out.append(String(y)) }
        if let d = item.duration, k != "show", k != "season" { out.append(Fmt.duration(d)) }
        if k == "show", let c = item.childCount { out.append(Fmt.plural(c, "season")) }
        if k == "collection", let c = item.childCount { out.append(Fmt.plural(c, "item")) }
        if let l = item.leafCount, ["show", "season", "album"].contains(k) { out.append(Fmt.plural(l, k == "album" ? "track" : "episode")) }
        return out
    }

    /// Rotten Tomatoes scores as percentages, other sources as a 10-point score.
    private var ratings: [String] {
        let rt = { (i: String?) in i?.contains("rottentomatoes") ?? false }
        let imdb = { (i: String?) in i?.contains("imdb") ?? false }
        var out: [String] = []
        if let r = item.rating {
            out.append(rt(item.ratingImage) ? "\(Int((r * 10).rounded()))% Critics"
                : imdb(item.ratingImage) ? "IMDb \(String(format: "%.1f", r))" : "★ \(String(format: "%.1f", r))")
        }
        if let r = item.audienceRating {
            out.append(rt(item.audienceRatingImage) ? "\(Int((r * 10).rounded()))% Audience"
                : imdb(item.audienceRatingImage) ? "IMDb \(String(format: "%.1f", r))" : "★ \(String(format: "%.1f", r)) Audience")
        }
        return out
    }
}

/// A small outlined label, e.g. "PG-13" or "4K · HEVC".
struct ItemBadge: View {
    let text: String
    var body: some View {
        Text(text)
            .font(.system(size: 10.5, weight: .medium))
            .foregroundStyle(.secondary)
            .padding(.horizontal, 5).padding(.vertical, 1.5)
            .overlay(RoundedRectangle(cornerRadius: 3).strokeBorder(Color.primary.opacity(0.2)))
    }
}

/// Filled, neutral primary button: text colour on the window background colour.
struct ItemPrimaryButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 13, weight: .semibold))
            .foregroundStyle(Color(nsColor: .windowBackgroundColor))
            .padding(.horizontal, 16)
            .frame(height: 30)
            .background(RoundedRectangle(cornerRadius: 6).fill(Color.primary.opacity(configuration.isPressed ? 0.7 : 0.9)))
            .contentShape(Rectangle())
    }
}
