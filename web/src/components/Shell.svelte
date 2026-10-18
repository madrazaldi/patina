<script lang="ts">
  // The signed-in layout: shadcn sidebar (collapses to icons with ⌘B), an inset
  // content panel with a slim top bar, and the global overlays (menus, dialogs,
  // now-playing bar).
  import {
    Activity,
    Bookmark,
    ChevronLeft,
    ChevronRight,
    ChevronsUpDown,
    Clapperboard,
    Dices,
    Download,
    Folder,
    House,
    Image as ImageIcon,
    ListMusic,
    LogOut,
    Music,
    Radio,
    Search,
    Server,
    Settings,
    Tv,
    UserRound,
  } from "@lucide/svelte";
  import type { Component, Snippet } from "svelte";
  import * as Avatar from "$lib/components/ui/avatar";
  import { Button } from "$lib/components/ui/button";
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu";
  import * as InputGroup from "$lib/components/ui/input-group";
  import { Kbd } from "$lib/components/ui/kbd";
  import { Progress } from "$lib/components/ui/progress";
  import { Separator } from "$lib/components/ui/separator";
  import * as Sidebar from "$lib/components/ui/sidebar";
  import { Spinner } from "$lib/components/ui/spinner";
  import ArtworkDialog from "./ArtworkDialog.svelte";
  import CollectionDialog from "./CollectionDialog.svelte";
  import CommandPalette from "./CommandPalette.svelte";
  import EditDialog from "./EditDialog.svelte";
  import ItemMenu from "./ItemMenu.svelte";
  import MatchDialog from "./MatchDialog.svelte";
  import NowPlaying from "./NowPlaying.svelte";
  import PlaybackFailure from "./PlaybackFailure.svelte";
  import PlaylistDialog from "./PlaylistDialog.svelte";
  import ResumeQueue from "./ResumeQueue.svelte";
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

  let searchInput: HTMLInputElement | null = $state(null);
  // The sidebar remembers whether it was collapsed. Not in the provider's cookie: WKWebView
  // drops cookies on the tauri:// scheme that release builds load from.
  let open = $state(localStorage.getItem("sidebar-open") !== "false");
  const mac = document.documentElement.classList.contains("mac");

  let session = $derived(app.session);
  let userName = $derived(session?.user?.name ?? "Server owner");

  const isLibrary = (id: string) => nav.route.name === "library" && nav.route.id === id;

  const SECTION_ICONS: Record<string, Component> = { movie: Clapperboard, show: Tv, artist: Music, photo: ImageIcon };

  // Library scans and other server work, live.
  let task = $derived(app.activities[0]);
  let scanning = $derived(new Set(app.activities.map((a) => a.sectionId).filter(Boolean)));
  let activeDownloads = $derived(app.downloads.filter((d) => d.state === "queued" || d.state === "downloading").length);

  function onSearch(value: string) {
    app.search = value;
    if (nav.route.name !== "search") nav.push({ name: "search" });
  }

  function onKey(e: KeyboardEvent) {
    if (document.querySelector("dialog[open], [role=dialog][data-state=open], [role=alertdialog][data-state=open]")) return;
    const t = e.target as HTMLElement | null;
    const typing = t && ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName);
    const mod = e.metaKey || e.ctrlKey;
    const editing = typing || !!t?.isContentEditable;
    if (mod && e.key.toLowerCase() === "k") {
      e.preventDefault();
      app.palette = true;
    } else if (mod && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "z" && !editing) {
      // Not while typing: a text field keeps its own undo.
      e.preventDefault();
      void app.undoLatest();
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
    }
  }

  // Mouse back/forward buttons.
  function onMouse(e: MouseEvent) {
    if (e.button === 3) nav.back();
    if (e.button === 4) nav.forward();
  }

  async function switchServer(id: string) {
    if (id === session?.serverId) return;
    nav.reset();
    await api.switchServer(id).catch((e) => app.error(e));
  }
</script>

<svelte:window onkeydown={onKey} onmouseup={onMouse} />

{#snippet link(route: Route, label: string, Icon: Component, active = nav.is(route), badge?: Snippet)}
  <Sidebar.MenuItem>
    <Sidebar.MenuButton isActive={active} tooltipContent={label} onclick={() => nav.push(route)}>
      <Icon />
      <span>{label}</span>
    </Sidebar.MenuButton>
    {#if badge}<Sidebar.MenuBadge>{@render badge()}</Sidebar.MenuBadge>{/if}
  </Sidebar.MenuItem>
{/snippet}

{#snippet downloadCount()}{activeDownloads}{/snippet}
{#snippet scanBadge()}<Spinner class="size-3 text-muted-foreground" />{/snippet}

<Sidebar.Provider bind:open onOpenChange={(v) => localStorage.setItem("sidebar-open", String(v))} style="--sidebar-width: 15rem" class="h-svh min-h-0 overflow-hidden">
  <Sidebar.Root variant="inset" collapsible="icon">
    <nav class="sidebar contents" aria-label="Main">
      <Sidebar.Header class={["gap-0 pb-1", mac ? "pt-9" : "pt-3"]} data-tauri-drag-region>
        <Sidebar.Menu>
          <Sidebar.MenuItem>
            <Sidebar.MenuButton size="lg" class="pointer-events-none" tabindex={-1}>
              <span class="bg-sidebar-primary text-sidebar-primary-foreground flex aspect-square size-8 items-center justify-center rounded-lg text-sm font-semibold">P</span>
              <span class="grid flex-1 text-left leading-tight">
                <span class="truncate font-semibold tracking-tight">Patina</span>
                <span class="text-muted-foreground truncate text-xs">Plex client{app.info?.version ? ` · ${app.info.version}` : ""}</span>
              </span>
            </Sidebar.MenuButton>
          </Sidebar.MenuItem>
        </Sidebar.Menu>
      </Sidebar.Header>

      <Sidebar.Content>
        <Sidebar.Group>
          <Sidebar.Menu>
            {@render link({ name: "home" }, "Home", House)}
            {@render link({ name: "search" }, "Search", Search)}
            {@render link({ name: "pick" }, "Pick something", Dices)}
            {#if app.hasTv}{@render link({ name: "watchlist" }, "Watchlist", Bookmark)}{/if}
            {@render link({ name: "playlists" }, "Playlists", ListMusic)}
            {#if app.liveTv}{@render link({ name: "livetv" }, "Live TV", Radio)}{/if}
            {#if app.downloads.length}
              {@render link({ name: "downloads" }, "Downloads", Download, undefined, activeDownloads ? downloadCount : undefined)}
            {/if}
          </Sidebar.Menu>
        </Sidebar.Group>

        {#if app.sections.length}
          <Sidebar.Group>
            <Sidebar.GroupLabel>Libraries</Sidebar.GroupLabel>
            <Sidebar.Menu>
              {#each app.sections as s (s.key)}
                {@render link(
                  { name: "library", id: s.key ?? "" },
                  s.title ?? "",
                  SECTION_ICONS[s.type ?? ""] ?? Folder,
                  isLibrary(s.key ?? ""),
                  scanning.has(s.key) ? scanBadge : undefined,
                )}
              {/each}
            </Sidebar.Menu>
          </Sidebar.Group>
        {/if}

        <Sidebar.Group class="mt-auto">
          {#if task}
            <button
              class="hover:bg-sidebar-accent mb-2 flex flex-col gap-2 rounded-lg border bg-sidebar-accent/40 px-3 py-2.5 text-left text-xs group-data-[collapsible=icon]:hidden"
              title={task.subtitle ?? task.title}
              onclick={() => nav.push({ name: "activity" })}
            >
              <span class="text-muted-foreground truncate">{task.title}</span>
              <Progress value={task.progress} max={100} class="[&>*]:bg-tint" />
            </button>
          {/if}
          <Sidebar.Menu>
            {@render link({ name: "activity" }, "Activity", Activity)}
            {#if app.owned}{@render link({ name: "server" }, "Server", Server)}{/if}
            {@render link({ name: "settings" }, "Settings", Settings)}
          </Sidebar.Menu>
        </Sidebar.Group>
      </Sidebar.Content>

      <Sidebar.Footer>
        <Sidebar.Menu>
          <Sidebar.MenuItem>
            <DropdownMenu.Root>
              <DropdownMenu.Trigger>
                {#snippet child({ props })}
                  <Sidebar.MenuButton size="lg" class="data-[state=open]:bg-sidebar-accent" {...props}>
                    <Avatar.Root class="size-8 rounded-lg after:rounded-lg">
                      {#if session?.user?.thumb}<Avatar.Image class="rounded-lg" src={img(session.user.thumb, 64, 64)} alt="" />{/if}
                      <Avatar.Fallback class="rounded-lg text-xs">{initials(userName)}</Avatar.Fallback>
                    </Avatar.Root>
                    <span class="grid flex-1 text-left leading-tight">
                      <span class="truncate font-medium">{userName}</span>
                      <span class="text-muted-foreground truncate text-xs">{session?.serverName}</span>
                    </span>
                    <ChevronsUpDown class="ml-auto" />
                  </Sidebar.MenuButton>
                {/snippet}
              </DropdownMenu.Trigger>
              <DropdownMenu.Content side="top" align="start" class="w-(--bits-dropdown-menu-anchor-width) min-w-56">
                {#if session && session.servers.length > 1}
                  <DropdownMenu.Label>Switch server</DropdownMenu.Label>
                  <DropdownMenu.RadioGroup value={session.serverId} onValueChange={switchServer}>
                    {#each session.servers as s (s.id)}
                      <DropdownMenu.RadioItem value={s.id}>{s.name}</DropdownMenu.RadioItem>
                    {/each}
                  </DropdownMenu.RadioGroup>
                  <DropdownMenu.Separator />
                {/if}
                <DropdownMenu.Item onSelect={() => nav.push({ name: "settings" })}><UserRound />Switch user…</DropdownMenu.Item>
                <DropdownMenu.Item variant="destructive" onSelect={() => api.signOut()}><LogOut />Sign out</DropdownMenu.Item>
              </DropdownMenu.Content>
            </DropdownMenu.Root>
          </Sidebar.MenuItem>
        </Sidebar.Menu>
      </Sidebar.Footer>
    </nav>
    <Sidebar.Rail />
  </Sidebar.Root>

  <Sidebar.Inset class="min-h-0 overflow-hidden">
    <header class={["topbar", { "clear-lights": mac && !open }]} data-tauri-drag-region>
      <Sidebar.Trigger class="text-muted-foreground" />
      <Separator orientation="vertical" class="mx-1.5 data-vertical:h-4 data-vertical:self-center" />
      <Button variant="ghost" size="icon-sm" class="text-muted-foreground" title="Back (⌘[)" disabled={!nav.canBack} onclick={() => nav.back()}><ChevronLeft /></Button>
      <Button variant="ghost" size="icon-sm" class="text-muted-foreground" title="Forward (⌘])" disabled={!nav.canForward} onclick={() => nav.forward()}><ChevronRight /></Button>
      <InputGroup.Root class="search ml-2 w-80">
        <InputGroup.Addon><Search /></InputGroup.Addon>
        <InputGroup.Input
          bind:ref={searchInput}
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
        <InputGroup.Addon align="inline-end"><Kbd>/</Kbd></InputGroup.Addon>
      </InputGroup.Root>
      <div class="grow" data-tauri-drag-region></div>
      <Button variant="outline" size="sm" class="text-muted-foreground gap-2" onclick={() => (app.palette = true)}>
        Commands <Kbd>⌘K</Kbd>
      </Button>
    </header>
    {#if !app.connected}
      <div class="connection" role="status">
        <span>Connection lost. Reconnecting… Saved results and downloads remain available.</span>
        <Button variant="outline" size="sm" onclick={() => { app.refresh(); api.reloadSections().catch((e) => app.error(e)); }}>Retry</Button>
      </div>
    {/if}
    <PlaybackFailure />
    <ResumeQueue />
    <div id="content" class="content">
      {@render children()}
    </div>
    <div class="player"><NowPlaying /></div>
  </Sidebar.Inset>
</Sidebar.Provider>

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
  .topbar {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 52px;
    flex: none;
    padding: 0 16px 0 12px;
    border-bottom: 1px solid var(--border);
    transition: padding 0.2s linear;
  }
  /* Room for the macOS window buttons once the sidebar collapses. */
  .topbar.clear-lights {
    padding-left: 84px;
  }
  .connection {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 8px var(--gutter);
    border-bottom: 1px solid var(--border);
    background: color-mix(in oklab, var(--destructive) 8%, transparent);
    color: var(--muted-foreground);
  }
  .content {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
  }
  .player {
    flex: none;
  }
</style>
