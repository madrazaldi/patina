import SwiftUI

/// Lyrics for the playing track, in a popover over the now-playing bar.
/// Synced lines follow playback; clicking one seeks to it.
struct LyricsPanel: View {
    @Environment(AppModel.self) private var app
    @State private var phase: Phase<Lyrics?> = .loading
    /// Position anchor: position ticks arrive about once a second; interpolate between them.
    @State private var anchor = (pos: 0, at: Date(), paused: true)
    /// The reader's own scrolling holds auto-scroll off until then.
    @State private var heldUntil = Date.distantPast

    private var key: String { app.nowPlaying?.item.ratingKey ?? "" }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 6) {
                Text("Lyrics").font(.system(size: 13, weight: .semibold))
                Text(app.nowPlaying?.item.name ?? "").font(.system(size: 12)).foregroundStyle(.secondary).lineLimit(1)
                Spacer()
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
            Hairline()
            content
        }
        .frame(width: 380, height: 480)
        .task(id: key) {
            phase = .loading
            let result: Phase<Lyrics?>
            do { result = .loaded(try await API.lyrics(key)) } catch { result = .failed(error.localizedDescription) }
            // The track may have changed meanwhile; backend calls don't observe cancellation.
            if !Task.isCancelled { phase = result }
        }
        .onAppear(perform: syncAnchor)
        .onChange(of: app.nowPlaying?.positionMs) { syncAnchor() }
        .onChange(of: app.nowPlaying?.paused) { syncAnchor() }
    }

    private func syncAnchor() {
        guard let np = app.nowPlaying else { return }
        anchor = (np.positionMs, Date(), np.paused)
    }

    @ViewBuilder private var content: some View {
        switch phase {
        case .loading: SheetMessage(text: "", spinner: true)
        case .failed(let e): SheetMessage(text: e, error: true)
        case .loaded(let l):
            if let l, !l.lines.isEmpty {
                if l.synced { synced(l) } else { plain(l) }
            } else {
                SheetMessage(text: "No lyrics for this track.")
            }
        }
    }

    private func plain(_ l: Lyrics) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 6) {
                ForEach(Array(l.lines.enumerated()), id: \.offset) { _, line in
                    Text(line.text.isEmpty ? " " : line.text)
                }
            }
            .font(Theme.display(15))
            .textSelection(.enabled)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(18)
        }
    }

    private func synced(_ l: Lyrics) -> some View {
        TimelineView(.periodic(from: .now, by: 0.2)) { ctx in
            let pos = anchor.paused ? anchor.pos : anchor.pos + Int(min(max(0, ctx.date.timeIntervalSince(anchor.at)), 1.5) * 1000)
            let active = l.lines.lastIndex { ($0.timeMs ?? .max) <= pos } ?? -1
            ScrollViewReader { proxy in
                ScrollView {
                    VStack(alignment: .leading, spacing: 8) {
                        ForEach(Array(l.lines.enumerated()), id: \.offset) { i, line in
                            Button { seek(line.timeMs) } label: {
                                Text(line.text.isEmpty ? " " : line.text)
                                    .font(Theme.display(15, weight: i == active ? .semibold : .regular))
                                    .foregroundStyle(i == active ? AnyShapeStyle(.primary) : AnyShapeStyle(.secondary))
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .contentShape(Rectangle())
                            }
                            .buttonStyle(.plain)
                            .disabled(line.timeMs == nil)
                            .id(i)
                        }
                    }
                    .padding(18)
                }
                .onScrollWheel { heldUntil = Date().addingTimeInterval(4) }
                .onChange(of: active) { _, i in
                    guard i >= 0, Date() >= heldUntil else { return }
                    withAnimation(.easeOut(duration: 0.3)) { proxy.scrollTo(i, anchor: .center) }
                }
                .onAppear { if active >= 0 { proxy.scrollTo(active, anchor: .center) } }
            }
        }
    }

    private func seek(_ ms: Int?) {
        guard let ms else { return }
        heldUntil = .distantPast
        sendControl(.seekTo(Double(ms) / 1000))
    }
}

private extension View {
    /// Runs `action` when the user scrolls with a wheel or trackpad over this view.
    func onScrollWheel(_ action: @escaping () -> Void) -> some View {
        background(ScrollWheelWatcher(action: action))
    }
}

/// Watches scroll-wheel events inside the view's window area.
private struct ScrollWheelWatcher: NSViewRepresentable {
    let action: () -> Void

    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        context.coordinator.monitor = NSEvent.addLocalMonitorForEvents(matching: .scrollWheel) { [weak view] event in
            if let view, let window = view.window, event.window == window {
                let p = view.convert(event.locationInWindow, from: nil)
                if view.bounds.contains(p) { action() }
            }
            return event
        }
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {}

    static func dismantleNSView(_ nsView: NSView, coordinator: Coordinator) {
        if let m = coordinator.monitor { NSEvent.removeMonitor(m) }
    }

    func makeCoordinator() -> Coordinator { Coordinator() }
    final class Coordinator { var monitor: Any? }
}
