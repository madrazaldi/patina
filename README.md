<img src="assets/mark.svg" alt="" width="48">

# Patina

A Plex client written in Rust, with its front ends on one shared backend:

- **Patina for macOS**, a native SwiftUI app (`macos/`).
- **Patina for Windows and Linux**, a desktop app built with Tauri and Svelte (`web/`).
- **patina-tui**, a keyboard-driven terminal client with Miller columns, plus a scriptable CLI with `--json` output. See [Terminal client](#terminal-client-patina-tui).

All of them play everything in **mpv**. mpv starts normally, so it reads your own `mpv.conf`, `input.conf`, scripts and shaders from mpv's default config directory (`~/.config/mpv` on macOS/Linux, `%APPDATA%\mpv` on Windows). They share one config file, sign-in and downloads, and can run at the same time.

## Features

The desktop app and the terminal client have the same features; the terminal client's differences are listed [below](#differences-from-the-desktop-app).

- **Sign in** with the plex.tv browser flow, a plex.tv/link code, email and password (with 2FA), or a direct server URL and token. Switch between Plex Home users (PIN-protected profiles supported) and between servers.
- **Server connections:** Patina tries every connection a server advertises at the same time. It prefers local, then remote, then relay.
- **Home layout:** choose Customize Home to reorder or hide rows and use a compact Continue Watching list. Layouts are saved per account and server.
- **Saved views and bulk actions (desktop):** name and save a library filter/sort combination, then restore it in one click. Select several titles to queue, download, mark watched, or add to a playlist.
- **Home:** a featured banner (step through recent and in-progress titles) plus every hub the server offers (Continue Watching, Recently Added, and so on).
- **Connection recovery:** a connection indicator reports reconnection, and failed page refreshes keep saved results visible with a Retry action.
- **Live updates:** Patina follows the server's notification stream. New and changed items appear without reloading, watch state played on other devices updates, and library scans show their progress in the sidebar.
- **Libraries:** a poster grid that keeps loading as you scroll. Sort by any server sort, combine any of the server's filters (genre, year, resolution, actor, unwatched, HDR, and so on), jump to a letter with the A–Z rail, and switch between types (shows/seasons/episodes, artists/albums/tracks). Each library also has Recommended, Collections and Folders tabs. Returning from details preserves filters, loaded results, scroll and keyboard focus. Large desktop libraries render only the visible cards and fetch the pages needed for A–Z jumps.
- **Detail pages:** backdrop art, ratings, cast, extras, related and similar titles (sonically similar music, too), seasons and episodes, albums and tracks. Pick the version, audio track and subtitle track; your choice is saved to Plex. Search for subtitles online and download them. Mark items watched, clear progress, remove them from Continue Watching, rate them, and add them to playlists, collections or your watchlist.
- **Editing (owners only):** edit titles, summaries, dates, genres and labels (edited fields are locked), choose posters and backgrounds, fix matches, and delete items.
- **Collections:** create them, add and remove items, reorder custom-sorted ones, and set how they show in the library.
- **Audio & subtitles for whole shows** (like [PASTA](https://github.com/cglatot/pasta)): from a show, season or episode, pick an audio and/or subtitle track in one episode and Patina selects the closest match in every episode of the season or show. It matches on language first (and forced vs. full subtitles), then title, codec and channels. It shows which episodes have no match before you apply, and it leaves them unchanged. You can also turn subtitles off everywhere.
- **Search** as you type, grouped by type, including people and tags.
- **Playlists** (create, add, remove, reorder by dragging, rename, clear, delete) and your plex.tv **Watchlist**. Watchlist items link to the copy on your server when there is one.
- **Downloads:** download a movie, episode, season, show, album or artist for offline playback. Downloads play in mpv even when the server can't be reached. Pause and resume in either client; interrupted transfers retry and resume after restart when the server supplies a file validator and supports byte ranges (otherwise they safely restart). Patina checks free space before transferring. Offline resume points are saved locally; watch progress syncs on reconnect to the original account/server, with newer server watch activity taking precedence. Older downloads without an account identity retain local resume points but do not sync automatically.
- **Live TV & DVR** (with a tuner): a channel guide with what's on now and next, watching channels, recording programs, and managing upcoming recordings and recording rules.
- **Activity:** live sessions (with transcode details and the option to stop a stream), watch history, and background tasks and transcoder sessions.
- **Server tools (owners only):** scan (and cancel scans), refresh, analyze or empty the trash per library, and add, edit or delete libraries and their settings. Also bandwidth and play statistics, butler tasks, database maintenance, updates, CPU and memory stats, downloading logs and database backups, a searchable editor for all server settings, and lists of accounts, friends and devices.

### Playback in mpv

- Items are played as a Plex play queue. Next episodes are queued automatically, and mpv's own next/previous keys work.
- The queue can be edited while it plays: **Play next** and **Add to queue** from any item, and reorder, remove, jump or shuffle in the queue panel. Artists, albums and tracks can start a **radio** station.
- Near the end of an episode (or in its closing credits) the app shows **Up next**, with **Play now** and **Stop after this**.
- Synced **lyrics** for tracks that have them.
- Playback resumes where you left off. Progress is reported to Plex every 10 seconds and whenever you pause or resume. Items are marked watched when they finish or pass the watched threshold.
- The audio and subtitle tracks selected in Plex are applied in mpv (matched by container stream index). External subtitle files are added too. If you switch tracks in mpv, the change is saved back to Plex.
- Intro and credits skipping uses Plex markers: press **TAB** when prompted, or turn on auto-skip.
- Quality is Original (direct play) by default. You can choose server transcoding at a range of bitrates instead.
- A now-playing bar in the app lets you play/pause, seek, skip or stop.
- What's playing shows in the **OS media controls** (Now Playing on macOS, MPRIS on Linux, SMTC on Windows), so media keys and headphones work. Optional **Discord Rich Presence**: create an application at discord.com/developers (its name is what Discord shows) and paste its ID in **Settings → Player**.

## Requirements

- Rust 1.85+ (edition 2024)
- macOS app: macOS 14+ to run; Xcode 16+ to build (the Command Line Tools alone lack SwiftUI's macros)
- Windows/Linux desktop app: Node.js 20+ and pnpm, for the web frontend
- mpv 0.38+ on `PATH` (or installed with Homebrew or as `/Applications/mpv.app`), or set its path in **Settings → Player**

## Build & run

macOS app:

```sh
macos/build.sh                              # debug build → macos/build/Patina.app
macos/build.sh --release --universal --dmg  # what releases ship
open macos/build/Patina.app
```

The script builds the backend as a static library (`crates/ffi`), then the Swift package in `macos/` against it, and bundles and signs the app (ad hoc unless `APPLE_SIGNING_IDENTITY` is set). You can also open `macos/Package.swift` in Xcode once the library is built.

Windows and Linux desktop app:

```sh
pnpm install
pnpm tauri dev      # development, with hot reload for the frontend
pnpm tauri build    # release app bundle in target/release/bundle
```

Terminal client:

```sh
cargo run -p patina-tui                  # the TUI
cargo run -p patina-tui -- libraries     # a CLI subcommand
cargo install --path crates/tui          # installs `patina-tui` into ~/.cargo/bin
```

Settings are stored in `~/.config/patina/config.toml` (or `$XDG_CONFIG_HOME/patina`; Windows: the platform config dir). Both front ends use this file, so signing in once signs in both, and settings changed in one show up in the other. Cross-process locks and atomic replacement protect settings, download state and offline progress; one process owns the download worker at a time. The file holds your auth token, so it is written with mode 0600. Downloads are listed in `downloads.json` next to it and show up in both front ends. Artwork is cached in the platform cache directory.

Logging: `PATINA_LOG=debug pnpm tauri dev`. The macOS app logs to `patina.log` in the platform data directory (`~/Library/Application Support/patina`). The terminal client logs to `patina-tui.log` in the platform data directory, never to the terminal, and reads the same `PATINA_LOG`.

## Terminal client (patina-tui)

A terminal client for the same backend: you browse your libraries in Miller columns with a details panel, and play in mpv. It also has a scriptable CLI.

### Install

```sh
cargo install --path crates/tui
```

### First run

```sh
patina-tui
```

If you have already signed in with the desktop app, it connects straight away. Otherwise it shows a code to enter at <https://plex.tv/link>. On that screen, `b` approves the sign-in in your browser instead, `p` signs in with email and password, and `m` connects to a server by URL and token. After signing in, you pick a server. To skip plex.tv from the shell:

```sh
patina-tui login --url http://192.168.1.10:32400 --token YOUR_TOKEN
```

When the server can't be reached, `d` browses and plays your downloads offline.

### Differences from the desktop app

- Artwork is not shown; items are text, with a details panel.
- There are no OS media controls; playback is controlled with the keys below or in mpv. Discord Rich Presence works as in the GUI (**Settings → Player**).
- Server statistics are text bars instead of charts.
- Sign-in, settings and downloads are shared with the desktop app. The terminal client has its own section of settings (accent colour, details panel, watch status and counts, hide watched).

### Keys

`?` shows these in the app, and the footer lists the keys for the selection.

| Key | Action |
|---|---|
| **Navigation** | |
| `j`/`k` `↑`/`↓` | Move |
| `l` `→` / `h` `←` `⌫` | Open / back |
| `g`/`G` | Top / bottom |
| `^d`/`^u` `PgDn`/`PgUp` | Half page / page |
| `Enter` | Play (resume) or open |
| **Playback** | |
| `p` | Play from start |
| `a` | Play all / from here (next-up for shows) |
| `z` | Shuffle |
| `o` | Play with options (version, tracks, quality) |
| `P` | Pause / resume the player |
| `<` / `>` | Previous / next in the queue |
| `-` / `=` | Seek back / forward 10 s |
| `X` | Stop the player |
| `n` / `N` | Play next / add to the end of the queue |
| `Q` | Play queue: jump, remove, `J`/`K` reorder, `z` shuffle |
| `E` | Stop after this (when Up next shows, `>` plays it now) |
| `Y` | Lyrics of the playing track (`Enter` seeks to a line) |
| `m` → Radio | Start radio from an artist, album or track |
| **Library** | |
| `w` / `u` | Mark watched / unwatched |
| `*` | Rate 0–10 |
| `Space` | Add to / remove from playlists |
| `x` | Delete playlist or collection · remove entry · stop stream · delete history entry |
| `J` / `K` | Move a playlist (or custom-sorted collection) entry down / up |
| `+` | Add to / remove from watchlist |
| `C` | Add to / remove from collections |
| `T` | Audio & subtitles for a whole show or season |
| `c` | Related & similar, cast & crew, genres and tags |
| `e` | Edit details, genres, labels (renames a playlist); menu: poster, background, match |
| `S` | Scan library for new files |
| `b` / `B` | Go to the season or album / the show or artist |
| `m` or `.` | Actions menu (more options) |
| **Downloads, Live TV, Activity, Server, Settings** | |
| `Enter` | A row's main action: play, pause/resume download, retry, watch, record, run, change |
| `x` | Remove a download · delete a rule · cancel a task · stop a transcode |
| `a` | Play a downloaded show, album or all movies |
| `l` | A channel's later programs · a library's settings |
| `/` | Search server settings (or any list) |
| **Finding things** | |
| `f` | Search the server, grouped by type (`Tab`: next type) |
| `/` | Filter this column |
| `s` | Sort (the server's sorts in a library) |
| `F` | Library filters (unwatched, genre, HDR…) |
| `A` | Jump to a letter (library sorted by title) |
| `t` | Switch type: shows / seasons / episodes… |
| `v` | Library views: recommended, collections, folders… |
| `H` | Hide / show watched |
| **Other** | |
| `i` | Toggle details panel |
| `[` / `]` | Scroll details |
| `O` | Open in Plex Web |
| `y` | Copy stream URL |
| `D` | Download for offline |
| `r` / `R` | Refresh view / everything |
| `U` | Switch Plex Home user |
| `L` | Sign out |
| `q` `^c` | Quit |

### Command line

```text
patina-tui                          start the TUI
patina-tui login [--url URL --token TOKEN]
patina-tui logout
patina-tui servers [--switch]       list or switch servers on your account
patina-tui info                     server details and activity
patina-tui libraries                library sections
patina-tui ls <key|/path>           an item's children, or any Plex path
patina-tui search <query>
patina-tui continue                 continue watching
patina-tui recent [-n N]            recently added
patina-tui sessions                 who is streaming now
patina-tui history [-n N]
patina-tui playlists [id]           playlists, or a playlist's items
patina-tui play <key> [--from-start] [--shuffle] [--quality original|1080p|…]
                                    plays items and containers in mpv
patina-tui watchlist [add|remove <key>]
patina-tui users [switch <name> [--pin PIN]]
patina-tui mark watched|unwatched <key>
patina-tui rate <key> [0-10]        omit the rating to clear it
patina-tui scan [section] [--force]
patina-tui url <key>                direct stream URL (contains your token)
patina-tui config-path
```

The listing commands (`libraries`, `ls`, `search`, `continue`, `recent`, `sessions`, `history`, `playlists`, `watchlist`) accept `--json` to print Plex's JSON instead of a table.

## Regression checks

```sh
cargo build --workspace
cargo test --workspace                 # includes an isolated Python mock-server download/reconnect test
cargo clippy --workspace --all-targets -- -D warnings
pnpm check
pnpm build
pnpm exec playwright install chromium # first time only
pnpm test                             # browser journeys using a development-only mock IPC fixture
# Optional: real mpv resume/completion check (also requires ffmpeg)
cargo test -p patina-core --test reliability mpv_playback_smoke -- --ignored
```

The Rust integration test starts `tools/mock_pms.py` on an ephemeral localhost port and uses a temporary config and download directory. It interrupts a transfer, pauses/resumes it, restarts the client process, and checks account-scoped offline progress replay. No real Plex server or user sign-in is used. The browser fixture at `/test/` is served by Vite during development and is not included in the production build.

## Workspace

| Crate | What it is |
|---|---|
| `crates/plex` (`patina-plex`) | Async client for the Plex Media Server and plex.tv APIs. It covers auth and PINs, resources, Plex Home, watchlist and discover, libraries, metadata editing and matching, artwork, hubs, search, playlists, collections, play queues, timeline, scrobbling and rating, stream selection, subtitles, transcoding, sessions, history, activities, butler, preferences, statistics, updates, logs and live notifications (SSE). `raw()` escape hatches cover anything else. |
| `crates/mpv` (`patina-mpv`) | Starts mpv and controls it over JSON IPC (commands, property observation, events, key bindings). |
| `crates/core` (`patina-core`) | The backend both front ends share, with no UI code: settings, sign-in and sessions, mpv playback and play queues, downloads, live server updates, Live TV, lyrics, and the heavier server queries. Front ends receive its events through an `EventSink`. |
| `crates/app` (`patina`) | The Tauri desktop app for Windows and Linux: a thin layer that exposes core to the frontend as typed commands and forwards its events to the webview. It also handles OS media controls. Tokens never reach the webview; artwork is served through the `plximg` URI scheme. |
| `web/` | The Svelte 5 + TypeScript frontend. `pnpm check` type-checks it. |
| `crates/ffi` (`patina-ffi`) | A C interface to core for the macOS app: the same commands as `crates/app`, called by name with JSON, plus core events and artwork. |
| `macos/` | The native macOS app (SwiftUI). It links `patina-ffi` and handles the OS media controls itself. |
| `crates/tui` (`patina-tui`) | The terminal client (ratatui) and its CLI. |

## Developing without a Plex server

`tools/mock_pms.py` is a fake Plex Media Server. It serves realistic JSON, artwork and a real video file, and logs every timeline, scrobble and stream-selection call:

```sh
python3 tools/mock_pms.py --media /path/to/media --port 32499

# macOS app: choose "Connect to a server directly"
XDG_CONFIG_HOME=/tmp/patina-dev macos/build/Patina.app/Contents/MacOS/Patina

# Windows/Linux desktop app: the same
XDG_CONFIG_HOME=/tmp/patina-dev pnpm tauri dev

# Terminal client
export XDG_CONFIG_HOME=/tmp/patina-dev
cargo run -p patina-tui -- login --url http://127.0.0.1:32499 --token x
cargo run -p patina-tui
```

Setting `XDG_CONFIG_HOME` keeps the mock sign-in out of your real config.

Debug builds also accept `PATINA_ROUTE=/item/100` (opens a page) and `PATINA_PLAY=<ratingKey>` (starts playback).
