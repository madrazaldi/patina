// App lifecycle hooks the SwiftUI scene does not cover: background download
// sessions and background refresh for smart downloads.

import UIKit

public final class PatinaAppDelegate: NSObject, UIApplicationDelegate {
    public func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
    ) -> Bool {
        true
    }
}
