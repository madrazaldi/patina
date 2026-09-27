import SwiftUI

/// A library section: the paged poster grid with sort, filters, saved views
/// and bulk actions, plus Recommended, Collections and Folders
/// (web/src/pages/Library.svelte).
struct LibraryPage: View {
    let id: String
    @Environment(AppModel.self) private var app

    var body: some View {
        if let section = app.sections.first(where: { $0.key == id }) {
            LibraryContent(section: section, model: LibraryModel.model(scope: app.scope, section: section))
                .id("\(app.scope):\(id)")
        } else {
            EmptyState(title: "Library not found")
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }
}

private struct LibraryContent: View {
    let section: Directory
    @Bindable var model: LibraryModel
    @Environment(AppModel.self) private var app

    private var scan: ServerActivity? { app.activities.first { $0.sectionId == model.id } }

    var body: some View {
        Group {
            switch model.tab {
            case .browse: LibraryBrowse(model: model, header: { header })
            case .recommended: page { LibraryRecommended(id: model.id) }
            case .collections: page { LibraryCollections(id: model.id) }
            case .folders: page { LibraryFolders(model: model, root: section.title ?? "Library") }
            }
        }
        .navigationTitle(section.title ?? "Library")
        .toolbar {
            ToolbarItem(placement: .principal) {
                Picker("View", selection: $model.tab) {
                    ForEach(LibraryModel.Tab.allCases) { Text($0.title).tag($0) }
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .fixedSize()
            }
            if app.owned {
                ToolbarItem(placement: .primaryAction) { LibraryActionsMenu(id: model.id, title: section.title ?? "") }
            }
        }
    }

    /// Non-grid tabs: header, then content, in a scroll view.
    private func page<C: View>(@ViewBuilder _ content: () -> C) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                header
                content()
            }
            .padding(.bottom, 32)
        }
    }

    private var header: some View {
        PageHeader(title: section.title ?? "Library") {
            if let scan {
                HStack(spacing: 7) {
                    ProgressView().controlSize(.small)
                    Text("Scanning… \(scan.progress)%")
                        .font(Theme.caption.monospacedDigit())
                        .foregroundStyle(.secondary)
                }
                .help(scan.subtitle.map { "\(scan.title) — \($0)" } ?? scan.title)
            }
        }
    }
}

/// Owner actions for a whole section.
private struct LibraryActionsMenu: View {
    let id: String
    let title: String
    @Environment(AppModel.self) private var app

    var body: some View {
        Menu {
            Button("Scan Library Files", systemImage: "arrow.clockwise") {
                app.perform("Scanning library…") { try await API.sectionAction(id, "scan") }
            }
            Button("Refresh All Metadata", systemImage: "arrow.triangle.2.circlepath") {
                app.confirm(
                    "Refresh all metadata in “\(title)”?",
                    message: "Plex re-downloads artwork and details for every item. This can take a while.",
                    action: "Refresh"
                ) {
                    await app.act("Refreshing metadata…") { try await API.sectionAction(id, "refresh") }
                }
            }
            Button("Analyze", systemImage: "waveform") {
                app.perform("Analyzing library…") { try await API.sectionAction(id, "analyze") }
            }
            Divider()
            Button("Empty Trash", systemImage: "trash") {
                app.confirm(
                    "Empty the trash for “\(title)”?",
                    message: "Items whose files are missing are removed from the library.",
                    action: "Empty Trash", destructive: true
                ) {
                    await app.act("Emptying trash…", refresh: true) { try await API.sectionAction(id, "emptyTrash") }
                }
            }
        } label: {
            Label("Library Actions", systemImage: "ellipsis.circle")
        }
        .help("Library actions")
    }
}
