# AGENTS.md

Guidance for coding agents working in this repository.

Patina is a Plex client with two front ends: an Electrobun desktop app (a Svelte webview in `web/` and a TypeScript backend on Bun in `desktop/`) and `patina-tui`, a ratatui terminal client with a `--json` CLI built on the Rust crates (`plex`, `mpv`, `core`, `tui`). The two share their on-disk state (`config.toml`, `downloads.json`, offline progress, lock files). All play in mpv only. `README.md` lists the user-facing features, the TUI keys and the CLI.

## Commands

```bash
cargo build --workspace
cargo test --workspace
cargo test -p patina-tui <name_substring>   # single test
cargo clippy --workspace --all-targets      # keep it warning-free

bun install
bun run check       # type-check web/ (svelte-check) and desktop/ (tsc)
bun run build       # build the webview bundle into dist/
bun run test        # Playwright journeys against web/test/bridge.ts
bun run test:desktop   # bun test desktop: the TypeScript backend
bun run dev         # build the webview and run the desktop app (Electrobun dev build)
bun run build:app   # the app bundle, in build/ and artifacts/
```

`bunx electrobun prepare` projects the Electrobun SDK into `.hutch/` (gitignored); `check`, `build`, `dev` and `build:app` run it first. Rust unit tests live in `#[cfg(test)] mod tests` inside each source file, the TypeScript ones next to the module as `*.test.ts` (integration tests in `desktop/test/`). No test talks to a real server.

Use the mock server (`tools/mock_pms.py`) instead of a real Plex server, and an isolated config dir so you don't touch the user's sign-in:

```bash
curl -s http://127.0.0.1:32499/identity    # already running?
python3 tools/mock_pms.py --media /tmp/patina-media --port 32499 > /tmp/mock.log 2>&1 &
export XDG_CONFIG_HOME=$(mktemp -d)
target/debug/patina-tui login --url http://127.0.0.1:32499 --token x
target/debug/patina-tui                     # or drive it in tmux and capture-pane
XDG_CONFIG_HOME=$XDG_CONFIG_HOME PATINA_ROUTE=/settings bun run dev
```

`PATINA_ROUTE` (dev builds, i.e. `bun run dev`) opens a page on start: `/item/<ratingKey>`, `/library/<id>`, `/playlist/<id>`, `/search?q=…`, `/downloads`, `/settings`, and so on.

The mock logs every timeline, scrobble and stream-selection call. Downloads go to the real downloads folder (set in Settings), so remove test downloads afterwards. Logging: `PATINA_LOG=debug` (the desktop backend logs to stderr). The TUI logs to `patina-tui.log` in the platform data dir, never to the terminal.

## CI and releases

`.github/workflows/ci.yml` runs on every push to `main` and every PR: clippy with `-D warnings` and `cargo test` on Linux, macOS and Windows (Windows runs unit tests only) for the Rust crates, the mpv smoke test on Linux, `bun run check`, the backend tests (`bun run test:desktop`, with mpv), `bun run build` and Playwright for the desktop app, and an Electrobun app build on all three platforms. `CI OK` is the single required check.

To release, run `tools/release.sh 0.2.0` on a clean `main` (it bumps `version` in `[workspace.package]`, refreshes `Cargo.lock`, commits and tags `v0.2.0`), then `git push origin main v0.2.0`. `release.yml` builds the Electrobun app for macOS (arm64, not signed or notarized), Linux (x86_64 and arm64) and Windows, `patina-tui` archives for all five targets, and `SHA256SUMS` into a draft GitHub release with notes grouped by `.github/release.yml`. A tag with a suffix (`v0.2.0-rc.1`) is a pre-release. Publish it by hand. The project is dual-licensed MIT OR Apache-2.0.

## Architecture

| Part | Role |
|---|---|
| `crates/plex` (`patina-plex`) | Async Plex Media Server and plex.tv API client and models (Rust, for the TUI). |
| `crates/mpv` (`patina-mpv`) | Starts mpv and drives it over JSON IPC (Rust). |
| `crates/core` (`patina-core`) | The shared Rust backend of the TUI, with no UI code: config, sign-in/sessions, playback and play queues, downloads, live server events, Live TV, and `ops::*`. |
| `crates/tui` (`patina-tui`) | The terminal client and CLI. |
| `desktop/` | The desktop backend, a TypeScript port of `plex`, `mpv` and `core` that runs on Bun inside the Electrobun main process: `desktop/plex` (Plex client), `desktop/mpv`, `desktop/core` (same module names as `crates/core/src`), `desktop/commands` (the ~140 webview commands), `desktop/shell` (RPC bridge, loopback artwork server) and `desktop/main.ts` (window and startup). |
| `web/` | The Svelte webview. |

- **The two backends are ports of each other.** The TypeScript modules keep the Rust names (camelCase) and behaviour, and read and write the same files and formats. When you change behaviour in `crates/core` or `crates/plex` that the desktop app shares (file formats, plex request shapes, retry policy), change the `desktop/` twin too, and the other way around. The desktop tests (`desktop/**/*.test.ts`) are ports of the Rust tests.
- **Core state.** The shell builds `new AppState(sink)` (`desktop/core/state.ts`). `AppState` holds the session, settings and player state. Its setters store a value and then emit a `CoreEvent` (`stage`, `session`, `now-playing`, `queue`, `activities`, `revision` (optionally preceded by `changes`, the sections and items it covers; the UI receives it as `library-changed`), `toast`, `downloads`, `sessions-changed`) to the `EventSink`.
- **Webview bridge.** `desktop/main.ts` pushes each event to the webview as an Electrobun RPC message with `eventName(event)` and `eventPayload(event)`, and `web/src/lib/state.svelte.ts` listens for them. `web/src/lib/ipc.ts` exposes `invoke(cmd, args)`, `listen(event, handler)` and `Channel` over Electrobun's browser RPC (schema in `desktop/rpc.ts`); every command lives in `desktop/commands/index.ts` with the name and argument keys `web/src/lib/api.ts` uses. Keep those names and payloads stable, or change both sides together. Commands that need the OS (opening a URL or folder) go through the `Host` the shell passes in. Tokens never reach the webview.
- **Artwork** is served to the page from a loopback HTTP server (`desktop/shell/images-server.ts`) under a random path prefix, which the shell tells the page in `window.__PATINA__.images`.
- **Tests without Electrobun.** `web/test/bridge.ts` installs `window.__PATINA_IPC__`, a fake backend, which `ipc.ts` prefers over the real RPC; Playwright drives the page at `/test/`.
- **Web UI.** `web/` uses Tailwind CSS v4 and shadcn-svelte (`components.json`). The generated components live in `web/src/lib/components/ui` (add more with `bunx shadcn-svelte@latest add <name>`) and are imported from `$lib/components/ui/<name>`. The theme (dark only) and the shared classes (`.btn`, `.rows`, `.modal`, `.menu`, …) are in `web/src/app.css`; `--tint` is the user's accent. Svelte scoped styles are unlayered, so they beat Tailwind utilities on the same element, and a class named like a Tailwind utility (`grid`, `fixed`, `table`, …) picks up that utility. Dialogs stay native `<dialog use:modal>` (`lib/dialog.ts`) and popup menus use `use:menu`. Window drag regions are the Electrobun classes `electrobun-webkit-app-region-drag` / `-no-drag`.
- **TUI sink.** `TuiSink` (`crates/tui/src/tui/mod.rs`) sends each event into the UI loop as `Msg::Core(event)`. The CLI has its own sink in `cli.rs`.
- **Shared config.** Both front ends read and write the same `config.toml` and `downloads.json`, and may run at the same time (cross-process `flock` locks on `<file>.lock`, atomic replacement). `update_settings` re-reads the file before saving. TUI-only settings go in `Settings.tui`; the `ui` section is GUI-only.
- **Lenient deserialization** (`crates/plex/src/de.rs`, `desktop/plex/de.ts`). Plex changes scalar types between endpoints and versions (string vs number, bool vs `0`/`1`). Put new numeric, bool, string or ID fields through `de::opt_*` / `de::boolean` (TS: `optString`, `optI64`, `optF64`, `boolean`), and single-or-list arrays through `one_or_many` (`oneOrMany`). Otherwise one odd field fails the whole response.

### TUI (`crates/tui/src`)

- `main.rs` dispatches to `cli::run` (subcommands) or `tui::run`. `fetch.rs` has listings shared by both.
- **`Item`** (`item.rs`) wraps a `patina_plex::Metadata` (it derefs to it) and adds where the entry came from (`Origin`), a `Directory` for sections, a `Link` for synthetic entries, and a `Row` for tool-screen rows. `kind()`, `children_key()` and the other navigation helpers live here.
- **Routes.** Each column lists a `Source` (`tui/source.rs`). A new screen is a `Source` variant, its arm in `source::fetch` (or `App::local_items` for local state), and a root entry in `App::root_items` (`app/tools.rs`).
- **Tool screens** (downloads, Live TV, activity, server tools, settings) are rows built in `tui/rows.rs`: a `Row` with a glyph, note, detail key/values and an `Act` that says what `Enter`, `x` and the menu do. Headings are `rows::heading`, and the cursor skips them.
- `App` (`tui/app/`) holds all state and is split by concern: `keys` (input), `nav` (columns and loading), `menu`, `playback`, `queue`, `actions`, `library`, `edit`, `tracks`, `tools`, `search`, `login`. Async work is `tokio::spawn` + a `Msg` back, or `spawn_op`/`server_op` with a `Refresh`; never block in a key handler. Listings carry a column `id` and `generation`, and stale results are dropped.
- `column.rs` handles the per-column cursor, filter, sort and hide-watched view. `modal.rs` holds dialog and picker state (`Modal`, `Menu`, `Confirm`, `Input`, `Picker`, …). `ui/` renders from `&mut App` with a `Theme`; it only adjusts scroll positions, never other state.
- **Adding a key:** handle it in `app/keys.rs`, add it to the `HELP` table in `ui/modals.rs`, and add it to the footer hints (`draw_footer`/`row_hints` in `ui/mod.rs`) if it applies to the selection. Also update the key table in `README.md`.
