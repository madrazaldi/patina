# ios/AGENTS.md

Guidance for the iOS app (iPhone and iPad). Read the root `AGENTS.md` first; this adds what differs.

The iOS app is a third port of the backend, in Swift, with a SwiftUI front end and an in-app player built on [AetherEngine](https://github.com/superuser404notfound/AetherEngine) (FFmpeg demux over AVPlayer and VideoToolbox) instead of mpv. It has the desktop app's features. It does not share files with the desktop app or the TUI (iOS apps are sandboxed), but it writes the same formats.

## Layout

| Part | Role |
|---|---|
| `ios/Patina.xcodeproj` | The app target: entry point, assets, Info.plist (`ios/Patina-Info.plist` is merged into the generated one). Its `Patina/` folder is a synchronized group, so new files need no project edits. |
| `ios/PatinaKit` | Swift package with the backend and no dependencies; builds and tests on macOS. |
| `PatinaKit/Sources/PlexKit` | Port of `desktop/plex`. `Sendable`, nonisolated. |
| `PatinaKit/Sources/SeerrKit` | Port of `desktop/seerr`. `Sendable`, nonisolated. |
| `PatinaKit/Sources/PatinaCore` | Port of `desktop/core` (and the command handlers of `desktop/commands`). Defaults to `@MainActor`, like the single-threaded JavaScript it ports; plain value types are marked `nonisolated`. |
| `ios/PatinaApp` | Swift package with the iOS-only code, depending on PatinaKit, AetherEngine and [swift-libass](https://github.com/mihai8804858/swift-libass). |
| `PatinaApp/Sources/PatinaPlayer` | The `PlaybackEngine` (`PatinaCore/Playback/PlaybackEngine.swift`) over AetherEngine, plus the player surface, subtitles, gestures, Picture in Picture and Now Playing. ASS and SSA tracks are drawn by libass (`ASSRenderer`, `ASSSubtitles`) from the raw event lines, header and fonts AetherEngine hands over under `preserveASSMarkup`; other text formats by `SubtitleOverlay`. |
| `PatinaApp/Sources/PatinaUI` | Every screen. `RootView` is what the app shows. |

## Commands

```bash
cd ios/PatinaKit
swift build --target PlexKit                 # the backend builds on macOS
swift test                                   # PlexKitTests, SeerrKitTests, PatinaCoreTests (no simulator needed)
swift test --filter PlexKitTests             # one target

# The player and the screens are iOS-only; build the app for the simulator:
cd ios && xcodebuild -project Patina.xcodeproj -scheme Patina -destination 'generic/platform=iOS Simulator' build -quiet
```

Run the app against the mock server (`tools/mock_pms.py`, and `tools/mock_seerr.py` for Requests), as the root `AGENTS.md` describes: the simulator reaches it at `http://127.0.0.1:32499`. Sign in with "Connect to a server" (URL and token `x`). The Swift tests don't use the scripts; they use in-process ports of them (`Tests/PatinaCoreTests/Support/MockPMS.swift`, `MockSeerr.swift`).

Debug builds read these launch environment variables (`SIMCTL_CHILD_<name>` with `xcrun simctl launch`):

- `PATINA_SIGNIN=<url>|<token>` signs in to a server on start.
- `PATINA_ROUTE=/item/<ratingKey>` (any route, as on the desktop) opens a page on start.
- `PATINA_PLAY=<ratingKey>` starts playing an item on start.

## Porting rules

- **Keep the TypeScript names and behaviour.** Every Swift file starts with `// Port of desktop/<path>.ts.` (`web/src/<path>` for the screens), or says what it is when it has no twin. Modules, types and functions keep their names in Swift casing (`downloads.enqueue(app, ratingKey)` becomes `Downloads.enqueue(app, ratingKey:)`). Comments explain why, as in the TypeScript, and stay as sparse.
- **Plex JSON round-trips.** Models are `Codable`. They decode leniently through the `de` helpers (`optString`, `optI64`, `optF64`, `boolean`, `oneOrMany`, the TypeScript names) and encode with Plex's own field names, so a stored model (in `downloads.json`, the saved queue) reads back as it was received.
- **Tests are ports of the TypeScript tests**, written with Swift Testing (`import Testing`). HTTP is stubbed with a `URLProtocol`. No test talks to a real server.
- **What iOS changes:**
  - Settings are `config.json` in Application Support, with the same keys as `config.toml`. Tokens are kept in the Keychain, never in the file.
  - There is one process, so there are no cross-process locks. The download worker and offline sync keep an in-process `Storage.tryLock` lease instead.
  - Downloads go to `Documents/Downloads`, which the Files app shows, and transfer in a background `URLSession`.
  - The app owns the play queue and the controls. mpv owned them on the desktop, so `player.ts` is ported against `PlaybackEngine` instead.
  - Discord Rich Presence and the mpv-specific settings (path, extra arguments, `mpv.conf`, the skip key) have no iOS equivalent.
- **Concurrency:** Swift 6 language mode. The `@MainActor` modules use `app.spawn {}` (a tracked `Task`; tests `await app.settled()`) where the TypeScript uses `app.spawn`, and `Task.sleep` loops where it uses `setInterval`. PlexKit and SeerrKit are free of actor isolation and `Sendable`.

## UI

- SwiftUI, iOS 18+. `NavigationSplitView` with a sidebar on iPad, a `TabView` on iPhone (compact width). Dark only, with the user's accent (`ui.accent`).
- Navigation uses `Route` values (`/item/<ratingKey>`, `/library/<id>`, ... as in `web/src/lib/router.svelte.ts`), so any screen can link to any other without importing it.
- iPad with a keyboard gets the desktop shortcuts: ⌘K opens the command palette, ⌘F focuses search, ⌘Z undoes, and Space plays or pauses in the player.
