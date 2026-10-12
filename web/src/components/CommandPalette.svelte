<script lang="ts">
  // Command palette (Cmd/Ctrl+K): fuzzy-filtered navigation, playback, saved views, servers and settings.
  // Rendered only while `app.palette` is set, so every open starts fresh. The input is an ARIA combobox
  // controlling a listbox; focus stays in the input and `aria-activedescendant` follows the highlighted option.
  import { tick } from "svelte";
  import { api } from "../lib/api";
  import { modal } from "../lib/dialog";
  import { match } from "../lib/fuzzy";
  import { nav, type Route } from "../lib/router.svelte";
  import { app, captureScope } from "../lib/state.svelte";
  import type { Metadata } from "../lib/types";
  import * as plex from "../lib/plex";

  interface Command {
    id: string;
    label: string;
    group: string;
    /** Extra words the filter also looks at. */
    keywords?: string;
    run: () => unknown;
  }

  let query = $state("");
  let active = $state(0);
  let playlists = $state<Metadata[]>([]);
  /** What Cmd/Ctrl+Z would undo. */
  let undoable = $state<string | null>(null);
  let list: HTMLUListElement | undefined = $state();
  const guard = captureScope();

  api.playlists().then(
    (l) => guard.valid() && (playlists = l),
    () => {},
  );
  api.undoLast().then(
    (u) => guard.valid() && (undoable = u?.label ?? null),
    () => {},
  );

  const close = () => (app.palette = false);

  async function focusSearch(text?: string) {
    if (text !== undefined) app.search = text;
    if (nav.route.name !== "search") nav.push({ name: "search" });
    await tick();
    const input = document.querySelector<HTMLInputElement>(".topbar .search input");
    input?.focus();
    input?.select();
  }

  let commands = $derived.by(() => {
    const out: Command[] = [];
    const go = (id: string, label: string, route: Route, keywords = "", group = "Go to") =>
      out.push({ id, label, group, keywords, run: () => nav.push(route) });

    go("home", "Home", { name: "home" });
    go("pick", "Pick something to watch", { name: "pick" }, "random choose decide movie night surprise me what to watch", "Actions");
    go("playlists", "Playlists", { name: "playlists" });
    if (app.hasTv) go("watchlist", "Watchlist", { name: "watchlist" }, "plex discover bookmarks");
    if (app.liveTv) go("livetv", "Live TV", { name: "livetv" }, "dvr channels guide");
    if (app.downloads.length) go("downloads", "Downloads", { name: "downloads" }, "offline");
    go("activity", "Activity", { name: "activity" }, "sessions history tasks");
    if (app.owned) go("server", "Server", { name: "server" }, "administration libraries maintenance");
    go("settings", "Settings", { name: "settings" }, "preferences player mpv appearance accent");
    if (nav.canBack) out.push({ id: "back", label: "Go back", group: "Go to", keywords: "previous page history", run: () => nav.back() });
    if (nav.canForward) out.push({ id: "forward", label: "Go forward", group: "Go to", keywords: "next page history", run: () => nav.forward() });

    for (const s of app.sections) go(`library:${s.key}`, s.title ?? "Library", { name: "library", id: s.key ?? "" }, "library section", "Libraries");
    for (const p of playlists) go(`playlist:${p.ratingKey}`, p.title ?? "Playlist", { name: "playlist", id: p.ratingKey ?? "" }, "playlist", "Playlists");

    const np = app.nowPlaying;
    if (np) {
      const control = (id: string, label: string, kind: "togglePause" | "next" | "previous" | "stop", keywords = "") =>
        out.push({ id, label, group: "Playback", keywords, run: () => api.control({ kind }).catch((e) => app.error(e)) });
      control("pause", np.paused ? "Play" : "Pause", "togglePause", "resume toggle");
      if (np.queuePos + 1 < np.queueLen) control("next", "Next", "next", "skip forward track episode");
      if (np.queuePos > 0) control("previous", "Previous", "previous", "back track episode");
      control("stop", "Stop playback", "stop", "end quit");
      if (!np.live) {
        out.push({ id: "back10", label: "Back 10 seconds", group: "Playback", keywords: "rewind seek", run: () => api.control({ kind: "seekRelative", seconds: -10 }).catch((e) => app.error(e)) });
        out.push({ id: "forward30", label: "Forward 30 seconds", group: "Playback", keywords: "skip seek", run: () => api.control({ kind: "seekRelative", seconds: 30 }).catch((e) => app.error(e)) });
      }
      if (app.queue) out.push({ id: "queue", label: "Show queue", group: "Playback", keywords: "up next list", run: () => (app.panel = "queue") });
      if (plex.kind(np.item) === "track") out.push({ id: "lyrics", label: "Show lyrics", group: "Playback", keywords: "words song", run: () => (app.panel = "lyrics") });
    }

    for (const v of app.settings?.ui.saved_views ?? []) {
      if (v.scope !== app.scope) continue;
      const library = app.sections.find((s) => s.key === v.library);
      if (!library) continue;
      out.push({
        id: `view:${v.id}`,
        label: `${library.title}: ${v.name}`,
        group: "Saved views",
        keywords: "view filter",
        run: () => {
          app.pendingView = v;
          nav.push({ name: "library", id: v.library });
        },
      });
    }

    const session = app.session;
    for (const s of session?.servers ?? []) {
      if (s.id === session?.serverId) continue;
      out.push({
        id: `server:${s.id}`,
        label: `Switch to ${s.name}`,
        group: "Servers",
        keywords: "server change",
        run: () => {
          nav.reset();
          return api.switchServer(s.id).catch((e) => app.error(e));
        },
      });
    }
    if ((session?.servers.length ?? 0) > 1) {
      out.push({
        id: "search-all",
        label: "Search all servers",
        group: "Servers",
        keywords: "find every server account copies",
        run: () => {
          app.searchAll = true;
          return focusSearch();
        },
      });
    }
    if (undoable) {
      out.push({ id: "undo", label: `Undo: ${undoable}`, group: "Actions", keywords: "revert reverse watched playlist continue watching", run: () => app.undo() });
    }
    out.push({ id: "user", label: "Switch user…", group: "Account", keywords: "profile account", run: () => nav.push({ name: "settings" }) });

    const spoilers = app.spoilers;
    out.push({ id: "spoilers", label: spoilers ? "Show spoilers" : "Hide spoilers", group: "Settings", keywords: "unwatched thumbnails summaries", run: () => app.updateUi({ hide_spoilers: !spoilers }) });
    out.push({ id: "refresh", label: "Refresh libraries", group: "Settings", keywords: "reload reconnect retry", run: () => { app.refresh(); api.reloadSections().catch((e) => app.error(e)); } });
    return out;
  });

  let filtering = $derived(query.trim() !== "");
  let results = $derived.by(() => {
    const q = query.trim();
    if (!q) return commands;
    const scored: [Command, number][] = [];
    for (const c of commands) {
      const s = match(q, c.label, `${c.keywords ?? ""} ${c.group}`);
      if (s !== null) scored.push([c, s]);
    }
    return scored.sort((a, b) => b[1] - a[1]).map(([c]) => c);
  });

  // "Search for …" is always last: what Cmd+K did before the palette existed.
  let options = $derived<Command[]>([
    ...results,
    {
      id: "search",
      label: query.trim() ? `Search for “${query.trim()}”` : "Search",
      group: "Search",
      run: () => focusSearch(query.trim() ? query.trim() : undefined),
    },
  ]);

  // The highlight stays on the first match as the query changes (and on "Search for" when nothing matches).
  $effect(() => {
    query;
    active = 0;
  });
  $effect(() => {
    active;
    options;
    tick().then(() => list?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: "nearest" }));
  });

  function run(c: Command) {
    close();
    void c.run();
  }

  function key(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
      e.preventDefault();
      // Once the dialog is gone the shell's own Cmd+K handler would open it again.
      e.stopPropagation();
      close();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      active = (active + 1) % options.length;
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      active = (active - 1 + options.length) % options.length;
    } else if (e.key === "PageDown" || e.key === "PageUp") {
      e.preventDefault();
      active = Math.min(options.length - 1, Math.max(0, active + (e.key === "PageDown" ? 8 : -8)));
    } else if (e.key === "Enter" && !e.isComposing) {
      e.preventDefault();
      const c = options[active];
      if (c) run(c);
    }
  }

  let activeId = $derived(`palette-option-${options[active]?.id.replace(/[^\w-]/g, "_")}`);
</script>

<dialog class="palette" use:modal={close} aria-label="Command palette">
  <input
    class="query"
    role="combobox"
    aria-label="Type a command or search"
    aria-expanded="true"
    aria-controls="palette-list"
    aria-autocomplete="list"
    aria-activedescendant={activeId}
    placeholder="Type a command or search…"
    autocomplete="off"
    spellcheck="false"
    data-initial-focus
    bind:value={query}
    onkeydown={key}
  />
  <ul id="palette-list" class="options" role="listbox" aria-label="Commands" bind:this={list}>
    {#each options as c, i (c.id)}
      {#if !filtering && (i === 0 || options[i - 1].group !== c.group)}
        <li class="group" role="presentation">{c.group}</li>
      {/if}
      <!-- Keyboard use goes through the combobox input above; options are not tab stops. -->
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_interactive_supports_focus -->
      <li
        id={`palette-option-${c.id.replace(/[^\w-]/g, "_")}`}
        class={["option", { on: i === active }]}
        role="option"
        aria-selected={i === active}
        onclick={() => run(c)}
        onmousemove={() => active !== i && (active = i)}
      >
        <span class="ellipsis grow">{c.label}</span>
        {#if filtering}<span class="where">{c.group}</span>{/if}
      </li>
    {/each}
  </ul>
  <div class="sr-only" role="status">{results.length} {results.length === 1 ? "command" : "commands"}</div>
  <div class="tips"><span><kbd>↑↓</kbd> move</span><span><kbd>↵</kbd> run</span><span><kbd>esc</kbd> close</span></div>
</dialog>

<style>
  dialog.palette {
    width: min(600px, calc(100vw - 48px));
    margin: 12vh auto auto;
    padding: 0;
    overflow: hidden;
    border: 1px solid var(--line-2);
    border-radius: 8px;
    background: #141414;
    color: var(--text);
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.6);
  }
  dialog.palette::backdrop {
    background: rgba(0, 0, 0, 0.55);
  }
  .query {
    display: block;
    width: 100%;
    height: 48px;
    padding: 0 18px;
    border: 0;
    border-bottom: 1px solid var(--line);
    outline: 0;
    background: none;
    color: var(--text);
    font-size: 15px;
  }
  .query::placeholder {
    color: var(--text-3);
  }
  .options {
    max-height: min(52vh, 420px);
    margin: 0;
    padding: 6px;
    overflow-y: auto;
    list-style: none;
  }
  .group {
    padding: 10px 10px 4px;
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .option {
    display: flex;
    align-items: center;
    height: 32px;
    padding: 0 10px;
    border-radius: 4px;
    color: var(--text-2);
    cursor: pointer;
  }
  .option.on {
    background: rgba(255, 255, 255, 0.08);
    color: var(--text);
  }
  .where {
    margin-left: 12px;
    font-size: 11.5px;
    color: var(--text-3);
  }
  .tips {
    display: flex;
    gap: 16px;
    padding: 8px 14px;
    border-top: 1px solid var(--line);
    font-size: 11.5px;
    color: var(--text-3);
  }
  kbd {
    font: 11px var(--mono);
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
