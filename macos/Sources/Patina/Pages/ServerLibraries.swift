import SwiftUI

// MARK: Libraries

enum ServerLibrarySheet: Identifiable {
    case add
    case edit(Directory)
    case prefs(Directory)

    var id: String {
        switch self {
        case .add: "add"
        case .edit(let d): "edit:\(d.id)"
        case .prefs(let d): "prefs:\(d.id)"
        }
    }
}

struct ServerLibrariesTab: View {
    @Environment(AppModel.self) private var app
    @State private var sheet: ServerLibrarySheet?
    @State private var selection: String?

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Scan for new files, change folders and settings, or add a library.")
                    .foregroundStyle(.secondary)
                Spacer()
                Button("Scan All") { app.perform("Scanning all libraries…") { try await API.serverAction("scanAll") } }
                Button("Add Library…", systemImage: "plus") { sheet = .add }
            }
            .controlSize(.small)
            .padding(.horizontal, Theme.gutter)
            .padding(.bottom, 10)
            if app.sections.isEmpty {
                EmptyState(title: "No libraries", message: "This server has no libraries yet.")
                Spacer()
            } else {
                List(selection: $selection) {
                    ForEach(app.sections) { s in
                        ServerLibraryRow(section: s, sheet: $sheet)
                            .tag(s.id)
                    }
                }
                .listStyle(.inset)
                .scrollContentBackground(.hidden)
                .contextMenu(forSelectionType: String.self) { ids in
                    if let id = ids.first, let s = app.sections.first(where: { $0.id == id }) {
                        ServerLibraryMenu(section: s, sheet: $sheet)
                    }
                } primaryAction: { ids in
                    if let id = ids.first, let s = app.sections.first(where: { $0.id == id }) { sheet = .edit(s) }
                }
            }
        }
        .sheet(item: $sheet) { s in
            switch s {
            case .add: ServerLibraryEditor(section: nil)
            case .edit(let d): ServerLibraryEditor(section: d)
            case .prefs(let d): ServerLibraryPrefs(section: d)
            }
        }
    }
}

struct ServerLibraryRow: View {
    let section: Directory
    @Binding var sheet: ServerLibrarySheet?
    @Environment(AppModel.self) private var app

    var body: some View {
        let id = section.key ?? ""
        let task = app.activities.first { $0.sectionId == id }
        HStack(spacing: 14) {
            Image(systemName: Sidebar.icon(section.type))
                .font(.system(size: 16))
                .foregroundStyle(.secondary)
                .frame(width: 24)
            VStack(alignment: .leading, spacing: 3) {
                Text(section.title ?? "").font(.system(size: 13, weight: .medium))
                Text(section.locations.compactMap(\.path).joined(separator: ", "))
                    .font(Theme.caption).foregroundStyle(.secondary)
                    .lineLimit(1).truncationMode(.middle)
                    .textSelection(.enabled)
                if let task {
                    Text("\(task.subtitle.flatMap { $0.isEmpty ? nil : $0 } ?? task.title) · \(task.progress)%")
                        .font(Theme.caption).foregroundStyle(.secondary).monospacedDigit()
                    ProgressView(value: Double(task.progress), total: 100)
                        .progressViewStyle(.linear).controlSize(.small).tint(app.accent)
                        .frame(maxWidth: 360)
                } else if section.refreshing {
                    Text("Scanning…").font(Theme.caption).foregroundStyle(.secondary)
                } else if let at = section.scannedAt {
                    Text("Last scanned \(Fmt.ago(at))").font(Theme.caption).foregroundStyle(.tertiary)
                }
            }
            Spacer(minLength: 12)
            HStack(spacing: 8) {
                if task != nil || section.refreshing {
                    Button("Cancel") { app.perform("Scan cancelled") { try await API.cancelScan(id) } }
                } else {
                    Button("Scan") { app.perform("Scanning…") { try await API.sectionAction(id, "scan") } }
                }
                Button("Edit…") { sheet = .edit(section) }
                Button("Settings…") { sheet = .prefs(section) }
                Menu {
                    ServerLibraryMenu(section: section, sheet: $sheet, compact: true)
                } label: {
                    Image(systemName: "ellipsis.circle")
                }
                .menuStyle(.borderlessButton)
                .menuIndicator(.hidden)
                .fixedSize()
                .help("More actions for \(section.title ?? "")")
            }
            .controlSize(.small)
        }
        .padding(.vertical, 6)
    }
}

/// Actions on a library, for its context menu and "…" button.
struct ServerLibraryMenu: View {
    let section: Directory
    @Binding var sheet: ServerLibrarySheet?
    /// Leave out what the row already has buttons for.
    var compact = false
    @Environment(AppModel.self) private var app

    var body: some View {
        let id = section.key ?? ""
        let title = section.title ?? ""
        if !compact {
            Button("Scan Library Files", systemImage: "arrow.triangle.2.circlepath") {
                app.perform("Scanning…") { try await API.sectionAction(id, "scan") }
            }
        }
        Button("Refresh All Metadata", systemImage: "arrow.clockwise") {
            app.perform("Refreshing all metadata…") { try await API.sectionAction(id, "refresh") }
        }
        Button("Analyze Media", systemImage: "waveform.badge.magnifyingglass") {
            app.perform("Analyzing…") { try await API.sectionAction(id, "analyze") }
        }
        Button("Empty Trash", systemImage: "trash.slash") {
            app.confirm(
                "Empty the trash for “\(title)”?",
                message: "Items whose files are missing are removed from the library.",
                action: "Empty Trash", destructive: true
            ) {
                await app.act("Trash emptied") { try await API.sectionAction(id, "emptyTrash") }
            }
        }
        if !compact {
            Divider()
            Button("Edit…", systemImage: "pencil") { sheet = .edit(section) }
            Button("Settings…", systemImage: "slider.horizontal.3") { sheet = .prefs(section) }
        }
        Divider()
        Button("Delete Library…", systemImage: "trash", role: .destructive) {
            app.confirm(
                "Delete “\(title)”?",
                message: "The library and its metadata are removed from the server. Media files on disk are not touched.",
                action: "Delete Library", destructive: true
            ) {
                await app.act("Deleted “\(title)”") { try await API.deleteLibrary(id) }
            }
        }
    }
}
