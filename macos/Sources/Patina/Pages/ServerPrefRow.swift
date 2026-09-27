import SwiftUI

/// One server or library setting: a toggle, a menu of choices, or text saved
/// on Return (web/src/components/ServerPrefRow.svelte). `save` returns
/// whether the new value was stored.
struct ServerPrefRow: View {
    let pref: Pref
    var showId = true
    let save: (String) async -> Bool

    /// Unsaved text edit.
    @State private var edit: String?
    @State private var busy = false

    private static func on(_ v: String) -> Bool { v == "true" || v == "1" }

    var body: some View {
        LabeledContent {
            control
        } label: {
            VStack(alignment: .leading, spacing: 3) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Text(pref.label)
                    if showId {
                        Text(pref.id)
                            .font(.system(size: 10.5, design: .monospaced))
                            .foregroundStyle(.tertiary)
                            .textSelection(.enabled)
                    }
                }
                if let s = pref.summary, !s.isEmpty {
                    Text(s).font(Theme.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                }
            }
        }
    }

    @ViewBuilder private var control: some View {
        if pref.kind == "bool" {
            Toggle(pref.label, isOn: Binding(
                get: { Self.on(pref.value) },
                set: { v in commit(v ? "true" : "false") }
            ))
            .labelsHidden()
            .toggleStyle(.switch)
            .controlSize(.small)
            .disabled(busy)
        } else if !pref.options.isEmpty {
            Picker(pref.label, selection: Binding(
                get: { pref.value },
                set: { commit($0) }
            )) {
                if !pref.options.contains(where: { $0.first == pref.value }) {
                    Text(pref.value).tag(pref.value)
                }
                ForEach(pref.options, id: \.self) { o in
                    Text(o.count > 1 ? o[1] : (o.first ?? "")).tag(o.first ?? "")
                }
            }
            .labelsHidden()
            .frame(maxWidth: 260)
            .disabled(busy)
        } else {
            HStack(spacing: 8) {
                TextField(pref.label, text: Binding(get: { edit ?? pref.value }, set: { edit = $0 }))
                    .labelsHidden()
                    .textFieldStyle(.roundedBorder)
                    .multilineTextAlignment(.leading)
                    .frame(width: 240)
                    .onSubmit { if let e = edit, e != pref.value { commit(e) } }
                if let e = edit, e != pref.value {
                    Button("Save") { commit(e) }
                        .controlSize(.small)
                        .disabled(busy)
                }
            }
        }
    }

    private func commit(_ value: String) {
        busy = true
        Task {
            if await save(value) { edit = nil }
            busy = false
        }
    }
}

/// A list of prefs a sheet or page can edit in place.
@MainActor
@Observable
final class ServerPrefList {
    var prefs: [Pref]
    init(_ prefs: [Pref]) { self.prefs = prefs }

    func stored(_ id: String, _ value: String) {
        if let i = prefs.firstIndex(where: { $0.id == id }) { prefs[i].value = value }
    }
}
