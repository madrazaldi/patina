import SwiftUI

/// Previous / back 10s / play-pause / forward 10s / next, above the scrubber.
struct NowPlayingTransport: View {
    let np: NowPlaying

    var body: some View {
        VStack(spacing: 5) {
            HStack(spacing: 14) {
                control("backward.end.fill", "Previous", disabled: np.queuePos == 0) { sendControl(.previous) }
                control("gobackward.10", "Back 10 Seconds", disabled: np.live) { sendControl(.seekRelative(-10)) }
                Button { sendControl(.togglePause) } label: {
                    Image(systemName: np.paused ? "play.fill" : "pause.fill")
                        .font(.system(size: 17))
                        .frame(width: 26, height: 22)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help(np.paused ? "Play" : "Pause")
                control("goforward.10", "Forward 10 Seconds", disabled: np.live) { sendControl(.seekRelative(10)) }
                control("forward.end.fill", "Next", disabled: np.queuePos + 1 >= np.queueLen) { sendControl(.next) }
            }
            Scrubber(np: np)
        }
    }

    private func control(_ symbol: String, _ help: String, disabled: Bool = false, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 12))
                .frame(width: 20, height: 20)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .foregroundStyle(disabled ? .tertiary : .secondary)
        .disabled(disabled)
        .help(help)
    }
}

/// Elapsed, a thin seek line, and remaining. Dragging previews the position;
/// the seek is sent on release and held until playback catches up.
struct Scrubber: View {
    let np: NowPlaying
    @Environment(AppModel.self) private var app
    @State private var scrub: Double?
    @State private var dragging = false
    @State private var hover = false

    private var seekable: Bool { !np.live && np.durationMs > 0 }
    private var position: Int {
        if let scrub { return Int(scrub * Double(np.durationMs)) }
        return np.positionMs
    }

    var body: some View {
        HStack(spacing: 8) {
            Text(Fmt.clock(position))
                .frame(width: 52, alignment: .trailing)
            if seekable {
                line
            } else {
                Rectangle().fill(Theme.hairline).frame(height: 1)
            }
            Text(np.live ? "Live" : "-" + Fmt.clock(max(0, np.durationMs - position)))
                .frame(width: 52, alignment: .leading)
        }
        .font(.system(size: 10).monospacedDigit())
        .foregroundStyle(.secondary)
        .onChange(of: np.positionMs) { _, pos in
            // Let go of the held position once playback reaches it.
            if !dragging, let s = scrub, abs(Double(pos) - s * Double(np.durationMs)) < 2500 { scrub = nil }
        }
    }

    private var line: some View {
        GeometryReader { geo in
            let fraction = min(1, max(0, Double(position) / Double(max(1, np.durationMs))))
            let tall = hover || dragging
            ZStack(alignment: .leading) {
                Capsule().fill(Color.primary.opacity(0.12))
                Capsule().fill(app.accent).frame(width: geo.size.width * fraction)
                if tall {
                    Circle().fill(app.accent)
                        .frame(width: 9, height: 9)
                        .offset(x: geo.size.width * fraction - 4.5)
                }
            }
            .frame(height: tall ? 4 : 3)
            .frame(maxHeight: .infinity)
            .contentShape(Rectangle())
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { v in
                        dragging = true
                        scrub = min(1, max(0, v.location.x / max(1, geo.size.width)))
                    }
                    .onEnded { v in
                        let f = min(1, max(0, v.location.x / max(1, geo.size.width)))
                        scrub = f
                        dragging = false
                        sendControl(.seekTo(f * Double(np.durationMs) / 1000))
                        Task {
                            try? await Task.sleep(for: .seconds(2))
                            if !dragging { scrub = nil }
                        }
                    }
            )
            .onHover { hover = $0 }
        }
        .frame(height: 12)
        .animation(.easeOut(duration: 0.12), value: hover || dragging)
    }
}
