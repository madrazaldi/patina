import SwiftUI

// MARK: Recordings

struct LiveTVRecordings: View {
    @Environment(AppModel.self) private var app

    var body: some View {
        Loading(id: app.revision) { try await API.recordings() } content: { r in
            List {
                Section {
                    if r.upcoming.isEmpty {
                        Text("Nothing scheduled. Record a program from the guide.").foregroundStyle(.secondary)
                    }
                    ForEach(r.upcoming) { u in
                        HStack(alignment: .firstTextBaseline, spacing: 16) {
                            Text(u.beginsAt.map(LiveTVFormat.when) ?? "")
                                .font(Theme.caption)
                                .foregroundStyle(.secondary)
                                .monospacedDigit()
                                .frame(width: 150, alignment: .leading)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(u.title)
                                let sub = [u.subtitle, u.channel].compactMap { $0 }.joined(separator: " · ")
                                if !sub.isEmpty { Text(sub).font(Theme.caption).foregroundStyle(.secondary) }
                            }
                            Spacer()
                            if let status = u.status { ServerTag(status.capitalized) }
                        }
                        .padding(.vertical, 3)
                    }
                } header: {
                    LiveTVSectionHeader("Upcoming")
                }
                Section {
                    if r.rules.isEmpty {
                        Text("No recording rules.").foregroundStyle(.secondary)
                    }
                    ForEach(r.rules) { rule in
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(rule.title)
                                if let k = rule.kind, Int(k) == nil { Text(k).font(Theme.caption).foregroundStyle(.secondary) }
                            }
                            Spacer()
                            Button { delete(rule) } label: { Image(systemName: "trash") }
                                .buttonStyle(.borderless)
                                .help("Delete rule")
                        }
                        .padding(.vertical, 3)
                        .contextMenu {
                            Button("Delete Rule…", systemImage: "trash", role: .destructive) { delete(rule) }
                        }
                    }
                } header: {
                    LiveTVSectionHeader("Rules")
                }
            }
            .listStyle(.inset)
            .scrollContentBackground(.hidden)
        }
        .frame(maxHeight: .infinity, alignment: .top)
    }

    private func delete(_ rule: Recordings.Rule) {
        app.confirm(
            "Delete recording rule?",
            message: "\(rule.title) will no longer be recorded. Existing recordings are kept.",
            action: "Delete", destructive: true
        ) {
            await app.act("Recording rule deleted", refresh: true) { try await API.deleteRecordingRule(rule.id) }
        }
    }
}

struct LiveTVSectionHeader: View {
    let title: String
    init(_ title: String) { self.title = title }
    var body: some View {
        Text(title)
            .font(Theme.display(17))
            .foregroundStyle(.primary)
            .textCase(nil)
            .padding(.top, 12)
            .padding(.bottom, 4)
    }
}
