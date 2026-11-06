// Port of desktop/commands/seerr.ts.
// Commands for Seerr: discover and request titles, follow and manage requests.

import Foundation
import SeerrKit

extension Commands {
    /// The configured instance (`url`, `auth`), or `nil`; does not contact it.
    public static func seerrConfig(_ app: AppState) -> SeerrConfig? {
        Seerr.config(app)
    }

    /// Contact the instance: version, the signed-in user and what they may do.
    public static func seerrStatus(_ app: AppState) async throws -> SeerrStatus {
        try await Seerr.status(app)
    }

    /// Check and save an instance; an empty `apiKey` signs in with the Plex account, `nil`
    /// keeps the saved key.
    public static func seerrConnect(_ app: AppState, url: String, apiKey: String?) async throws -> SeerrStatus {
        try await Seerr.connect(app, url, apiKey)
    }

    public static func seerrDisconnect(_ app: AppState) throws {
        try Seerr.disconnect(app)
    }

    /// A discover list: `trending`, `movies`, `tv`, `upcoming-movies` or `upcoming-tv`.
    public static func seerrDiscover(_ app: AppState, list: String, page: Int = 1) async throws -> SeerrPage<SeerrTitle> {
        try await Seerr.discover(app, list, page)
    }

    public static func seerrSearch(_ app: AppState, query: String, page: Int = 1) async throws -> SeerrPage<SeerrTitle> {
        try await Seerr.search(app, query, page)
    }

    public static func seerrDetails(_ app: AppState, mediaType: SeerrMediaType, tmdbId: Int) async throws -> SeerrDetails {
        try await Seerr.details(app, mediaType, tmdbId)
    }

    /// Request a movie, or seasons of a show. Returns the request's state.
    public static func seerrRequest(_ app: AppState, request: Seerr.NewRequest) async throws -> RequestState {
        try await Seerr.request(app, request)
    }

    /// Radarr or Sonarr servers to choose from (advanced requests).
    public static func seerrRequestOptions(_ app: AppState, mediaType: SeerrMediaType, is4k: Bool) async throws
        -> [Seerr.RequestOptionServer]
    {
        try await Seerr.requestOptions(app, mediaType, is4k)
    }

    /// Requests by `filter` (see `Seerr.REQUEST_FILTERS`), 20 a page; `mine` limits them to the user's own.
    public static func seerrRequests(_ app: AppState, filter: String = "all", page: Int = 1, mine: Bool = false)
        async throws -> SeerrPage<SeerrRequest>
    {
        try await Seerr.requests(app, filter, page, mine: mine)
    }

    /// `approve`, `decline`, `retry` or `delete` a request.
    public static func seerrRequestAction(_ app: AppState, id: Int, action: String) async throws {
        try await Seerr.requestAction(app, id, action)
    }

    /// Open a title's page on the Seerr website.
    public static func seerrOpen(_ app: AppState, _ host: any Host, mediaType: SeerrMediaType, tmdbId: Int) async throws {
        try await host.openUrl(Seerr.pageUrl(app, mediaType, tmdbId))
    }
}
