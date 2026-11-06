// Port of the playback-preferences half of desktop/commands/auto.ts (the smart-downloads half
// is Commands+SmartDownloads.swift).

import Foundation
import PlexKit

extension Commands {
    /// The profile's rules, and a show's when `show` is given, with what applies.
    public static func playbackPrefs(_ app: AppState, show: String? = nil) throws -> Prefs.View {
        try Prefs.view(app, show)
    }

    /// Replace the rules of the profile (`show: nil`), or of a show; empty rules clear them.
    public static func setPlaybackPrefs(_ app: AppState, show: String? = nil, title: String? = nil, rules: Prefs.Rules)
        throws
    {
        try Prefs.save(app, show, title, rules)
    }

    /// Shows that have rules of their own.
    public static func playbackPrefsShows(_ app: AppState) throws -> [Prefs.ShowRules] {
        try Prefs.shows(app)
    }

    /// Index of the version of an item the rules prefer.
    public static func preferredVersion(_ app: AppState, item: Metadata) -> Int {
        Prefs.preferredVersion(app, item)
    }

    /// Remember the track dialog's choices as the show's rules.
    public static func tracksRemember(_ app: AppState, query: Tracks.Query) throws {
        try Prefs.rememberTracks(app, query)
    }
}
