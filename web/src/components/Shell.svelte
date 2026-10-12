<script lang="ts">
  // The signed-in layout: text sidebar, slim top bar, scrolling content, and
  // the global overlays (menus, dialogs, now-playing bar).
  import { ChevronDown, ChevronLeft, ChevronRight, Search } from "@lucide/svelte";
  import type { Snippet } from "svelte";
  import ArtworkDialog from "./ArtworkDialog.svelte";
  import CollectionDialog from "./CollectionDialog.svelte";
  import CommandPalette from "./CommandPalette.svelte";
  import { menu as menunav } from "../lib/menu";
  import EditDialog from "./EditDialog.svelte";
  import Img from "./Img.svelte";
  import ItemMenu from "./ItemMenu.svelte";
  import MatchDialog from "./MatchDialog.svelte";
  import NowPlaying from "./NowPlaying.svelte";
  import PlaylistDialog from "./PlaylistDialog.svelte";
  import SubtitleDialog from "./SubtitleDialog.svelte";
  import TrackDialog from "./TrackDialog.svelte";
  import ShowPrefsDialog from "./ShowPrefsDialog.svelte";
  import KeepDialog from "./KeepDialog.svelte";
  import { api } from "../lib/api";
  import { initials } from "../lib/format";
  import { img } from "../lib/images";
  import { nav, type Route } from "../lib/router.svelte";
  import { app } from "../lib/state.svelte";

  let { children }: { children: Snippet } = $props();

  let userMenu = $state(false);
  let searchInput: HTMLInputElement | undefined = $state();

  let session = $derived(app.session);
  let userName = $derived(session?.user?.name ?? "Server owner");

  const isLibrary = (id: string) => nav.route.name === "library" && nav.route.id === id;

  // Library scans and other server work, live.
  let task = $derived(app.activities[0]);
  let scanning = $derived(new Set(app.activities.map((a) => a.sectionId).filter(Boolean)));
  let activeDownloads = $derived(app.downloads.filter((d) => d.state === "queued" || d.state === "downloading").length);

  function onSearch(value: string) {
    app.search = value;
    if (nav.route.name !== "search") nav.push({ name: "search" });
  }

  function onKey(e: KeyboardEvent) {
    if (document.querySelector("dialog[open]")) return;
    const t = e.target as HTMLElement | null;
    const typing = t && ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName);
    const mod = e.metaKey || e.ctrlKey;
    if (mod && e.key.toLowerCase() === "k") {
      e.preventDefault();
      app.palette = true;
    } else if ((e.key === "/" && !typing) || (mod && e.key === "f")) {
      e.preventDefault();
      searchInput?.focus();
      searchInput?.select();
    } else if (mod && e.key === "[") {
      nav.back();
    } else if (mod && e.key === "]") {
      nav.forward();
    } else if (e.key === "Escape") {
      app.menu = null;
      app.playlistTarget = null;
      app.collectionTarget = null;
      app.editTarget = null;
      app.artworkTarget = null;
      app.matchTarget = null;
      app.subtitleTarget = null;
      app.panel = null;
      userMenu = false;
    }
  }

  // Mouse back/forward buttons.
  function onMouse(e: MouseEvent) {
    if (e.button === 3) nav.back();
    if (e.button === 4) nav.forward();
  }

  async function switchServer(id: string) {
    userMenu = false;
    nav.reset();
    await api.switchServer(id).catch((e) => app.error(e));
  }
</script>

<svelte:window onkeydown={onKey} onmouseup={onMouse} />

{#snippet link(route: Route, label: string, active = nav.is(route), note = "")}
  <button class={["nav-item", { active }]} onclick={() => nav.push(route)}>
    <span class="ellipsis">{label}</span>{#if note}<span class="note">{note}</span>{/if}
  </button>
{/snippet}

<div class="app">
  <nav class="sidebar">
    <div class="drag" data-tauri-drag-region></div>
    <div class="brand serif" data-tauri-drag-region>Patina</div>

    <div class="group">
      {@render link({ name: "home" }, "Home")}
      {@render link({ name: "search" }, "Search")}
      {#if app.hasTv}{@render link({ name: "watchlist" }, "Watchlist")}{/if}
      {@render link({ name: "playlists" }, "Playlists")}
      {#if app.liveTv}{@render link({ name: "livetv" }, "Live TV")}{/if}
      {#if app.downloads.length}
        {@render link({ name: "downloads" }, "Downloads", undefined, activeDownloads ? String(activeDownloads) : "")}
      {/if}
    </div>

    {#if app.sections.length}
      <div class="label">Libraries</div>
      <div class="group">
        {#each app.sections as s (s.key)}
          {@render link(
            { name: "library", id: s.key ?? "" },
            s.title ?? "",
            isLibrary(s.key ?? ""),
            scanning.has(s.key) ? "·" : "",
          )}
        {/each}
      </div>
    {/if}

    <div class="spacer"></div>
    {#if task}
      <button class="task" title={task.subtitle ?? task.title} onclick={() => nav.push({ name: "activity" })}>
        <span class="ellipsis">{task.title}</span>
        <span class="bar"><i style:width="{task.progress}%"></i></span>
      </button>
    {/if}
    <div class="group">
      {@render link({ name: "activity" }, "Activity")}
      {#if app.owned}{@render link({ name: "server" }, "Server")}{/if}
      {@render link({ name: "settings" }, "Settings")}
    </div>

    <button class="user" onclick={() => (userMenu = !userMenu)}>
      <span class="avatar">
        {#if session?.user?.thumb}<Img src={img(session.user.thumb, 64, 64)} />{:else}{initials(userName)}{/if}
      </span>
      <span class="grow">
        <span class="ellipsis who">{userName}</span>
        <span class="ellipsis where">{session?.serverName}</span>
      </span>
      <ChevronDown size={14} />
    </button>
  </nav>

  <main>
    <header class="topbar" data-tauri-drag-region>
      <button class="icon-btn" title="Back (⌘[)" disabled={!nav.canBack} onclick={() => nav.back()}><ChevronLeft /></button>
      <button class="icon-btn" title="Forward (⌘])" disabled={!nav.canForward} onclick={() => nav.forward()}><ChevronRight /></button>
      <label class="search">
        <Search size={14} />
        <input
          bind:this={searchInput}
          placeholder="Search"
          value={app.search}
          oninput={(e) => onSearch(e.currentTarget.value)}
          onkeydown={(e) => {
            if (e.key === "Escape") {
              app.search = "";
              e.currentTarget.blur();
            }
          }}
        />
        <kbd>/</kbd>
      </label>
    </header>
    {#if !app.connected}
      <div class="connection" role="status">Connection lost. Reconnecting… Saved results and downloads remain available.
        <button class="btn sm" onclick={() => { app.refresh(); api.reloadSections().catch((e) => app.error(e)); }}>Retry</button>
      </div>
    {/if}
    <div id="content" class="content">
      {@render children()}
    </div>
  </main>

  <div class="player"><NowPlaying /></div>
</div>

{#if userMenu}
  <div class="scrim" role="presentation" onclick={() => (userMenu = false)}></div>
  <div class="menu" use:menunav={() => (userMenu = false)} aria-label="Account" style="left: 12px; bottom: 64px; width: 220px">
    {#if session && session.servers.length > 1}
      <div class="head"><span>Switch server</span></div>
      {#each session.servers as s (s.id)}
        <button onclick={() => switchServer(s.id)}>
          <span class="grow ellipsis">{s.name}</span>
          {#if s.id === session.serverId}<span class="faint">current</span>{/if}
        </button>
      {/each}
      <hr />
    {/if}
    <button
      onclick={() => {
        userMenu = false;
        nav.push({ name: "settings" });
      }}>Switch user…</button
    >
    <button
      class="danger"
      onclick={() => {
        userMenu = false;
        api.signOut();
      }}>Sign out</button
    >
  </div>
{/if}

<ItemMenu />
{#if app.palette}<CommandPalette />{/if}
<PlaylistDialog />
<TrackDialog />
<ShowPrefsDialog />
<KeepDialog />
<CollectionDialog />
<EditDialog />
<ArtworkDialog />
<MatchDialog />
<SubtitleDialog />

<style>
  .connection { padding: 8px var(--gutter); background: var(--surface); border-bottom: 1px solid var(--line); }
  .app {
    display: grid;
    grid-template-columns: var(--sidebar) minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr) auto;
    height: 100vh;
  }
  .sidebar {
    grid-row: 1 / span 2;
    display: flex;
    flex-direction: column;
    padding: 0 10px 10px;
    border-right: 1px solid var(--line);
    overflow-y: auto;
    background: var(--bg);
  }
  .drag {
    flex: none;
    height: 14px;
  }
  :global(.mac) .drag {
    height: 38px;
  }
  .brand {
    padding: 2px 10px 22px;
    font-size: 21px;
    letter-spacing: -0.01em;
  }
  .label {
    padding: 22px 10px 6px;
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .nav-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px 10px;
    border: 0;
    border-radius: var(--r);
    background: none;
    color: var(--text-2);
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: pointer;
  }
  .nav-item:hover {
    color: var(--text);
  }
  .note {
    margin-left: auto;
    font-size: 11.5px;
    color: var(--accent);
  }
  .task {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0 0 10px;
    padding: 6px 10px;
    border: 0;
    background: none;
    color: var(--text-3);
    font-size: 11.5px;
    text-align: left;
    cursor: pointer;
  }
  .task:hover {
    color: var(--text-2);
  }
  .task .bar {
    display: block;
    height: 2px;
    background: var(--line);
  }
  .task .bar i {
    display: block;
    height: 100%;
    background: var(--accent);
    transition: width 0.4s linear;
  }
  .nav-item.active {
    color: var(--text);
    background: rgba(255, 255, 255, 0.06);
  }
  .spacer {
    flex: 1;
    min-height: 24px;
  }
  .user {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 12px;
    padding: 10px 8px 2px;
    border: 0;
    border-top: 1px solid var(--line);
    background: none;
    color: var(--text-3);
    text-align: left;
    cursor: pointer;
  }
  .user:hover .who {
    color: var(--text);
  }
  .who,
  .where {
    display: block;
  }
  .who {
    color: var(--text-2);
    font-weight: 500;
  }
  .where {
    font-size: 11.5px;
    color: var(--text-3);
  }

  main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .topbar {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 48px;
    flex: none;
    padding: 0 calc(var(--gutter) - 8px);
    border-bottom: 1px solid var(--line);
  }
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 300px;
    height: 30px;
    margin-left: 12px;
    padding: 0 10px;
    border-radius: var(--r);
    background: var(--bg-2);
    border: 1px solid var(--line);
    color: var(--text-3);
  }
  .search:focus-within {
    border-color: var(--line-2);
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: 0;
    background: none;
    color: var(--text);
  }
  .search input::placeholder {
    color: var(--text-3);
  }
  kbd {
    font: 11px var(--mono);
    color: var(--text-3);
  }
  .content {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
  }
  .player {
    grid-column: 2;
  }
</style>
