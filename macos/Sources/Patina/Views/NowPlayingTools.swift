import SwiftUI

/// Status and the right-hand buttons of the now-playing bar.
struct NowPlayingTools: View {
    let np: NowPlaying
    @Environment(AppModel.self) private var app

    private var isTrack: Bool { np.item.kind == "track" }
    private var status: String {
        np.live ? "Live" : np.local ? "Offline" : np.transcoding ? "Transcoding" : "Direct Play"
    }

    var body: some View {
        @Bindable var app = app
        HStack(spacing: 10) {
            if np.stopAfter {
                HStack(spacing: 4) {
                    Text("Stops after this").foregroundStyle(.secondary)
                    Button("Undo") { sendQueue(.stopAfter(false)) }
                        .buttonStyle(.link)
                }
                .font(.system(size: 11))
            }
            BarBadge(text: status, accent: np.live)
                .help(np.transcoding ? "The server is converting this for playback." : "")
            if np.queueLen > 1 {
                Text("\(np.queuePos + 1) of \(np.queueLen)")
                    .font(.system(size: 11).monospacedDigit())
                    .foregroundStyle(.secondary)
            }
            if isTrack {
                toolButton("quote.bubble", "Lyrics", on: app.showLyrics) { app.showLyrics.toggle() }
                    .popover(isPresented: $app.showLyrics, arrowEdge: .top) {
                        LyricsPanel().environment(app)
                    }
            }
            if app.queue != nil {
                toolButton("list.bullet", "Queue", on: app.showQueue) { app.showQueue.toggle() }
                    .popover(isPresented: $app.showQueue, arrowEdge: .top) {
                        QueuePanel().environment(app)
                    }
            }
            toolButton("stop.fill", "Stop", on: false) { sendControl(.stop) }
        }
    }

    private func toolButton(_ symbol: String, _ help: String, on: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 12))
                .frame(width: 24, height: 22)
                .background(on ? Color.primary.opacity(0.08) : .clear, in: RoundedRectangle(cornerRadius: 4))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .foregroundStyle(on ? AnyShapeStyle(app.accent) : AnyShapeStyle(.secondary))
        .help(help)
    }
}

/// A small outlined label: "Direct Play", "Live", ...
struct BarBadge: View {
    let text: String
    var accent = false
    @Environment(AppModel.self) private var app

    var body: some View {
        Text(text)
            .font(.system(size: 10, weight: .medium))
            .foregroundStyle(accent ? AnyShapeStyle(app.accent) : AnyShapeStyle(.secondary))
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .overlay(RoundedRectangle(cornerRadius: 3).strokeBorder(accent ? app.accent.opacity(0.6) : Color.primary.opacity(0.15)))
    }
}
