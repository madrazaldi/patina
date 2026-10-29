// App lifecycle hooks the SwiftUI scene does not cover: background download
// sessions, background refresh for smart downloads, the reports owed to Plex when the app
// leaves the screen, and the orientations the player asks for.

import BackgroundTasks
import PatinaCore
import UIKit

public final class PatinaAppDelegate: NSObject, UIApplicationDelegate {
    /// The `BGAppRefreshTask` of smart downloads (`BGTaskSchedulerPermittedIdentifiers`).
    static let smartDownloadsTask = "io.github.madrazaldi.patina.smart-downloads"

    /// What the app may rotate to now; the video player narrows it to landscape on iPhone.
    static var orientations: UIInterfaceOrientationMask = .all

    /// The app's state, made here so background launches have it before any scene exists.
    public private(set) lazy var store = AppStore()

    public func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
    ) -> Bool {
        BGTaskScheduler.shared.register(forTaskWithIdentifier: Self.smartDownloadsTask, using: .main) { [weak self] task in
            MainActor.assumeIsolated { self?.refresh(task) }
        }
        let paths = store.app.paths
        Task { _ = await Images.pruneCache(paths) }
        NotificationCenter.default.addObserver(
            forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.enteredBackground(application) }
        }
        return true
    }

    public func application(
        _ application: UIApplication, handleEventsForBackgroundURLSession identifier: String,
        completionHandler: @escaping () -> Void
    ) {
        let done = UncheckedCompletion(completionHandler)
        if !Downloads.handleBackgroundEvents(identifier: identifier, completion: { done.call() }) {
            completionHandler()
        }
    }

    public func application(
        _ application: UIApplication, supportedInterfaceOrientationsFor window: UIWindow?
    ) -> UIInterfaceOrientationMask {
        Self.orientations
    }

    // MARK: Background

    /// Deliver the playback reports before the system suspends the app, and ask for the next
    /// smart downloads refresh.
    private func enteredBackground(_ application: UIApplication) {
        scheduleRefresh()
        let assertion = BackgroundAssertion()
        assertion.id = application.beginBackgroundTask(withName: "Playback reports") { assertion.end(application) }
        Task {
            await Report.flush(8000)
            assertion.end(application)
        }
    }

    private func scheduleRefresh() {
        guard !Smart.overview(store.app).policies.isEmpty else { return }
        let request = BGAppRefreshTaskRequest(identifier: Self.smartDownloadsTask)
        request.earliestBeginDate = Date(timeIntervalSinceNow: 60 * 60)
        do {
            try BGTaskScheduler.shared.submit(request)
        } catch {
            logger("ui").debug("could not schedule smart downloads: \(errorText(error))")
        }
    }

    private func refresh(_ task: BGTask) {
        scheduleRefresh()
        let app = store.app!
        let work = Task {
            await Smart.backgroundRefresh(app)
            task.setTaskCompleted(success: !Task.isCancelled)
        }
        task.expirationHandler = { work.cancel() }
    }
}

/// The system's completion handler, which the session may hand back on its own queue.
nonisolated private final class UncheckedCompletion: @unchecked Sendable {
    private let f: () -> Void
    init(_ f: @escaping () -> Void) { self.f = f }
    func call() { DispatchQueue.main.async { self.f() } }
}

/// Extra time to finish in the background, ended once.
private final class BackgroundAssertion {
    var id = UIBackgroundTaskIdentifier.invalid

    func end(_ application: UIApplication) {
        guard id != .invalid else { return }
        application.endBackgroundTask(id)
        id = .invalid
    }
}
