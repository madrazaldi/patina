// Port of the "Session" section of desktop/commands/index.ts.

import Foundation
import PlexKit

extension Commands {
    public static func retry(_ app: AppState) async {
        await Session.retry(app)
    }

    /// Start a PIN sign-in; strong PINs open the browser straight away.
    public static func startPin(_ app: AppState, _ host: any Host, strong: Bool) async throws -> Session.NewPin {
        let pin = try await Session.createPin(app, strong)
        if strong { Task { try? await host.openUrl(pin.url) } }
        return pin
    }

    /// Wait for a PIN to be approved and sign in. Returns early if cancelled.
    public static func completePin(_ app: AppState, id: Int, attempt: Int) async throws {
        try await Session.completePin(app, id, attempt)
    }

    public static func cancelPin(_ app: AppState) {
        Session.cancelPin(app)
    }

    public static func passwordSignIn(_ app: AppState, login: String, password: String, code: String? = nil)
        async throws
    {
        try await Session.passwordSignIn(app, login, password, code)
    }

    public static func signInManual(_ app: AppState, url: String, token: String) async throws {
        try await Session.signInManual(app, url, token)
    }

    public static func pickServer(_ app: AppState, id: String) async {
        await Session.run(app) { try await Session.pickServer(app, id) }
    }

    public static func switchServer(_ app: AppState, id: String) async throws {
        try await Session.switchServer(app, id)
    }

    public static func switchUser(_ app: AppState, uuid: String, pin: String? = nil) async throws {
        try await Session.switchUser(app, uuid, pin)
    }

    public static func signOut(_ app: AppState) {
        Session.signOut(app)
    }

    public static func reloadSections(_ app: AppState) async throws {
        try await Session.reloadSections(app)
    }

    public static func homeUsers(_ app: AppState) async throws -> [HomeUser] {
        guard let tv = app.tv() else { return [] }
        return try await tv.homeUsers()
    }
}
