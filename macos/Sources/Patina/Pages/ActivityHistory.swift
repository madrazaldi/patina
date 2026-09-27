import SwiftUI

// MARK: History

struct ActivityHistory: View {
    let history: HistoryPage
    @Binding var page: Int

    @Environment(AppModel.self) private var app
    @State private var selection = Set<String>()

    struct Row: Identifiable {
        var id: String
        var item: Metadata
        var who: String
    }

    private var rows: [Row] {
        history.items.enumerated().map { i, m in
            Row(
                id: m.historyKey ?? "\(i)",
                item: m,
                who: m.accountID.flatMap { history.accounts[String($0)] } ?? ""
            )
        }
    }

    var body: some View {
        if history.items.isEmpty && page == 0 {
            EmptyState(title: "No history yet")
        } else {
            VStack(spacing: 0) {
                Table(rows, selection: $selection) {
                    TableColumn("Title") { r in
                        HStack(spacing: 10) {
                            Artwork(r.item.posterPath, width: 64, height: 96)
                                .frame(width: 22, height: 33)
                                .clipShape(RoundedRectangle(cornerRadius: 2))
                            Text(r.item.fullTitle).lineLimit(1)
                        }
                    }
                    .width(min: 240, ideal: 420)
                    TableColumn("User") { r in Text(r.who).foregroundStyle(.secondary) }
                        .width(min: 80, ideal: 140)
                    TableColumn("Watched") { r in
                        Text(r.item.viewedAt.map(Fmt.ago) ?? "")
                            .foregroundStyle(.secondary)
                            .help(r.item.viewedAt.map { Fmt.date($0) } ?? "")
                    }
                    .width(min: 90, ideal: 140)
                }
                .contextMenu(forSelectionType: String.self) { ids in
                    menu(ids)
                } primaryAction: { ids in
                    if ids.count == 1, let r = rows.first(where: { ids.contains($0.id) }), r.item.ratingKey != nil {
                        app.open(r.item)
                    }
                }
                .onDeleteCommand { delete(selection) }
                Hairline()
                HStack(spacing: 14) {
                    Button("Newer", systemImage: "chevron.left") { page -= 1; selection = [] }
                        .disabled(page == 0)
                    Text("Page \(page + 1)").font(Theme.caption).foregroundStyle(.secondary).monospacedDigit()
                    Button { page += 1; selection = [] } label: {
                        HStack(spacing: 4) { Text("Older"); Image(systemName: "chevron.right") }
                    }
                    .disabled(!history.hasMore)
                }
                .controlSize(.small)
                .padding(.vertical, 10)
                .frame(maxWidth: .infinity)
            }
        }
    }

    @ViewBuilder private func menu(_ ids: Set<String>) -> some View {
        let picked = rows.filter { ids.contains($0.id) }
        if picked.count == 1, let r = picked.first, r.item.ratingKey != nil {
            Button("Go to \(r.item.name)", systemImage: "arrow.right.circle") { app.open(r.item) }
        }
        if app.owned, picked.contains(where: { $0.item.historyKey != nil }) {
            Divider()
            Button(picked.count == 1 ? "Delete from History" : "Delete \(picked.count) Entries from History", systemImage: "trash", role: .destructive) {
                delete(ids)
            }
        }
    }

    private func delete(_ ids: Set<String>) {
        guard app.owned else { return }
        let keys = rows.filter { ids.contains($0.id) }.compactMap(\.item.historyKey)
        guard !keys.isEmpty else { return }
        app.perform(refresh: true) {
            for k in keys { try await API.deleteHistory(k) }
        }
        selection = []
    }
}
