# AGENTS.md

Guidance for coding agents working in this repository.

Patina is a Plex client in Rust (edition 2024) with three front ends on one backend: a native SwiftUI app for macOS (`macos/`), a Tauri + Svelte desktop app for Windows and Linux, and `patina-tui`, a ratatui terminal client with a `--json` CLI. All play in mpv only. `README.md` lists the user-facing features, the TUI keys and the CLI.

## Commands

```bash
cargo build --workspace
cargo test --workspace
cargo test -p patina-tui <name_substring>   # single test
cargo clippy --workspace --all-targets      # keep it warning-free

pnpm install
pnpm check          # type-check web/ (svelte-check)
pnpm tauri dev      # run the Windows/Linux desktop app

macos/build.sh      # build the macOS app → macos/build/Patina.app (needs Xcode)
```

Unit tests live in `#[cfg(test)] mod tests` inside each source file. No test talks to a real server.

Use the mock server (`tools/mock_pms.py`) instead of a real Plex server, and an isolated config dir so you don't touch the user's sign-in:

```bash
curl -s http://127.0.0.1:32499/identity    # already running?
python3 tools/mock_pms.py --media /tmp/patina-media --port 32499 > /tmp/mock.log 2>&1 &
export XDG_CONFIG_HOME=$(mktemp -d)
target/debug/patina-tui login --url http://127.0.0.1:32499 --token x
target/debug/patina-tui                     # or drive it in tmux and capture-pane
XDG_CONFIG_HOME=$XDG_CONFIG_HOME pnpm tauri dev
XDG_CONFIG_HOME=$XDG_CONFIG_HOME PATINA_ROUTE=/library/1 macos/build/Patina.app/Contents/MacOS/Patina
```

`PATINA_ROUTE` (debug builds) opens a page on start: `/item/<ratingKey>`, `/library/<id>`, `/playlist/<id>`, `/search?q=…`, `/downloads`, `/settings`, and so on. `screencapture -l <window id>` captures the macOS app's window without clicking.

The mock logs every timeline, scrobble and stream-selection call. Downloads go to the real downloads folder (set in Settings), so remove test downloads afterwards. Logging: `PATINA_LOG=debug`. The TUI logs to `patina-tui.log` in the platform data dir, never to the terminal.

## CI and releases

`.github/workflows/ci.yml` runs on every push to `main` and every PR: clippy with `-D warnings` and `cargo test` on Linux, macOS and Windows (Windows runs unit tests only), the mpv smoke test on Linux, a build of the macOS app, and `pnpm check`, `pnpm build` and Playwright for `web/`. `CI OK` is the single required check.

To release, bump `version` in `[workspace.package]`, commit, and push a matching tag (`v0.2.0`). `release.yml` builds the macOS app (a universal DMG), the Tauri bundles for Linux (x86_64 and arm64) and Windows, and `patina-tui` archives for all five targets into a draft GitHub release. Publish it by hand. The macOS app is signed and notarized only when the `APPLE_*` secrets are set, and ad-hoc signed otherwise.

macOS jobs run on `macos-latest` unless the `MACOS_RUNNER` repo variable names a self-hosted runner label (`patina-mac`, the maintainer's Mac). Delete the variable to fall back to GitHub's runners when that Mac is offline, or jobs queue until it returns.

## Architecture

| Crate | Role |
|---|---|
| `crates/plex` (`patina-plex`) | Async Plex Media Server and plex.tv API client and models. |
| `crates/mpv` (`patina-mpv`) | Starts mpv and drives it over JSON IPC. |
| `crates/core` (`patina-core`) | The shared backend, with no UI code: config, sign-in/sessions, playback and play queues, downloads, live server events, Live TV, and `ops::*` (lists, library, edit, server, playback, setup) for anything beyond a single Plex call. |
| `crates/app` (`patina`) + `web/` | The Windows/Linux desktop app. `commands.rs` holds one-line Tauri commands over core; `remote.rs` handles OS media controls and Discord. |
| `crates/ffi` (`patina-ffi`) + `macos/` | The macOS app. `patina-ffi` is a static library with a C interface (`include/patina.h`); `dispatch.rs` has the same commands as `commands.rs`, by name with JSON arguments. `macos/` is the SwiftUI app that links it. |
| `crates/tui` (`patina-tui`) | The terminal client and CLI. |

- **Core state.** A front end builds `AppState::new(sink, runtime_handle)`. `AppState` holds the session, settings and player state. Background work runs on the given tokio runtime handle (`patina_core::runtime()`), not on a UI thread. Its setters store a value and then emit a `CoreEvent` (`Stage`, `Session`, `NowPlaying`, `Queue`, `Activities`, `Revision`, `Toast`, `Downloads`, `SessionsChanged`) through `Arc<dyn EventSink>`.
- **GUI sink.** `crates/app/src/main.rs` emits each event to the webview as `event.name()` with `event.payload()`, and `web/src/lib/state.svelte.ts` listens for them. Keep those names and payloads stable, or change both sides together.
- **macOS sink.** `crates/ffi/src/lib.rs` sends each event to Swift as `event.name()` with its JSON payload; `macos/Sources/Patina/Core/AppModel.swift` handles them. It also updates Discord; the Swift side (`Core/MediaRemote.swift`) drives Now Playing and media keys.
- **TUI sink.** `TuiSink` (`crates/tui/src/tui/mod.rs`) sends each event into the UI loop as `Msg::Core(event)`. The CLI has its own sink in `cli.rs`.
- **Shared config.** All front ends read and write the same `config.toml` and `downloads.json`, and may run at the same time. `update_settings` re-reads the file before saving. TUI-only settings go in `Settings.tui`; the `ui` section is GUI-only.
- **Lenient deserialization** (`crates/plex/src/de.rs`). Plex changes scalar types between endpoints and versions (string vs number, bool vs `0`/`1`). Put new numeric, bool, string or ID fields through `de::opt_*` / `de::boolean`, and single-or-list arrays through `one_or_many`. Otherwise one odd field fails the whole response.

### TUI (`crates/tui/src`)

- `main.rs` dispatches to `cli::run` (subcommands) or `tui::run`. `fetch.rs` has listings shared by both.
- **`Item`** (`item.rs`) wraps a `patina_plex::Metadata` (it derefs to it) and adds where the entry came from (`Origin`), a `Directory` for sections, a `Link` for synthetic entries, and a `Row` for tool-screen rows. `kind()`, `children_key()` and the other navigation helpers live here.
- **Routes.** Each column lists a `Source` (`tui/source.rs`). A new screen is a `Source` variant, its arm in `source::fetch` (or `App::local_items` for local state), and a root entry in `App::root_items` (`app/tools.rs`).
- **Tool screens** (downloads, Live TV, activity, server tools, settings) are rows built in `tui/rows.rs`: a `Row` with a glyph, note, detail key/values and an `Act` that says what `Enter`, `x` and the menu do. Headings are `rows::heading`, and the cursor skips them.
- `App` (`tui/app/`) holds all state and is split by concern: `keys` (input), `nav` (columns and loading), `menu`, `playback`, `queue`, `actions`, `library`, `edit`, `tracks`, `tools`, `search`, `login`. Async work is `tokio::spawn` + a `Msg` back, or `spawn_op`/`server_op` with a `Refresh`; never block in a key handler. Listings carry a column `id` and `generation`, and stale results are dropped.
- `column.rs` handles the per-column cursor, filter, sort and hide-watched view. `modal.rs` holds dialog and picker state (`Modal`, `Menu`, `Confirm`, `Input`, `Picker`, …). `ui/` renders from `&mut App` with a `Theme`; it only adjusts scroll positions, never other state.
- **Adding a key:** handle it in `app/keys.rs`, add it to the `HELP` table in `ui/modals.rs`, and add it to the footer hints (`draw_footer`/`row_hints` in `ui/mod.rs`) if it applies to the selection. Also update the key table in `README.md`.

### macOS app (`macos/`)

- A Swift package (`Package.swift`, macOS 14+, Swift language mode 5). `CPatina` is a module map over `crates/ffi/include/patina.h`; `build.sh` builds the library with `MACOSX_DEPLOYMENT_TARGET=14.0` and passes its directory to the linker, then bundles `build/Patina.app` from `Info.plist` and `Resources/AppIcon.icns`.
- **Commands.** Adding a Tauri command means adding the same arm to `crates/ffi/src/dispatch.rs` and a wrapper to `Core/API.swift` (named as in `web/src/lib/api.ts`). Streamed progress goes out as an extra event (`tracks-progress`).
- **Models** (`Models/`) mirror `web/src/lib/types.ts`. Plex models decode leniently and `Metadata` keeps its raw JSON, so an item sent back to the backend (to play or queue) loses nothing.
- **State.** `AppModel` (`@Observable`, main actor) holds what the events carry plus navigation: a sidebar `selection` and a `Route` stack per place, the item `sheet`, and `confirmation`. Views read it from the environment; `app.revision` bumps mean refetch. `Loading` fetches and shows loading/error states.
- **Layout.** `Pages/` are the screens (`Gate*`, `Home`, `Library*`, `Item*`, …), `Sheets/` the per-item dialogs, `Views/` shared pieces (`Card`, `HubRow`, `CardGrid`, `ItemMenu`, `Artwork`, `NowPlayingBar`, `Theme`, `Mark`).
- **Design.** Quiet and editorial: New York serif titles, hairlines, artwork for colour, one user-chosen accent (`app.accent`). No gradients or glows. Use native controls, menus and sheets rather than web-style widgets.
