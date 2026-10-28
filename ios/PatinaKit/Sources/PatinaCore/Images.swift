// STUB: the downloads agent replaces this file with the port of desktop/core/images.ts.
// Only what the foundation calls: the server artwork is fetched from.

import Foundation
import PlexKit

public enum Images {
    static var server: PlexServer?

    public static func setServer(_ server: PlexServer?) {
        Images.server = server
    }

    public static func currentServer() -> PlexServer? {
        server
    }
}
