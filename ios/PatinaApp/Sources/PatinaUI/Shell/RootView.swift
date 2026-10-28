// What the app shows: the sign-in flow until a server is connected, then the shell, with
// the overlays every screen shares (sheets, confirmations, toasts). The iOS counterpart of
// web/src/App.svelte.

import PatinaCore
import SwiftUI

public struct RootView: View {
    @State private var store: AppStore

    public init() {
        _store = State(initialValue: AppStore())
    }

    init(store: AppStore) {
        _store = State(initialValue: store)
    }

    public var body: some View {
        @Bindable var store = store
        ZStack {
            Theme.background.ignoresSafeArea()
            if store.stage == .ready && store.session != nil {
                Shell()
                    .transition(.opacity)
            } else {
                Gate()
                    .transition(.opacity)
            }
        }
        .animation(.easeInOut(duration: 0.35), value: store.stage == .ready && store.session != nil)
        .overlay(alignment: .bottom) { ToastStack() }
        .sheet(item: $store.sheet) { sheet in
            SheetHost(sheet: sheet)
                .environment(store)
                .tint(store.accent)
                .preferredColorScheme(.dark)
        }
        .sheet(isPresented: $store.palette) {
            CommandPalettePlaceholder()
                .environment(store)
                .presentationDetents([.medium])
                .preferredColorScheme(.dark)
        }
        .alert(
            store.confirming?.title ?? "",
            isPresented: Binding(get: { store.confirming != nil }, set: { if !$0 { store.answer(false) } }),
            presenting: store.confirming
        ) { c in
            Button("Cancel", role: .cancel) { store.answer(false) }
            Button(c.action, role: c.danger ? .destructive : nil) { store.answer(true) }
        } message: { c in
            if let text = c.text { Text(text) }
        }
        .environment(store)
        .tint(store.accent)
        .preferredColorScheme(.dark)
        .focusedSceneValue(\.patinaStore, store)
    }
}

/// The dialog an `ItemSheet` opens.
struct SheetHost: View {
    let sheet: ItemSheet

    var body: some View {
        switch sheet {
        case .playlist(let item, let items): PlaylistSheet(item: item, items: items)
        case .collection(let item, let items): CollectionSheet(item: item, items: items)
        case .edit(let item): EditSheet(item: item)
        case .artwork(let item, let kind): ArtworkSheet(item: item, kind: kind)
        case .match(let item): MatchSheet(item: item)
        case .subtitle(let item): SubtitleSheet(item: item)
        case .switchUser: SwitchUserSheet()
        case .customizeHome: CustomizeHomeSheet()
        }
    }
}

#Preview("Sign in") {
    RootView(store: .preview(stage: .login))
}
