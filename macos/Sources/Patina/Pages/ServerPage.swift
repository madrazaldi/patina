import SwiftUI

/// Plex Media Server administration for owners (web/src/pages/Server.svelte):
/// overview and statistics, libraries, maintenance, settings, and users & devices.
struct ServerPage: View {
    enum Tab: String, CaseIterable, Identifiable {
        case overview = "Overview", libraries = "Libraries", maintenance = "Maintenance", settings = "Settings", users = "Users & Devices"
        var id: Self { self }
    }

    @Environment(AppModel.self) private var app
    @State private var tab: Tab = .overview
    /// Bumped by Reload to refetch the current tab.
    @State private var reload = 0

    var body: some View {
        Group {
            if !app.owned {
                EmptyState(title: "Server tools need owner access", message: "This server is shared with you.")
                    .frame(maxHeight: .infinity)
            } else {
                VStack(spacing: 0) {
                    PageHeader(app.session?.serverName ?? "Server", subtitle: "Manage your Plex Media Server.")
                    Group {
                        switch tab {
                        case .overview: ServerOverviewTab(reload: reload)
                        case .libraries: ServerLibrariesTab()
                        case .maintenance: ServerMaintenanceTab(reload: reload)
                        case .settings:
                            Loading(id: reload) { try await API.prefs() } content: { prefs in
                                // @State keeps its first value: rebuild when a reload returns new values.
                                ServerSettingsTab(list: ServerPrefList(prefs))
                                    .id(prefs.map { "\($0.id)=\($0.value)" })
                            }
                        case .users:
                            Loading(id: reload) { try await API.usersDevices() } content: { d in
                                ServerUsersTab(data: d)
                            }
                        }
                    }
                    .frame(maxHeight: .infinity, alignment: .top)
                }
            }
        }
        .navigationTitle("Manage Server")
        .toolbar {
            if app.owned {
                ToolbarItem(placement: .principal) {
                    Picker("Section", selection: $tab) {
                        ForEach(Tab.allCases) { Text($0.rawValue).tag($0) }
                    }
                    .pickerStyle(.segmented)
                    .labelsHidden()
                    .fixedSize()
                }
                ToolbarItem(placement: .primaryAction) {
                    Button("Reload", systemImage: "arrow.clockwise") {
                        if tab == .libraries { app.perform { try await API.reloadSections() } } else { reload += 1 }
                    }
                    .help("Reload")
                    .keyboardShortcut("r", modifiers: .command)
                }
            }
        }
    }
}
