import SwiftUI

/// The bar along the bottom of the window while mpv plays: what's playing,
/// transport, position, status, and the queue and lyrics popovers.
struct NowPlayingBar: View {
    @Environment(AppModel.self) private var app

    var body: some View {
        if let np = app.nowPlaying {
            VStack(spacing: 0) {
                if np.ending, !np.stopAfter, let next = upNext(np) {
                    Hairline()
                    UpNextStrip(next: next)
                }
                Hairline()
                HStack(spacing: 18) {
                    NowPlayingInfo(np: np)
                        .frame(minWidth: 180, maxWidth: .infinity, alignment: .leading)
                    NowPlayingTransport(np: np)
                        .frame(minWidth: 320, maxWidth: 520)
                    NowPlayingTools(np: np)
                        .frame(minWidth: 180, maxWidth: .infinity, alignment: .trailing)
                }
                .padding(.horizontal, 16)
                .frame(height: 62)
            }
            .background(.bar)
        }
    }

    private func upNext(_ np: NowPlaying) -> Metadata? {
        guard let q = app.queue, let i = q.items.firstIndex(where: { $0.uid == np.uid }), i + 1 < q.items.count else { return nil }
        return q.items[i + 1].item
    }
}

/// Send a transport command, toasting failures.
@MainActor
func sendControl(_ c: Control) {
    Task { do { try await API.control(c) } catch { AppModel.shared.error(error) } }
}

/// Edit the running queue, toasting failures.
@MainActor
func sendQueue(_ c: QueueCommand) {
    Task { do { try await API.queue(c) } catch { AppModel.shared.error(error) } }
}

/// Artwork for what's playing: offline plays use the download's saved poster.
struct NowPlayingArt: View {
    let np: NowPlaying
    var size: CGFloat = 40
    @Environment(AppModel.self) private var app

    var body: some View {
        Artwork(source, fallback: nil)
            .frame(width: size, height: size)
            .clipShape(RoundedRectangle(cornerRadius: Theme.radius))
            .overlay(RoundedRectangle(cornerRadius: Theme.radius).strokeBorder(Theme.hairline))
    }

    private var source: ImageSource? {
        let item = np.item
        if np.local {
            let d = app.downloads.first { $0.item.ratingKey == item.ratingKey && $0.poster != nil }
            return d.map { .download($0.id) }
        }
        let path = item.kind == "episode" ? item.thumb : item.posterPath
        let px = Int(size * 2)
        return path.map { .plex($0, width: px, height: px) }
    }
}

/// Artwork and title lines; clicking opens the item.
struct NowPlayingInfo: View {
    let np: NowPlaying
    @Environment(AppModel.self) private var app

    var body: some View {
        let (title, sub) = np.item.playingLines
        Button {
            if !np.local || app.connected { app.open(np.item) }
        } label: {
            HStack(spacing: 10) {
                NowPlayingArt(np: np)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).font(.system(size: 13, weight: .medium)).lineLimit(1)
                    if !sub.isEmpty {
                        Text(sub).font(.system(size: 11)).foregroundStyle(.secondary).lineLimit(1)
                    }
                }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help("Show \(np.item.name)")
        .contextMenu { ItemMenu(item: np.item) }
    }
}

/// "Up next" while the current item is ending.
struct UpNextStrip: View {
    let next: Metadata

    var body: some View {
        let (title, sub) = next.playingLines
        HStack(spacing: 10) {
            Artwork(next.posterPath ?? next.thumb, width: 56, height: 56)
                .frame(width: 24, height: 24)
                .clipShape(RoundedRectangle(cornerRadius: 3))
            Eyebrow("Up next")
            Text(title).font(.callout).lineLimit(1)
            if !sub.isEmpty { Text(sub).font(.callout).foregroundStyle(.secondary).lineLimit(1) }
            Spacer()
            Button("Stop After This") { sendQueue(.stopAfter(true)) }
                .buttonStyle(.borderless)
            Button("Play Now") { sendControl(.next) }
                .controlSize(.small)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 6)
        .transition(.move(edge: .bottom).combined(with: .opacity))
    }
}
