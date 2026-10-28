// Port of desktop/core/session.ts.
// Sign-in, server discovery and connection management. The functions are statics of the
// `Session` struct (State.swift), which keeps the TypeScript type's name.

import Foundation
import PlexKit

extension Session {
    /// Run a sign-in step, showing its error on the failure screen.
    public static func run(_ app: AppState, _ step: () async throws -> Void) async {
        do {
            try await step()
        } catch {
            app.setStage(.failed(message: errorText(error)))
        }
    }

    /// Restore the previous session on startup (or after changing accounts).
    public static func restore(_ app: AppState) async {
        let settings = app.settings()
        if let url = settings.server.manualUrl, let manualToken = settings.server.manualToken {
            app.setStage(.connecting(name: url))
            await run(app) { try await connectManual(app, url, manualToken) }
            return
        }
        guard let token = settings.auth.token else {
            app.setStage(.login)
            return
        }
        await run(app) { try await signIn(app, token) }
    }

    /// Try the saved session again (from the failure screen).
    public static func retry(_ app: AppState) async {
        app.setStage(.booting)
        await restore(app)
    }

    /// Validate a plex.tv token and connect to the preferred server.
    public static func signIn(_ app: AppState, _ token: String) async throws {
        app.setStage(.connecting(name: "plex.tv"))
        let tv = app.newTv(token)
        let user: User
        do {
            user = try await tv.user()
        } catch let e as PlexError where e.kind == .unauthorized {
            app.updateSettings { $0.auth.token = nil }
            app.setStage(.login)
            return
        } catch {
            throw context(error, "reaching plex.tv")
        }
        app.updateSettings { $0.auth.token = token }
        let servers: [Resource]
        do {
            servers = try await tv.servers()
        } catch {
            throw context(error, "listing servers")
        }
        if servers.isEmpty {
            throw CoreError("No Plex Media Servers are available to \(User.displayName(user)).")
        }
        let preferred = app.settings().server.id
        let choice =
            preferred.flatMap { id in servers.first { Resource.id($0) == id } } ?? (servers.count == 1 ? servers[0] : nil)
        if let choice {
            try await connect(app, tv, user, choice, servers)
        } else {
            app.setStage(.pickServer(servers: servers.map(serverView), user: userView(user)))
            app.pending = PendingServers(tv: tv, user: user, servers: servers)
        }
    }

    /// Connect to a server offered on the "choose a server" screen.
    public static func pickServer(_ app: AppState, _ id: String) async throws {
        guard let pending = app.pending else { throw CoreError("No server list to pick from.") }
        guard let resource = pending.servers.first(where: { Resource.id($0) == id }) else {
            throw CoreError("That server is no longer available.")
        }
        try await connect(app, pending.tv, pending.user, resource, pending.servers)
    }

    /// Connect to one server resource and make it the active session.
    public static func connect(_ app: AppState, _ tv: PlexTv, _ user: User, _ resource: Resource, _ servers: [Resource])
        async throws
    {
        let name = Resource.name(resource)
        app.setStage(.connecting(name: name))
        let server: PlexServer
        do {
            server = try await connectResource(tv, resource)
        } catch {
            throw context(error, "connecting to \(name)")
        }
        app.updateSettings { s in
            s.server.id = Resource.id(resource)
            s.server.lastUri = server.baseUrl.absoluteString
        }
        try await activate(
            app,
            Session(
                tv: tv, server: server, serverName: name, serverId: Resource.id(resource), user: user,
                owned: resource.owned, servers: servers))
    }

    private static func connectManual(_ app: AppState, _ url: String, _ token: String) async throws {
        var server = try app.newServer(url, token)
        let id: Identity
        do {
            id = try await server.identity()
        } catch {
            throw context(error, "contacting server")
        }
        server = server.withMachineIdentifier(id.machineIdentifier)
        let caps: MediaContainer
        do {
            caps = try await server.capabilities()
        } catch {
            throw context(error, "server rejected the token")
        }
        let name = MediaContainer.attr(caps, "friendlyName") ?? url
        // A server token usually also works against plex.tv (watchlist, users).
        let tv = app.newTv(token)
        let user = try? await tv.user()
        try await activate(
            app,
            Session(
                tv: user != nil ? tv : nil, server: server, serverName: name, serverId: id.machineIdentifier ?? "",
                user: user, owned: true, servers: []))
    }

    /// Save a manual server and connect to it. Errors are thrown (and shown on the form)
    /// rather than sending the app to the failure screen.
    public static func signInManual(_ app: AppState, _ url: String, _ token: String) async throws {
        let trimmed = url.trimmingCharacters(in: .whitespacesAndNewlines)
        let tok = token.trimmingCharacters(in: .whitespacesAndNewlines)
        let full = trimmed.hasPrefix("http") ? trimmed : "http://\(trimmed)"
        try await connectManual(app, full, tok)
        app.updateSettings { s in
            s.server.manualUrl = full
            s.server.manualToken = tok
        }
    }

    private static func activate(_ app: AppState, _ session: Session) async throws {
        let account = Offline.account(session)
        let sections: [Directory]
        do {
            sections = try await session.server.sections()
        } catch {
            throw context(error, "loading libraries")
        }
        app.updateSettings { $0.auth.accountId = account }
        app.setConnected(true)
        Images.setServer(session.server)
        app.pending = nil
        app.setSession(session, sections.filter { !$0.hidden })
        app.setStage(.ready)
        app.bump()
        Live.watch(app)
        Downloads.resume(app)
    }

    public static func reloadSections(_ app: AppState) async throws {
        let server = try app.server()
        let sections = try await server.sections()
        if let current = app.session(), sameServer(current.server, server) {
            app.setConnected(true)
            app.setSections(sections.filter { !$0.hidden })
        }
    }

    /// Switch to another server of the same account.
    public static func switchServer(_ app: AppState, _ id: String) async throws {
        guard let session = app.session() else { throw CoreError("Not connected.") }
        guard let tv = session.tv, let user = session.user else {
            throw CoreError("Switching servers needs a plex.tv sign-in.")
        }
        guard let resource = session.servers.first(where: { Resource.id($0) == id }) else {
            throw CoreError("Unknown server.")
        }
        do {
            try await connect(app, tv, user, resource, session.servers)
        } catch {
            // Stay on the current server.
            app.setStage(.ready)
            throw error
        }
    }

    /// Switch Plex Home user; the new user's token replaces the stored one.
    public static func switchUser(_ app: AppState, _ uuid: String, _ pin: String?) async throws {
        guard let tv = app.tv() else { throw CoreError("Not signed in to plex.tv.") }
        let user: User
        do {
            user = try await tv.switchHomeUser(uuid, pin: pin)
        } catch {
            throw context(error, "switching user")
        }
        guard let token = user.authToken else { throw CoreError("plex.tv returned no token") }
        app.updateSettings { $0.auth.accountId = user.uuid ?? Offline.digest(token) }
        // The previous user's listener must not outlive their session, even if signing in as the new one fails.
        Live.stop(app)
        app.setSession(nil, [])
        await run(app) { try await signIn(app, token) }
    }

    public static func signOut(_ app: AppState) {
        Player.control(app, .stop)
        Live.stop(app)
        app.updateSettings { s in
            s.auth.token = nil
            s.auth.accountId = nil
            s.server = ServerSettings()
        }
        Images.setServer(nil)
        app.pending = nil
        app.setSession(nil, [])
        app.setStage(.login)
    }

    public nonisolated struct NewPin: Sendable, Equatable {
        public var id: Int
        public var code: String
        public var url: String
        public var attempt: Int
    }

    /// Start a PIN sign-in. `strong` PINs are approved in the browser; short ones are typed in
    /// at plex.tv/link.
    public static func createPin(_ app: AppState, _ strong: Bool) async throws -> NewPin {
        let tv = app.newTv(nil)
        let pin: Pin
        do {
            pin = try await tv.createPin(strong)
        } catch {
            throw context(error, "contacting plex.tv")
        }
        let code = pin.code ?? ""
        let url = tv.authUrl(code)
        app.loginAttempt += 1
        let attempt = app.loginAttempt
        guard let id = pin.id else { throw CoreError("plex.tv returned no PIN") }
        return NewPin(id: id, code: code, url: url, attempt: attempt)
    }

    /// Poll a PIN until it is claimed (or 15 minutes pass), then sign in. Returns early
    /// without signing in when the attempt is cancelled.
    public static func completePin(_ app: AppState, _ pinId: Int, _ attempt: Int) async throws {
        let tv = app.newTv(nil)
        func current() -> Bool { app.loginAttempt == attempt }
        for _ in 0..<450 {
            try await Task.sleep(for: .seconds(2))
            if !current() { return }
            // Keep polling on errors.
            if let pin = try? await tv.checkPin(pinId), let token = pin.authToken {
                if current() { await run(app) { try await signIn(app, token) } }
                return
            }
        }
        throw CoreError("The sign-in code expired. Please try again.")
    }

    public static func cancelPin(_ app: AppState) {
        app.loginAttempt += 1
    }

    /// Sign in with an email or username and password; `code` is the 2FA code, if any.
    public static func passwordSignIn(_ app: AppState, _ login: String, _ password: String, _ code: String?)
        async throws
    {
        let trimmed = code?.trimmingCharacters(in: .whitespacesAndNewlines)
        let twoFactor = trimmed.flatMap { $0.isEmpty ? nil : $0 }
        let tv = app.newTv(nil)
        let user: User
        do {
            user = try await tv.signIn(login, password: password, code: twoFactor)
        } catch let e as PlexError where e.status == 401 {
            throw CoreError("Incorrect email/username or password.")
        }
        guard let token = user.authToken else { throw CoreError("plex.tv returned no token") }
        await run(app) { try await signIn(app, token) }
    }
}
