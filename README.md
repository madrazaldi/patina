<img src="assets/mark.svg" alt="" width="48">

# Patina

A Plex client written in Rust, with its front ends on one shared backend:

- **Patina desktop**, a desktop app built with Tauri and Svelte (`web/`).
- **patina-tui**, a keyboard-driven terminal client with Miller columns, plus a scriptable CLI with `--json` output. See [Terminal client](#terminal-client-patina-tui).

All of them play everything in **mpv**. mpv starts normally, so it reads your own `mpv.conf`, `input.conf`, scripts and shaders from mpv's default config directory (`~/.config/mpv` on macOS/Linux, `%APPDATA%\mpv` on Windows). They share one config file, sign-in and downloads, and can run at the same time.

## Features

The desktop app and the terminal client have the same features; the terminal client's differences are listed [below](#differences-from-the-desktop-app).

- **Sign in** with the plex.tv browser flow, a plex.tv/link code, email and password (with 2FA), or a direct server URL and token. Switch between Plex Home users (PIN-protected profiles supported) and between servers.
- **Server connections:** Patina tries every connection a server advertises at the same time. It prefers local, then remote, then relay.
- **Home layout:** choose Customize Home to reorder or hide rows and use a compact Continue Watching list. Layouts are saved per account and server.
- **Saved views and bulk actions:** name and save a library filter/sort combination, then restore it in one click. Select several titles to queue, download, mark watched, or add to a playlist or collection. Both clients keep saved views in the same settings, so a view saved in one is in the other (the terminal client also remembers hide-watched and its `/` filter, which the desktop app ignores).
- **Command palette (desktop):** press **Cmd/Ctrl+K** to jump to any page, library or playlist, control playback (play/pause, next, previous, stop, show the queue), open a saved view, switch server or user, and change settings, all by typing a few letters. The last entry, "Search for …", takes what you typed to the search box (`/` and Cmd/Ctrl+F still focus search directly).
- **Keyboard and screen readers (desktop):** every editor and picker is a modal dialog with a named heading, initial focus, Tab that stays inside, Escape to close and focus returned to where you were. The now-playing seek bar is a slider: use the arrow keys (5 seconds), PageUp/PageDown (1 minute), Home and End. Editors close when you switch account or server, so an edit can never land on the wrong one.
- **Home:** a featured banner (step through recent and in-progress titles) plus every hub the server offers (Continue Watching, Recently Added, and so on).
- **Connection recovery:** a connection indicator reports reconnection, and failed page refreshes keep saved results visible with a Retry action.
- **Live updates:** Patina follows the server's notification stream. New and changed items appear without reloading, watch state played on other devices updates, and library scans show their progress in the sidebar. Only the library that changed refetches, starting with the pages on screen; the rest refresh when you scroll to them.
- **Libraries:** a poster grid that keeps loading as you scroll. Sort by any server sort, combine any of the server's filters (genre, year, resolution, actor, unwatched, HDR, and so on), jump to a letter with the A–Z rail, and switch between types (shows/seasons/episodes, artists/albums/tracks). Each library also has Recommended, Collections and Folders tabs. Returning from details preserves filters, loaded results, scroll and keyboard focus. Large desktop libraries render only the visible cards and fetch the pages needed for A–Z jumps. The terminal client likewise fetches only the pages around the cursor in a big library, and loads the whole library only when you filter, hide watched titles, sort locally or select all.
- **Detail pages:** backdrop art, ratings, cast, extras, related and similar titles (sonically similar music, too), seasons and episodes, albums and tracks. Pick the version, audio track and subtitle track; your choice is saved to Plex. Search for subtitles online and download them. Mark items watched, clear progress, remove them from Continue Watching, rate them, and add them to playlists, collections or your watchlist.
- **Editing (owners only):** edit titles, summaries, dates, genres and labels (edited fields are locked), choose posters and backgrounds, fix matches, and delete items.
- **Collections:** create them, add and remove items, reorder custom-sorted ones, and set how they show in the library.
- **Audio & subtitles for whole shows** (like [PASTA](https://github.com/cglatot/pasta)): from a show, season or episode, pick an audio and/or subtitle track in one episode and Patina selects the closest match in every episode of the season or show. It matches on language first (and forced vs. full subtitles), then title, codec and channels. It shows which episodes have no match before you apply, and it leaves them unchanged. You can also turn subtitles off everywhere.
- **Automatic playback preferences:** remember your audio and subtitle languages, whether subtitles are on, off or only for foreign dialogue, forced vs. full subtitles, skipping commentary and audio-description tracks, and which version to play (resolution, codec, HDR). Set them for your profile in **Settings → Automatic playback**, and override them per show from the show's menu (**Playback preferences…**) or by ticking **Remember for this show** in the audio & subtitles dialog. They are applied every time something starts playing, so episodes added later follow them too. They reuse the whole-show track matching above (language first, forced vs. full, commentary by title). A choice you make by hand in the version picker or in mpv always wins. They are stored in `playback-prefs.json` next to the config file, per account and server.
- **Smart downloads:** keep the next N unwatched episodes of a show, season or playlist downloaded (**Keep next unwatched downloaded…** on its page or menu), remove them after you watch them, and set an overall storage budget. Patina checks the policies at start, after playback, when the server reports changes and every few minutes, so new episodes are fetched on their own. Policies and the budget are managed on the Downloads page, are scoped to the account and server they were created on (they do nothing on another), and live in `smart-downloads.json`. Downloads you made yourself are never removed by a policy.
- **Search** as you type, grouped by type, including people and tags.
- **Search all servers:** with more than one server on your account, turn on **Search all servers** on the Search page (Cmd/Ctrl+K → "Search all servers", or `Ctrl-A` in the terminal client's search). Every server is asked at once (a few at a time, each with a 12 second limit) and results appear as servers answer, with a note for any that is offline, failed or too slow. Copies of the same title are grouped (by Plex guid, else by title and year), each showing its server and best quality. Choosing a title lists the copies with their files (resolution, codec, container, bitrate, size) so you can pick one to play, or open its details. An item from another server stays with that server: its details, seasons, artwork, playback and progress reports all use that server's own connection and token, and its watch state and resume point are kept apart from the connected server's. Editing, playlists, downloads and the like are only offered for the connected server. A queue plays from one server, so mixing is refused with a message.
- **Pick something to watch:** from Home, the sidebar, the command palette or the terminal client's root list. Choose what you feel like (movie, show or both; only unwatched; longest runtime; genres; content ratings; a library; year range; lowest audience rating) and get three or four random candidates, with **Reroll** (`R`) for others that skips what you have seen until everything that fits has been offered. The server does the filtering and random ordering, so large libraries stay fast.
- **Undo:** marking watched or unwatched (a single item, a whole show or season, or a selection), removing an entry from a playlist (it goes back to its original position), clearing progress and removing an item from Continue Watching show a toast with **Undo**; **Cmd/Ctrl+Z** undoes the latest one anywhere except in a text field or a dialog (`I` or `Ctrl-Z` in the terminal client). The last 25 actions are remembered for the current account and server only, and dropped when you switch either. Undo restores watch counts, resume positions and playlist order, not just the visible state.
- **Playlists** (create, add, remove, reorder by dragging, rename, clear, delete; playlists of any length are read in pages, up to 100,000 entries, and a cut-off is shown) and your plex.tv **Watchlist**. Watchlist items link to the copy on your server when there is one. Availability is checked a few titles at a time, rechecked when the library changes, and a failed check is reported with a Retry instead of showing the title as missing.
- **Downloads:** download a movie, episode, season, show, album or artist for offline playback. Downloads play in mpv even when the server can't be reached. Pause and resume in either client; interrupted transfers retry and resume after restart when the server supplies a file validator and supports byte ranges (otherwise they safely restart). Patina checks free space before transferring. Offline resume points are saved locally; watch progress syncs on reconnect to the original account/server, with newer server watch activity taking precedence. Older downloads without an account identity retain local resume points but do not sync automatically.
- **Live TV & DVR** (with a tuner): a channel guide with what's on now and next, watching channels, recording programs, and managing upcoming recordings and recording rules.
- **Activity:** live sessions (with transcode details and the option to stop a stream), watch history, and background tasks and transcoder sessions.
- **Server tools (owners only):** scan (and cancel scans), refresh, analyze or empty the trash per library, and add, edit or delete libraries and their settings. Also bandwidth and play statistics, database maintenance, scheduled tasks you can run now or switch off (with the maintenance window), updates, CPU and memory stats, downloading logs and database backups, a searchable editor for all server settings grouped like Plex groups them, and lists of accounts, friends and devices.

### Playback in mpv

- Items are played as a Plex play queue. Next episodes are queued automatically, and mpv's own next/previous keys work.
- The queue can be edited while it plays: **Play next** and **Add to queue** from any item, and reorder, remove, jump or shuffle in the queue panel. Artists, albums and tracks can start a **radio** station.
- Near the end of an episode (or in its closing credits) the app shows **Up next**, with **Play now** and **Stop after this**.
- Synced **lyrics** for tracks that have them.
- Playback resumes where you left off. Progress is reported to Plex every 10 seconds and whenever you pause or resume. Items are marked watched when they finish or pass the watched threshold.
- **Reporting never holds playback up.** Progress, pause, stop and track-choice updates go to Plex from a background worker, in order. A slow or unreachable server delays them, never mpv or the queue; progress updates are collapsed while one is in flight, and the final "stopped" update is always delivered (retried, and flushed on exit).
- **When playback fails** you get the reason (from mpv or the connection), not a silent stop, with the actions that make sense: **Try again** (from where it stopped), **another version** of the item, or a **lower quality** transcode. It covers failures before anything loads and ones mid-playback. A failure caused by the connection is retried automatically once the connection is back.
- **Player controls:** volume and mute, speed (0.25x to 4x), chapters, repeat (off, all, one) and a sleep timer (15 to 90 minutes, or at the end of the current item). They live in the player panel of the desktop app and on keys in the terminal client. Volume, mute and repeat carry over from one item to the next.
- **Diagnostics:** the player panel shows whether playback is buffering and how much is buffered, the hardware decoder in use, dropped frames, and whether the server is reached locally, remotely or through the Plex relay.
- **Recovery:** Patina keeps a local position per item (account- and server-scoped, in the platform data directory). If the connection fails before Plex hears about the last minutes, the next play continues from it, as long as Plex has not seen the item since. The play queue and its position are saved too; on the next start both front ends **offer to resume it** (nothing plays until you accept, and you can forget it). Set `PATINA_DATA_DIR` to keep this data somewhere else.
- The audio and subtitle tracks selected in Plex are applied in mpv (matched by container stream index). External subtitle files are added too. If you switch tracks in mpv, the change is saved back to Plex.
- Intro and credits skipping uses Plex markers: press **TAB** when prompted, or turn on auto-skip.
- Quality is Original (direct play) by default. You can choose server transcoding at a range of bitrates instead.
- A now-playing bar in the app lets you play/pause, seek, skip or stop.
- What's playing shows in the **OS media controls** (Now Playing on macOS, MPRIS on Linux, SMTC on Windows), so media keys and headphones work. Optional **Discord Rich Presence**: create an application at discord.com/developers (its name is what Discord shows) and paste its ID in **Settings → Player**.

## Requirements

- Rust 1.85+ (edition 2024)
- Desktop app: Bun 1.4+, for the web frontend
- mpv 0.38+ on `PATH` (or installed with Homebrew or as `/Applications/mpv.app`), or set its path in **Settings → Player**

## Build & run

Desktop app:

```sh
bun install
bun run tauri dev   # development, with hot reload for the frontend
bun run tauri build # release app bundle in target/release/bundle
```

Terminal client:

```sh
cargo run -p patina-tui                  # the TUI
cargo run -p patina-tui -- libraries     # a CLI subcommand
cargo install --path crates/tui          # installs `patina-tui` into ~/.cargo/bin
```

Settings are stored in `~/.config/patina/config.toml` (or `$XDG_CONFIG_HOME/patina`; Windows: the platform config dir). Both front ends use this file, so signing in once signs in both, and settings changed in one show up in the other. Cross-process locks and atomic replacement protect settings, download state and offline progress; one process owns the download worker at a time. The file holds your auth token, so it is written with mode 0600. Downloads are listed in `downloads.json` next to it and show up in both front ends. Artwork is cached in the platform cache directory.

Logging: `PATINA_LOG=debug bun run tauri dev`. The terminal client logs to `patina-tui.log` in the platform data directory, never to the terminal, and reads the same `PATINA_LOG`.

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
| `V` | Player panel: chapters (`Enter` jumps), buffering, decoder, dropped frames, connection. After a failed play, the retry / other version / lower quality options |
| `9` / `0` | Volume down / up 5% |
| `M` | Mute / unmute |
| `{` / `}` | Playback speed down / up 0.25x |
| `Z` | Sleep timer: off, 15, 30, 45, 60, 90 minutes, end of this item |
| `r` | Repeat off / all / one (in the queue or player panel; elsewhere `r` refreshes) |
| `m` → Radio | Start radio from an artist, album or track |
| **Library** | |
| `w` / `u` | Mark watched / unwatched |
| `I` `^z` | Undo the last watch-state change, playlist removal or Continue Watching removal (the footer shows `I undo` when there is something to undo) |
| `*` | Rate 0–10 |
| `Space` | Add to / remove from playlists |
| `x` | Delete playlist or collection · remove entry · stop stream · delete history entry |
| `J` / `K` | Move a playlist (or custom-sorted collection) entry down / up |
| `+` | Add to / remove from watchlist |
| `C` | Add to / remove from collections |
| `T` | Audio & subtitles for a whole show or season (`r` there: also remember them for the show) |
| `m`, then **Playback preferences for the show…** / **Keep next unwatched downloaded…** | Automatic audio, subtitle and version rules for a show · smart download of a show, season or playlist |
| `c` | Related & similar, cast & crew, genres and tags |
| `e` | Edit details, genres, labels (renames a playlist); menu: poster, background, match |
| `S` | Scan library for new files |
| `b` / `B` | Go to the season or album / the show or artist |
| `m` or `.` | Actions menu (more options) |
| **Selecting several items** | |
| `Tab` | Mark / unmark the item and move on (a `+` shows in the margin) |
| `^a` | Mark everything shown (a library shown a page at a time is loaded in full first) |
| `M` | While items are marked: actions for them (otherwise `M` mutes); `Esc` clears the marks |
| `n` `N` `w` `u` `D` | With items marked: queue next / at the end · mark watched / unwatched · download them all |
| `Space` `C` | With items marked: add them to a playlist / collection, or start a new one |
| `x` | With items marked inside a playlist or collection: remove them |
| **Downloads, Live TV, Activity, Server, Settings** | |
| `Enter` | A row's main action: play, pause/resume download, retry, watch, record, run, change |
| `x` | Remove a download · stop a smart download · clear a show's playback preferences · delete a rule · cancel a task · stop a transcode |
| `a` | Play a downloaded show, album or all movies |
| `l` | A channel's later programs · a library's settings |
| `/` | Search server settings (or any list) |
| **Finding things** | |
| `f` | Search the server, grouped by type (`Tab`: next type). `^a` in the search searches every server of the account: results are badged with their servers and quality, `Enter` opens the copy chooser (`Enter` plays, `p` from start, `l` browses that server's copy) |
| `r` | On **Pick something to watch** (a root entry): roll again for other candidates; `Enter` on a criterion changes it |
| `/` | Filter this column |
| `s` | Sort (the server's sorts in a library) |
| `F` | Library filters (unwatched, genre, HDR…) |
| `A` | Jump to a letter (library sorted by title) |
| `t` | Switch type: shows / seasons / episodes… |
| `v` | Library views: recommended, collections, folders… |
| `W` | Saved views of this library: apply one, save the current sort and filters, `x` deletes |
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
patina-tui search <query> [--all-servers]
                                    --all-servers: one line per title with each server's copy and quality
patina-tui pick [--kind movie|show|any] [--watched] [--max-minutes N] [--genre G]… [--content-rating R]…
          [--library id|name]… [--year-from Y] [--year-to Y] [--min-rating N] [-n COUNT] [--exclude KEYS]
                                    random candidates to watch; --exclude skips earlier picks
patina-tui continue                 continue watching
patina-tui recent [-n N]            recently added
patina-tui sessions                 who is streaming now
patina-tui history [-n N]
patina-tui playlists [id]           playlists, or a playlist's items
patina-tui play <key> [--from-start] [--shuffle] [--quality original|1080p|…]
                                    plays items and containers in mpv
patina-tui resume                   play the queue that was playing when Patina last stopped
patina-tui watchlist [add|remove <key>]
patina-tui users [switch <name> [--pin PIN]]
patina-tui mark watched|unwatched <key>
patina-tui rate <key> [0-10]        omit the rating to clear it
patina-tui scan [section] [--force]
patina-tui downloads                downloaded items
patina-tui downloads policy         smart download policies, the budget and what they use
patina-tui downloads policy set <key> [--keep N] [--keep-watched]
                                    keep the next N unwatched of a show, season or playlist
patina-tui downloads policy pause|resume <policy>
patina-tui downloads policy remove <policy> [--delete-files]
patina-tui downloads policy budget <GB|off>
patina-tui downloads policy run [--wait]
                                    fetch and remove downloads to match the policies now
patina-tui prefs [--show <key>]     automatic playback preferences of the profile, or a show
patina-tui prefs [--show <key>] set [--audio eng,jpn] [--subtitle-languages …]
          [--subtitles keep|off|always|foreign] [--forced auto|prefer|avoid|only]
          [--commentary avoid|allow] [--resolution any|4k|1080|720|sd] [--codec hevc]
          [--hdr any|prefer|avoid]
patina-tui prefs [--show <key>] clear
patina-tui url <key>                direct stream URL (contains your token)
patina-tui config-path
```

The listing commands (`libraries`, `ls`, `search`, `pick`, `continue`, `recent`, `sessions`, `history`, `playlists`, `watchlist`) accept `--json` to print Plex's JSON instead of a table. So do `downloads`, `downloads policy` (and its `run`) and `prefs`, which print Patina's own JSON.

## Regression checks

```sh
cargo build --workspace
cargo test --workspace                 # includes an isolated Python mock-server download/reconnect test
cargo clippy --workspace --all-targets -- -D warnings
bun run check
bun run build
bun x playwright install chromium     # first time only
bun run test                          # browser journeys using a development-only mock IPC fixture
# Optional: real mpv checks (also require ffmpeg and python3): resume/completion, slow or failing
# progress reports, failure recovery, controls and the saved queue
cargo test -p patina-core --test reliability -- --ignored --test-threads=1 mpv_
```

`crates/core/tests/servers.rs` starts three mock servers and checks that a search across them groups titles with each server's quality and keeps going when one is slow, that watch state, playlist and resume-point changes land on (and are undone on) the right server, and that undo history is dropped on a server change. The Rust integration test starts `tools/mock_pms.py` on an ephemeral localhost port and uses a temporary config and download directory. It interrupts a transfer, pauses/resumes it, restarts the client process, and checks account-scoped offline progress replay. No real Plex server or user sign-in is used. The browser fixture at `/test/` is served by Vite during development and is not included in the production build.

## Workspace

| Crate | What it is |
|---|---|
| `crates/plex` (`patina-plex`) | Async client for the Plex Media Server and plex.tv APIs. It covers auth and PINs, resources, Plex Home, watchlist and discover, libraries, metadata editing and matching, artwork, hubs, search, playlists, collections, play queues, timeline, scrobbling and rating, stream selection, subtitles, transcoding, sessions, history, activities, butler, preferences, statistics, updates, logs and live notifications (SSE). `raw()` escape hatches cover anything else. |
| `crates/mpv` (`patina-mpv`) | Starts mpv and controls it over JSON IPC (commands, property observation, events, key bindings). |
| `crates/core` (`patina-core`) | The backend both front ends share, with no UI code: settings, sign-in and sessions, mpv playback and play queues, downloads, live server updates, Live TV, lyrics, and the heavier server queries. Front ends receive its events through an `EventSink`. |
| `crates/app` (`patina`) | The Tauri desktop app: a thin layer that exposes core to the frontend as typed commands and forwards its events to the webview. It also handles OS media controls. Tokens never reach the webview; artwork is served through the `plximg` URI scheme. |
| `web/` | The Svelte 5 + TypeScript frontend. `bun run check` type-checks it. |
| `crates/tui` (`patina-tui`) | The terminal client (ratatui) and its CLI. |

## Developing without a Plex server

`tools/mock_pms.py` is a fake Plex Media Server. It serves realistic JSON, artwork and a real video file, and logs every timeline, scrobble and stream-selection call:

```sh
python3 tools/mock_pms.py --media /path/to/media --port 32499

# Add a 5,000-title library and a 5,000-entry playlist, to try paging
python3 tools/mock_pms.py --media /path/to/media --port 32499 --large 5000

# Desktop app: the same
XDG_CONFIG_HOME=/tmp/patina-dev bun run tauri dev

# Terminal client
export XDG_CONFIG_HOME=/tmp/patina-dev
cargo run -p patina-tui -- login --url http://127.0.0.1:32499 --token x
cargo run -p patina-tui
```

Setting `XDG_CONFIG_HOME` keeps the mock sign-in out of your real config.

To try several servers on one account, start the mock more than once. `--name`, `--machine` and `--seed` make servers that share some titles but have different libraries and file qualities (the same rating key is a different title on each), `--search-delay` makes one slow, and `--servers` has any of them answer plex.tv's account and resource lists:

```sh
S=32515:Alpha:machine-alpha,32516:Beta:machine-beta
python3 tools/mock_pms.py --media /path/to/media --port 32515 --name Alpha --machine machine-alpha --seed 0 --servers $S &
python3 tools/mock_pms.py --media /path/to/media --port 32516 --name Beta --machine machine-beta --seed 1 --servers $S &
export XDG_CONFIG_HOME=$(mktemp -d)
# Debug builds only: ask the mock instead of plex.tv for the account and its servers
export PATINA_PLEX_TV_URL=http://127.0.0.1:32515
cargo run -p patina-tui -- login --token x   # signs in "via plex.tv", then pick Alpha
cargo run -p patina-tui -- search --all-servers ember
```

Debug builds also accept `PATINA_ROUTE=/item/100` (also `/pick`, `/search`, `/library/1` and so on; opens a page) and `PATINA_PLAY=<ratingKey>` (starts playback).
