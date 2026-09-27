import AppKit
import SwiftUI

// MARK: Pieces

/// A setting's name with a quieter explanation below it.
struct SettingsLabel: View {
    let title: String
    let detail: String?
    init(_ title: String, _ detail: String? = nil) { self.title = title; self.detail = detail }

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title)
            if let detail {
                Text(detail)
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

/// A binding that reads the stored value and saves through `set`.
func settingsBinding<T>(_ value: T, _ set: @escaping (T) -> Void) -> Binding<T> {
    Binding(get: { value }, set: set)
}

/// A text field that saves after typing pauses, on Return, and when it
/// loses focus, rather than on every keystroke.
struct SettingsTextField: View {
    let value: String
    var prompt: String = ""
    var width: CGFloat = 200
    var monospaced = false
    var trim = false
    let save: (String) -> Void

    @State private var text = ""
    @State private var pending: Task<Void, Never>?
    @FocusState private var focused: Bool

    var body: some View {
        TextField("", text: $text, prompt: Text(prompt))
            .labelsHidden()
            .textFieldStyle(.roundedBorder)
            .multilineTextAlignment(.leading)
            .font(monospaced ? .body.monospaced() : .body)
            .autocorrectionDisabled()
            .frame(width: width)
            .focused($focused)
            .onAppear { text = value }
            .onChange(of: value) { _, v in if !focused, pending == nil { text = v } }
            .onChange(of: text) { _, _ in schedule() }
            .onChange(of: focused) { _, on in if !on { commit() } }
            .onSubmit(commit)
            .onDisappear(perform: commit)
    }

    private var cleaned: String { trim ? text.trimmingCharacters(in: .whitespacesAndNewlines) : text }

    private func schedule() {
        pending?.cancel()
        guard cleaned != value else { pending = nil; return }
        pending = Task {
            try? await Task.sleep(for: .milliseconds(700))
            guard !Task.isCancelled else { return }
            commit()
        }
    }

    private func commit() {
        pending?.cancel()
        pending = nil
        let v = cleaned
        if v != value { save(v) }
    }
}
