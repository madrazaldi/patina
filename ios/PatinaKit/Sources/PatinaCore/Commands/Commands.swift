// Port of desktop/commands/index.ts.
// The front end's whole API: one function per webview command, named the camelCase of the
// command (`set_watched` is `Commands.setWatched`), with typed arguments and results, doing
// what the handler does. The UI calls these directly. They throw to reject; the UI shows
// `errorText(e)`. Tokens never leave the core.
//
// One file per section of index.ts, each an `extension Commands`:
//
// | File                          | Section of index.ts                         |
// |-------------------------------|---------------------------------------------|
// | Commands+App.swift            | App                                         |
// | Commands+Session.swift        | Session                                     |
// | Commands+Browsing.swift       | Browsing                                    |
// | Commands+ItemActions.swift    | Item actions                                |
// | Commands+Undo.swift           | Undo                                        |
// | Commands+Watchlist.swift      | Watchlist, Playlists (admin agent)          |
// | Commands+Playback.swift       | Playback, Bulk audio & subtitles (playback) |
// | Commands+Activity.swift       | Activity, Server administration (admin)     |
// | Commands+Library.swift        | Library browsing, saved views, Pick, Search across servers, Collections (admin) |
// | Commands+Downloads.swift      | Downloads                                   |
// | Commands+LiveTv.swift         | Live TV (admin)                             |
// | Commands+PlaybackPrefs.swift  | auto.ts: playback preferences (playback)    |
// | Commands+SmartDownloads.swift | auto.ts: smart downloads (downloads)        |
// | Commands+Seerr.swift          | seerr.ts (admin)                            |
//
// Conventions: the `AppState` comes first, then the arguments with the TypeScript argument
// keys as labels (`metadata(app, server:, ratingKey:)`). An optional argument the handler
// passes through `clean(...)` is an optional parameter. Commands that need the OS take the
// `Host` (Types.swift); a `Channel` argument (progress) becomes a closure parameter.
// Desktop-only commands are not ported: `probe_mpv` and `open_mpv_config` (no mpv on iOS).

import Foundation

public enum Commands {}
