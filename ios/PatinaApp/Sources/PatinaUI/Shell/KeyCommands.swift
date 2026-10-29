// Menu commands, which iPad shows in the ⌘ overlay and binds to the hardware keyboard:
// the desktop shortcuts of web/src/components/Shell.svelte (⌘K command palette, ⌘F search,
// ⌘Z undo). The store comes from the focused scene.

import PatinaCore
import SwiftUI

extension FocusedValues {
    /// The store of the focused window.
    @Entry var patinaStore: AppStore?
}

public struct PatinaCommands: SwiftUI.Commands {
    @FocusedValue(\.patinaStore) private var store

    public init() {}

    public var body: some SwiftUI.Commands {
        // ⌘Z undoes the latest action (watched, Continue Watching, playlist entries) rather than
        // text edits, as on the desktop.
        CommandGroup(replacing: .undoRedo) {
            Button("Undo") { store?.undoLatest() }
                .keyboardShortcut("z", modifiers: .command)
                .disabled(store?.stage != .ready)
        }
        CommandGroup(after: .textEditing) {
            Button("Search") { store?.focusSearch() }
                .keyboardShortcut("f", modifiers: .command)
                .disabled(store?.stage != .ready)
            // Opens the command palette (`store.palette`).
            Button("Command Palette") { store?.palette = true }
                .keyboardShortcut("k", modifiers: .command)
                .disabled(store?.stage != .ready)
        }
    }
}
