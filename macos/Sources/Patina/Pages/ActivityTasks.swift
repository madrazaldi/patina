import SwiftUI

// MARK: Server tasks

struct ActivityTasks: View {
    let tick: Int
    @Environment(AppModel.self) private var app

    var body: some View {
        Form {
            Section("Background activity") {
                if app.activities.isEmpty {
                    Text("The server is idle.").foregroundStyle(.secondary)
                }
                ForEach(app.activities) { a in
                    ActivityProgressRow(
                        title: a.title,
                        detail: a.subtitle ?? "",
                        progress: Double(a.progress) / 100
                    ) {
                        if a.cancellable {
                            Button("Cancel") { app.perform { try await API.cancelActivity(a.uuid) } }
                        }
                    }
                }
            }
            Section("Transcoder sessions") {
                Loading(id: tick) { try await API.tasks().transcodes } content: { list in
                    if list.isEmpty {
                        Text("No transcodes running.").foregroundStyle(.secondary)
                    }
                    ForEach(Array(list.enumerated()), id: \.offset) { _, t in
                        let up = { (s: String?) in (s ?? "").uppercased() }
                        let speed = String(format: "%.1f", t.speed ?? 0)
                        ActivityProgressRow(
                            title: "\(up(t.sourceVideoCodec)) → \(up(t.videoCodec)) · \(up(t.audioCodec))",
                            detail: "\(t.context ?? "") · \(t.protocol ?? "") · speed \(speed)×\(t.throttled ? " (throttled)" : "")",
                            progress: (t.progress ?? 0) / 100
                        ) {
                            if let key = t.key {
                                Button("Stop", role: .destructive) { app.perform("Transcode stopped", refresh: false) { try await API.killTranscode(key) } }
                            }
                        }
                    }
                }
            }
        }
        .formStyle(.grouped)
        .scrollContentBackground(.hidden)
    }
}

struct ActivityProgressRow<Control: View>: View {
    let title: String
    let detail: String
    let progress: Double
    @ViewBuilder var control: () -> Control

    @Environment(AppModel.self) private var app

    var body: some View {
        HStack(alignment: .center, spacing: 16) {
            VStack(alignment: .leading, spacing: 3) {
                Text(title)
                if !detail.isEmpty { Text(detail).font(Theme.caption).foregroundStyle(.secondary) }
                ProgressView(value: min(1, max(0, progress)))
                    .progressViewStyle(.linear)
                    .controlSize(.small)
                    .tint(app.accent)
                    .frame(maxWidth: 480)
                    .padding(.top, 5)
            }
            Spacer()
            Text("\(Int((progress * 100).rounded()))%").font(Theme.caption).foregroundStyle(.secondary).monospacedDigit()
            control()
        }
        .padding(.vertical, 3)
    }
}
