// What the app shows: the sign-in flow until a server is connected, then the shell.
// The iOS counterpart of web/src/App.svelte.

import SwiftUI

public struct RootView: View {
    public init() {}

    public var body: some View {
        ZStack {
            Theme.background.ignoresSafeArea()
            Text("Patina")
                .font(.largeTitle.bold())
        }
        .preferredColorScheme(.dark)
    }
}
