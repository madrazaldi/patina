import SwiftUI

/// Any server listing by key: "See all" for hubs, and tag filters (genre,
/// actor, ...). Loads in pages as you scroll.
struct BrowsePage: View {
    let path: String
    let title: String

    @Environment(AppModel.self) private var app
    @State private var items: [Metadata] = []
    @State private var total: Int?
    @State private var done = false
    @State private var loading = false
    @State private var error: String?

    private static let page = 100

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                PageHeader(title, subtitle: total.map { Fmt.plural($0, "title") })
                if items.isEmpty {
                    if let error {
                        ErrorState(message: error) { load() }
                    } else if loading || !done {
                        ProgressView().controlSize(.small).frame(maxWidth: .infinity, minHeight: 200)
                    } else {
                        EmptyState(title: "Nothing here")
                    }
                } else {
                    CardGrid(items: items, onAppear: { i in
                        if i >= items.count - 24 { load() }
                    }) {
                        footer
                    }
                }
            }
            .padding(.bottom, 40)
        }
        .navigationTitle(title)
        .task { if items.isEmpty { load() } }
    }

    @ViewBuilder private var footer: some View {
        if let error {
            HStack(spacing: 10) {
                Text(error).font(.callout).foregroundStyle(.secondary).lineLimit(2)
                Button("Retry") { load() }
            }
            .frame(maxWidth: .infinity)
            .padding(.top, 24)
        } else if !done {
            ProgressView().controlSize(.small).frame(maxWidth: .infinity).padding(.top, 24)
                .onAppear { load() }
        }
    }

    private func load() {
        guard !loading, !done else { return }
        loading = true
        error = nil
        let start = items.count
        Task {
            do {
                let mc = try await API.listing(path, start: start, size: Self.page)
                items.append(contentsOf: mc.metadata)
                total = mc.totalSize ?? (mc.metadata.count < Self.page ? items.count : nil)
                // Some endpoints ignore paging; stop when a page comes back short.
                done = mc.metadata.count < Self.page || (mc.totalSize.map { items.count >= $0 } ?? false)
            } catch {
                self.error = error.localizedDescription
            }
            loading = false
        }
    }
}
