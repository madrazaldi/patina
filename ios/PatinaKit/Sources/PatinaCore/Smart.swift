// STUB: the downloads agent replaces this file with the port of desktop/core/smart.ts.
// Only what the foundation calls.

import Foundation

public enum Smart {
    /// Changes when the policies or the budget change (the poller re-sends the downloads then).
    public static func fingerprint() -> String {
        ""
    }

    /// Reconcile the policies soon, coalescing calls.
    public static func reconcileSoon(_ app: AppState) {}
}
