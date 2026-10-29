import PatinaUI
import SwiftUI

@main
struct PatinaApp: App {
    @UIApplicationDelegateAdaptor(PatinaAppDelegate.self) private var delegate

    var body: some Scene {
        WindowGroup {
            RootView(store: delegate.store)
        }
        .commands { PatinaCommands() }
    }
}
