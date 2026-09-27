import SwiftUI

/// The section's server hubs (Recently Added, Rediscover, ...).
struct LibraryRecommended: View {
    let id: String
    @Environment(AppModel.self) private var app

    var body: some View {
        Loading(id: "\(id)|\(app.revision)", { try await API.sectionHubs(id) }) { hubs in
            let shown = hubs.filter { !$0.metadata.isEmpty }
            if shown.isEmpty {
                EmptyState(title: "No recommendations yet")
            } else {
                VStack(alignment: .leading, spacing: 34) {
                    ForEach(Array(shown.enumerated()), id: \.offset) { _, hub in
                        HubRow(hub: hub)
                    }
                }
                .padding(.top, 4)
            }
        }
    }
}

struct LibraryCollections: View {
    let id: String
    @Environment(AppModel.self) private var app

    var body: some View {
        Loading(id: "\(id)|\(app.revision)", { try await API.collections(id) }) { items in
            if items.isEmpty {
                EmptyState(title: "No collections", message: "Collections you create in Plex show up here.")
            } else {
                CardGrid(items: items)
            }
        }
    }
}

/// The section's folder tree, one level at a time, with a breadcrumb trail.
struct LibraryFolders: View {
    @Bindable var model: LibraryModel
    let root: String
    @Environment(AppModel.self) private var app

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            crumbs
            Loading(id: "\(model.id)|\(model.trail.last?.key ?? "")|\(app.revision)", {
                try await API.sectionFolders(model.id, key: model.trail.last?.key)
            }) { folder in
                if folder.directories.isEmpty && folder.items.isEmpty {
                    EmptyState(title: "Empty folder")
                } else {
                    VStack(alignment: .leading, spacing: 28) {
                        if !folder.directories.isEmpty { directories(folder.directories) }
                        if !folder.items.isEmpty { CardGrid(items: folder.items) }
                    }
                }
            }
        }
    }

    private var crumbs: some View {
        HStack(spacing: 6) {
            crumb(root, current: model.trail.isEmpty) { model.trail = [] }
            ForEach(Array(model.trail.enumerated()), id: \.offset) { i, step in
                Image(systemName: "chevron.right").font(.system(size: 9, weight: .semibold)).foregroundStyle(.tertiary)
                crumb(step.title, current: i == model.trail.count - 1) { model.trail = Array(model.trail.prefix(i + 1)) }
            }
            Spacer()
        }
        .padding(.horizontal, Theme.gutter)
        .padding(.bottom, 12)
        .overlay(alignment: .bottom) { Hairline().padding(.horizontal, Theme.gutter) }
        .padding(.bottom, 18)
    }

    private func crumb(_ title: String, current: Bool, action: @escaping () -> Void) -> some View {
        Button(title, action: action)
            .buttonStyle(.plain)
            .font(.callout)
            .foregroundStyle(current ? .primary : .secondary)
            .disabled(current)
    }

    private func directories(_ dirs: [Directory]) -> some View {
        VStack(spacing: 0) {
            ForEach(Array(dirs.enumerated()), id: \.offset) { i, d in
                if i > 0 { Hairline() }
                Button {
                    model.trail.append(.init(key: d.key, title: d.title ?? "Folder"))
                } label: {
                    HStack(spacing: 10) {
                        Image(systemName: "folder").foregroundStyle(.secondary)
                        Text(d.title ?? "Folder").lineLimit(1)
                        Spacer()
                        Image(systemName: "chevron.right").font(.system(size: 10, weight: .semibold)).foregroundStyle(.tertiary)
                    }
                    .frame(minHeight: 36)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, Theme.gutter)
    }
}
