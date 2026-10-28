// Menu commands, which iPad shows in the ⌘ overlay and binds to the hardware keyboard:
// the desktop shortcuts (⌘K command palette, ⌘F search, ⌘Z undo).

import SwiftUI

public struct PatinaCommands: Commands {
    public init() {}

    public var body: some Commands {
        CommandGroup(after: .textEditing) {}
    }
}
