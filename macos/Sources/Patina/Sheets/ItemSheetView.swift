import SwiftUI

/// The sheet for `app.sheet`: one native dialog per item action.
struct ItemSheetView: View {
    let sheet: ItemSheet

    var body: some View {
        switch sheet {
        case .playlist(let m): PlaylistSheet(item: m)
        case .collection(let m): CollectionSheet(item: m)
        case .edit(let m): EditSheet(item: m)
        case .artwork(let m, let kind): ArtworkSheet(item: m, kind: kind)
        case .match(let m): MatchSheet(item: m)
        case .subtitles(let m): SubtitleSheet(item: m)
        case .tracks(let m): TrackSheet(item: m)
        }
    }
}

/// Shared layout for item sheets: a serif title with the item beneath, the
/// content, and a footer of buttons separated by a hairline.
struct SheetFrame<Content: View, Footer: View>: View {
    let title: String
    var subtitle: String? = nil
    var width: CGFloat = 480
    var height: CGFloat? = nil
    @ViewBuilder var content: () -> Content
    @ViewBuilder var footer: () -> Footer

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 3) {
                Text(title).font(Theme.display(20))
                if let subtitle, !subtitle.isEmpty {
                    Text(subtitle).font(.callout).foregroundStyle(.secondary).lineLimit(2)
                }
            }
            .padding(.horizontal, 20)
            .padding(.top, 18)
            .padding(.bottom, 12)
            content()
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
            Hairline()
            HStack(spacing: 8) { footer() }
                .padding(.horizontal, 20)
                .padding(.vertical, 12)
        }
        .frame(width: width, height: height)
    }
}

/// Small centered states inside sheets.
struct SheetMessage: View {
    let text: String
    var spinner = false
    var error = false

    var body: some View {
        VStack(spacing: 8) {
            if spinner { ProgressView().controlSize(.small) }
            if !text.isEmpty {
                Text(text)
                    .font(.callout)
                    .foregroundStyle(error ? AnyShapeStyle(.red) : AnyShapeStyle(.secondary))
                    .multilineTextAlignment(.center)
                    .textSelection(.enabled)
            }
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// A list row with a thumbnail, two lines of text and a trailing accessory.
struct SheetRow<Accessory: View>: View {
    let thumb: String?
    var shape: CardShape = .square
    let title: String
    var subtitle: String? = nil
    var thumbWidth: CGFloat = 36
    @ViewBuilder var accessory: () -> Accessory

    var body: some View {
        HStack(spacing: 10) {
            let h = thumbWidth * shape.ratio
            Artwork(thumb, width: Int(thumbWidth * 2), height: Int(h * 2))
                .frame(width: thumbWidth, height: h)
                .clipShape(RoundedRectangle(cornerRadius: Theme.radius))
            VStack(alignment: .leading, spacing: 2) {
                Text(title).lineLimit(1)
                if let subtitle, !subtitle.isEmpty {
                    Text(subtitle).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
            }
            Spacer(minLength: 8)
            accessory()
        }
        .padding(.vertical, 3)
        .contentShape(Rectangle())
    }
}
