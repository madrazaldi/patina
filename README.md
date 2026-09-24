# Patina

A Plex desktop client written in Rust. It gives you a polished interface for browsing your libraries and plays everything in **mpv**. mpv starts normally, so it reads your own `mpv.conf`, `input.conf`, scripts and shaders from mpv's default config directory (`~/.config/mpv` on macOS/Linux, `%APPDATA%\mpv` on Windows).

## Features

- **Sign in** with the plex.tv browser flow, a plex.tv/link code, email and password (with 2FA), or a direct server URL and token. Switch between Plex Home users (PIN-protected profiles supported) and between servers.
- **Server connections:** Patina tries every connection a server advertises at the same time. It prefers local, then remote, then relay.
- **Home:** a featured banner (step through recent and in-progress titles) plus every hub the server offers (Continue Watching, Recently Added, and so on).
- **Live updates:** Patina follows the server's notification stream. New and changed items appear without reloading, watch state played on other devices updates, and library scans show their progress in the sidebar.
- **Libraries:** a poster grid that keeps loading as you scroll. Sort by any server sort, combine any of the server's filters (genre, year, resolution, actor, unwatched, HDR, and so on), jump to a letter with the A–Z rail, and switch between types (shows/seasons/episodes, artists/albums/tracks). Each library also has Recommended, Collections and Folders tabs.
- **Detail pages:** backdrop art, ratings, cast, extras, related and similar titles (sonically similar music, too), seasons and episodes, albums and tracks. Pick the version, audio track and subtitle track; your choice is saved to Plex. Search for subtitles online and download them. Mark items watched, clear progress, remove them from Continue Watching, rate them, and add them to playlists, collections or your watchlist.
- **Editing (owners only):** edit titles, summaries, dates, genres and labels (edited fields are locked), choose posters and backgrounds, fix matches, and delete items.
- **Collections:** create them, add and remove items, reorder custom-sorted ones, and set how they show in the library.
- **Audio & subtitles for whole shows** (like [PASTA](https://github.com/cglatot/pasta)): from a show, season or episode, pick an audio and/or subtitle track in one episode and Patina selects the closest match in every episode of the season or show. It matches on language first (and forced vs. full subtitles), then title, codec and channels. It shows which episodes have no match before you apply, and it leaves them unchanged. You can also turn subtitles off everywhere.
- **Search** as you type, grouped by type, including people and tags.
- **Playlists** (create, add, remove, reorder by dragging, rename, clear, delete) and your plex.tv **Watchlist**. Watchlist items link to the copy on your server when there is one.
- **Downloads:** download a movie, episode, season, show, album or artist for offline playback. Downloads play in mpv even when the server can't be reached.
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
- Node.js 20+ and pnpm (for the web frontend)
- mpv 0.38+ on `PATH`, or set its path in **Settings → Player**
- macOS or Linux (Windows should work, but hasn't been tested)

## Build & run

```sh
pnpm install
pnpm tauri dev      # development, with hot reload for the frontend
pnpm tauri build    # release app bundle in target/release/bundle
```

Settings are stored in `~/.config/patina/config.toml` (Windows: the platform config dir). The file holds your auth token, so it is written with mode 0600. Artwork is cached in the platform cache directory.

Logging: `PATINA_LOG=debug pnpm tauri dev`.

## Workspace

| Crate | What it is |
|---|---|
| `crates/plex` (`patina-plex`) | Async client for the Plex Media Server and plex.tv APIs. It covers auth and PINs, resources, Plex Home, watchlist and discover, libraries, metadata editing and matching, artwork, hubs, search, playlists, collections, play queues, timeline, scrobbling and rating, stream selection, subtitles, transcoding, sessions, history, activities, butler, preferences, statistics, updates, logs and live notifications (SSE). `raw()` escape hatches cover anything else. |
| `crates/mpv` (`patina-mpv`) | Starts mpv and controls it over JSON IPC (commands, property observation, events, key bindings). |
| `crates/app` (`patina`) | The Tauri desktop app: session, playback and every Plex call, exposed to the frontend as typed commands. Tokens never reach the webview; artwork is served through the `plximg` URI scheme. |
| `web/` | The Svelte 5 + TypeScript frontend. `pnpm check` type-checks it. |

## Developing without a Plex server

`tools/mock_pms.py` is a fake Plex Media Server. It serves realistic JSON, artwork and a real video file, and logs every timeline, scrobble and stream-selection call:

```sh
python3 tools/mock_pms.py --media /path/to/media --port 32499
XDG_CONFIG_HOME=/tmp/patina-dev pnpm tauri dev   # choose "Connect to a server directly"
```

Debug builds also accept `PATINA_ROUTE=/item/100` (opens a page) and `PATINA_PLAY=<ratingKey>` (starts playback).
