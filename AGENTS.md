# AGENTS.md

Guidance for coding agents working in this repository.

Patina is a Plex client with desktop and terminal front ends: the desktop app `patina` (`crates/desktop`, on GPUI and gpui-kit, over `patina-core`), `patina-tui`, a ratatui terminal client with a `--json` CLI, both built on the Rust crates (`plex`, `mpv`, `core`), and an iOS app (`ios/`, see `ios/AGENTS.md`). The desktop app and the TUI share their on-disk state (`config.toml`, `downloads.json`, offline progress, lock files). All play in mpv only. `README.md` lists the user-facing features, the TUI keys and the CLI.

## Commands

```bash
cargo build --workspace
cargo test --workspace
cargo test -p patina-tui <name_substring>   # single test
cargo clippy --workspace --all-targets      # keep it warning-free
cargo run -p patina-desktop                 # the desktop app (binary `patina`)
```

Rust unit tests live in `#[cfg(test)] mod tests` inside each source file (integration tests in `crates/*/tests/`). No test talks to a real server.

Use the mock server (`tools/mock_pms.py`) instead of a real Plex server, and an isolated config dir so you don't touch the user's sign-in:

```bash
curl -s http://127.0.0.1:32499/identity    # already running?
python3 tools/mock_pms.py --media /tmp/patina-media --port 32499 > /tmp/mock.log 2>&1 &
export XDG_CONFIG_HOME=$(mktemp -d)
target/debug/patina-tui login --url http://127.0.0.1:32499 --token x
target/debug/patina-tui                     # or drive it in tmux and capture-pane
XDG_CONFIG_HOME=$XDG_CONFIG_HOME PATINA_ROUTE=/settings cargo run -p patina-desktop
```

`PATINA_ROUTE` (debug builds) opens a page on start: `/item/<ratingKey>`, `/library/<id>`, `/playlist/<id>`, `/search?q=…`, `/downloads`, `/settings`, and so on.

`tools/mock_seerr.py` fakes a Seerr server for the Requests page (API key `mock-key`; see `README.md`).

The mock logs every timeline, scrobble and stream-selection call. Downloads go to the real downloads folder (set in Settings), so remove test downloads afterwards. Logging: `PATINA_LOG=debug` (the desktop app logs to stderr). The TUI logs to `patina-tui.log` in the platform data dir, never to the terminal.

## CI and releases

`.github/workflows/ci.yml` runs on every push to `main` and every PR: clippy with `-D warnings` and `cargo test` on Linux, macOS and Windows (Windows runs unit tests only) for the Rust crates (the desktop app included; Linux installs the GPUI system libraries), and the mpv smoke test on Linux. `CI OK` is the single required check.

To release, run `tools/release.sh 0.2.0` on a clean `main` (it bumps `version` in `[workspace.package]`, refreshes `Cargo.lock`, commits and tags `v0.2.0`), then `git push origin main v0.2.0`. `release.yml` builds `patina` (the desktop app) archives for macOS arm64 (not signed or notarized), Linux x86_64 and arm64 and Windows, `patina-tui` archives for all five targets, and `SHA256SUMS` into a draft GitHub release with notes grouped by `.github/release.yml`. A tag with a suffix (`v0.2.0-rc.1`) is a pre-release. Publish it by hand. The project is dual-licensed MIT OR Apache-2.0.

## Architecture

| Part | Role |
|---|---|
| `crates/plex` (`patina-plex`) | Async Plex Media Server and plex.tv API client and models. |
| `crates/mpv` (`patina-mpv`) | Starts mpv and drives it over JSON IPC (Rust). |
| `crates/core` (`patina-core`) | The shared Rust backend of the TUI and the desktop app, with no UI code: config, sign-in/sessions, playback and play queues, downloads, live server events, Live TV, `seerr` (Seerr client; the desktop app only) and `ops::*`. |
| `crates/tui` (`patina-tui`) | The terminal client and CLI. |
| `crates/desktop` (`patina-desktop`) | The desktop app (binary `patina`) on GPUI and gpui-kit 0.9, calling `patina-core` directly. |
| `ios/` | The iOS and iPadOS app (Swift). See `ios/AGENTS.md`. |

- **Core state.** A front end builds `AppState` with its `EventSink`. `AppState` holds the session, settings and player state. Its setters store a value and then emit a `CoreEvent` (`stage`, `session`, `now-playing`, `queue`, `activities`, `revision` (optionally preceded by `changes`, the sections and items it covers), `toast`, `downloads`, `sessions-changed`, ...) to the sink.
- **TUI sink.** `TuiSink` (`crates/tui/src/tui/mod.rs`) sends each event into the UI loop as `Msg::Core(event)`. The CLI has its own sink in `cli.rs`.
- **Shared config.** The desktop app and the TUI read and write the same `config.toml` and `downloads.json`, and may run at the same time (cross-process `flock` locks on `<file>.lock`, atomic replacement). `update_settings` re-reads the file before saving. TUI-only settings go in `Settings.tui`; the `ui` section is GUI-only.
- **Lenient deserialization** (`crates/plex/src/de.rs`). Plex changes scalar types between endpoints and versions (string vs number, bool vs `0`/`1`). Put new numeric, bool, string or ID fields through `de::opt_*` / `de::boolean`, and single-or-list arrays through `one_or_many`. Otherwise one odd field fails the whole response.

### Desktop app (`crates/desktop/src`)

- Built on GPUI (`gpui-pre`, pinned by gpui-kit) and gpui-kit 0.9. It has no command layer: pages call `patina-core` (and `patina_plex::PlexServer` through `app.server()?`) directly.
- `main.rs` starts a multi-thread tokio runtime, builds `AppState` with `GuiSink` (`core.rs`) and opens the window through `gpui_kit::open_window`. Core futures need tokio, so run them with `core::bg(fut)` and await that inside `cx.spawn`; sync file IO goes through `core::blocking`.
- `store.rs` is the app-wide `Entity<Store>` that mirrors core state from the `CoreEvent`s (session, sections, now playing, queue, activities, downloads, settings, `revision`, ...); pages observe it and reload when `revision` (narrowed with `Store::affects`) changes. `store::act` / `act_then` run an op and toast its error.
- `nav.rs` has the `Route` enum (one per page, parsed from `PATINA_ROUTE` in debug builds) and the history stack, which keeps page views alive. `pages/mod.rs` builds a page from its route; `pages/home.rs` is the reference page (`Load<T>`, stale data while reloading).
- `shell.rs` is the window: sidebar, top bar, banners, now-playing bar, key bindings (use the `cmd!` macro; GPUI rejects `secondary-`). `gate.rs` is sign-in and server choice. `widgets/` has the shared pieces (`page`, `rows`, `Card`, `CardRow`, `HubRow`, `confirm`), `menu.rs` the item menu, `dialogs/` the dialogs, `player.rs` the player bar and panels, `art.rs` artwork through `images::fetch` decoded with `image`, `theme.rs` the colours.
- Debug builds read `PATINA_ROUTE` (start page), `PATINA_BACKGROUND=1` (open without taking focus) and `PATINA_AUTOMATION=<dir>` (`automation.rs`: clicks, scrolls and keys written to `<dir>/cmd` are dispatched into the window, for end-to-end tests without moving the real pointer; GPUI stops drawing a fully covered window, so keep it partly visible). Capture the window with `screencapture -l <window id>`.
- Inside a scrolling flex column children shrink; give fixed-height children `.flex_none()`. Use `uniform_list` or gpui-kit's virtual lists for long lists.
- Scroll containers come from `widgets`: `page`/`v_scroll` for a page, `h_scroll` for a sideways row, `inner_v_scroll` (or `keep_wheel`) for a scrolling box inside a page. Plain `overflow_*_scroll` turns a wheel on one axis into a scroll on the other and moves every scroller under the pointer at once.

### TUI (`crates/tui/src`)

- `main.rs` dispatches to `cli::run` (subcommands) or `tui::run`. `fetch.rs` has listings shared by both.
- **`Item`** (`item.rs`) wraps a `patina_plex::Metadata` (it derefs to it) and adds where the entry came from (`Origin`), a `Directory` for sections, a `Link` for synthetic entries, and a `Row` for tool-screen rows. `kind()`, `children_key()` and the other navigation helpers live here.
- **Routes.** Each column lists a `Source` (`tui/source.rs`). A new screen is a `Source` variant, its arm in `source::fetch` (or `App::local_items` for local state), and a root entry in `App::root_items` (`app/tools.rs`).
- **Tool screens** (downloads, Live TV, activity, server tools, settings) are rows built in `tui/rows.rs`: a `Row` with a glyph, note, detail key/values and an `Act` that says what `Enter`, `x` and the menu do. Headings are `rows::heading`, and the cursor skips them.
- `App` (`tui/app/`) holds all state and is split by concern: `keys` (input), `nav` (columns and loading), `menu`, `playback`, `queue`, `actions`, `library`, `edit`, `tracks`, `tools`, `search`, `login`. Async work is `tokio::spawn` + a `Msg` back, or `spawn_op`/`server_op` with a `Refresh`; never block in a key handler. Listings carry a column `id` and `generation`, and stale results are dropped.
- `column.rs` handles the per-column cursor, filter, sort and hide-watched view. `modal.rs` holds dialog and picker state (`Modal`, `Menu`, `Confirm`, `Input`, `Picker`, …). `ui/` renders from `&mut App` with a `Theme`; it only adjusts scroll positions, never other state.
- **Adding a key:** handle it in `app/keys.rs`, add it to the `HELP` table in `ui/modals.rs`, and add it to the footer hints (`draw_footer`/`row_hints` in `ui/mod.rs`) if it applies to the selection. Also update the key table in `README.md`.
