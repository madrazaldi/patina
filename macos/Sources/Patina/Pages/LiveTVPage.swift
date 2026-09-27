import SwiftUI

/// Live TV from the server's DVR (web/src/pages/LiveTV.svelte): a guide of
/// channels against the next hours, and scheduled recordings with their rules.
struct LiveTVPage: View {
    enum Tab: String, CaseIterable, Identifiable {
        case guide = "Guide", recordings = "Recordings"
        var id: Self { self }
    }

    @Environment(AppModel.self) private var app
    @State private var tab: Tab = .guide
    /// Bumped every 15 minutes to refetch the guide.
    @State private var refetch = 0

    var body: some View {
        VStack(spacing: 0) {
            PageHeader("Live TV", subtitle: "Watch and record from your server's tuner.")
            switch tab {
            case .guide:
                Loading(id: refetch) { try await API.livetvGuide(hours: 6) } content: { channels in
                    if channels.isEmpty {
                        EmptyState(title: "No channels", message: "Set up a tuner and scan for channels in Plex's Live TV & DVR settings.")
                            .frame(maxHeight: .infinity, alignment: .top)
                    } else {
                        LiveTVGuide(channels: channels)
                    }
                }
                .frame(maxHeight: .infinity, alignment: .top)
                .task {
                    while !Task.isCancelled {
                        try? await Task.sleep(for: .seconds(15 * 60))
                        if Task.isCancelled { break }
                        refetch += 1
                    }
                }
            case .recordings:
                LiveTVRecordings()
            }
        }
        .navigationTitle("Live TV")
        .toolbar {
            ToolbarItem(placement: .principal) {
                Picker("View", selection: $tab) {
                    ForEach(Tab.allCases) { Text($0.rawValue).tag($0) }
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .fixedSize()
            }
            ToolbarItem(placement: .primaryAction) {
                Button("Reload", systemImage: "arrow.clockwise") {
                    if tab == .guide { refetch += 1 } else { app.refresh() }
                }
                .help("Reload")
                .keyboardShortcut("r", modifiers: .command)
            }
        }
    }
}

enum LiveTVFormat {
    private static let timeFormat: DateFormatter = {
        let f = DateFormatter()
        f.dateStyle = .none
        f.timeStyle = .short
        return f
    }()

    private static let whenFormat: DateFormatter = {
        let f = DateFormatter()
        f.setLocalizedDateFormatFromTemplate("EEE MMM d j:mm")
        return f
    }()

    static func time(_ s: Int64) -> String { timeFormat.string(from: Date(timeIntervalSince1970: TimeInterval(s))) }
    static func range(_ p: LiveProgram) -> String { "\(time(p.beginsAt)) – \(time(p.endsAt))" }
    static func when(_ s: Int64) -> String { whenFormat.string(from: Date(timeIntervalSince1970: TimeInterval(s))) }
}
