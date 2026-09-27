import SwiftUI

// MARK: Maintenance

struct ServerMaintenanceTab: View {
    let reload: Int
    @Environment(AppModel.self) private var app
    /// Diagnostics can take a while on big servers.
    @State private var saving: String?

    var body: some View {
        Form {
            Section("Database") {
                action("Clean Bundles", "Delete metadata bundles for media that no longer exists.") {
                    app.perform("Cleaning bundles…") { try await API.serverAction("cleanBundles") }
                }
                action("Optimize Database", "Compact and re-index the library database.") {
                    app.perform("Optimizing database…") { try await API.serverAction("optimizeDatabase") }
                }
            }
            Section("Diagnostics") {
                diagnostics("logs", "Download Logs", "Server logs as a zip, for troubleshooting or a support request.")
                diagnostics("database", "Download Database Backup", "A copy of the library database. Large libraries take a while.")
            }
            Section {
                Loading(id: reload) { try await API.butlerTasks() } content: { tasks in
                    ForEach(tasks) { t in
                        let name = t.name ?? ""
                        LabeledContent {
                            HStack(spacing: 8) {
                                if !t.enabled { ServerTag("Disabled") }
                                Button("Start") { app.perform("Task started") { try await API.butlerTask(name, start: true) } }
                                Button("Stop") { app.perform("Task stopped") { try await API.butlerTask(name, start: false) } }
                            }
                            .controlSize(.small)
                        } label: {
                            Text(t.title ?? name)
                            if let d = t.description { Text(d) }
                        }
                    }
                }
            } header: {
                HStack {
                    Text("Scheduled Tasks")
                    Spacer()
                    Button("Start All") { app.perform("Started all tasks") { try await API.butlerAll(start: true) } }
                    Button("Stop All") { app.perform("Stopped all tasks") { try await API.butlerAll(start: false) } }
                }
                .controlSize(.small)
            } footer: {
                Text("Plex's butler runs these during the maintenance window. You can start any of them now.")
                    .font(Theme.caption).foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .formStyle(.grouped)
        .scrollContentBackground(.hidden)
    }

    private func action(_ title: String, _ detail: String, run: @escaping () -> Void) -> some View {
        LabeledContent {
            Button("Run", action: run).controlSize(.small)
        } label: {
            Text(title)
            Text(detail)
        }
    }

    private func diagnostics(_ what: String, _ title: String, _ detail: String) -> some View {
        LabeledContent {
            HStack(spacing: 8) {
                if saving == what { ProgressView().controlSize(.small) }
                Button(saving == what ? "Saving…" : "Download") {
                    saving = what
                    Task {
                        do { app.toast("Saved to \(try await API.saveDiagnostics(what))") } catch { app.error(error) }
                        saving = nil
                    }
                }
                .disabled(saving != nil)
            }
            .controlSize(.small)
        } label: {
            Text(title)
            Text(detail)
        }
    }
}
