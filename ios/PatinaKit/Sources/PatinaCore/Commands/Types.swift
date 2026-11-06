// Port of desktop/commands/types.ts.
// What only the app can do: open things in the OS. On macOS, Foundation has a `Host` class,
// so code there (the tests) writes `PatinaCore.Host`.

import Foundation

public protocol Host: AnyObject {
    /// Open a web page (the PIN sign-in opens plex.tv in the browser).
    func openUrl(_ url: String) async throws
    /// Open a file or folder with the default application (the Files app on iOS).
    func openPath(_ path: String) async throws
}
