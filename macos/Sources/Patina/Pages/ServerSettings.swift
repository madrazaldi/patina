import SwiftUI

// MARK: Settings

struct ServerSettingsTab: View {
    @State var list: ServerPrefList
    @Environment(AppModel.self) private var app
    @State private var filter = ""
    @State private var advanced = false

    private var shown: [Pref] {
        let f = filter.lowercased()
        return list.prefs.filter { p in
            (advanced || !p.advanced)
                && (f.isEmpty || [p.label, p.id, p.summary ?? ""].contains { $0.lowercased().contains(f) })
        }
    }

    var body: some View {
        let shown = self.shown
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                HStack(spacing: 6) {
                    Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
                    TextField("Filter settings", text: $filter).textFieldStyle(.plain)
                    if !filter.isEmpty {
                        Button { filter = "" } label: { Image(systemName: "xmark.circle.fill") }
                            .buttonStyle(.borderless).foregroundStyle(.secondary)
                    }
                }
                .padding(.horizontal, 8)
                .frame(width: 300, height: 26)
                .background(RoundedRectangle(cornerRadius: 6).fill(Theme.placeholder))
                Toggle("Show Advanced", isOn: $advanced).toggleStyle(.checkbox)
                Spacer()
                Text(Fmt.plural(shown.count, "setting")).font(Theme.caption).foregroundStyle(.secondary).monospacedDigit()
            }
            .padding(.horizontal, Theme.gutter)
            .padding(.bottom, 6)
            if shown.isEmpty {
                EmptyState(title: "No matching settings")
                Spacer()
            } else {
                Form {
                    ForEach(shown) { p in
                        ServerPrefRow(pref: p) { v in
                            let ok = await app.act("Setting saved") { try await API.setPref(p.id, v) }
                            if ok { list.stored(p.id, v) }
                            return ok
                        }
                    }
                }
                .formStyle(.grouped)
                .scrollContentBackground(.hidden)
            }
        }
    }
}
