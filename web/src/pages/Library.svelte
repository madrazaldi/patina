<script lang="ts">
  import {
    ArrowDownWideNarrow,
    ArrowUpNarrowWide,
    Check,
    ChevronLeft,
    ChevronRight,
    Folder,
    ListFilter,
    RefreshCw,
    X,
  } from "@lucide/svelte";
  import { onDestroy, untrack } from "svelte";
  import { LibraryPages, libraryMemory, rememberLibrary } from "../lib/library.svelte";
  import { Badge } from "$lib/components/ui/badge";
  import { Button } from "$lib/components/ui/button";
  import { ButtonGroup } from "$lib/components/ui/button-group";
  import { Input } from "$lib/components/ui/input";
  import * as NativeSelect from "$lib/components/ui/native-select";
  import { Separator } from "$lib/components/ui/separator";
  import { Spinner } from "$lib/components/ui/spinner";
  import VirtualGrid from "../components/VirtualGrid.svelte";
  import BulkActions from "../components/BulkActions.svelte";
  import Card from "../components/Card.svelte";
  import HubRow from "../components/HubRow.svelte";
  import Status from "../components/Status.svelte";
  import { api } from "../lib/api";
  import { plural } from "../lib/format";
  import type { Shape } from "../lib/plex";
  import { resource } from "../lib/resource.svelte";
  import { act, app } from "../lib/state.svelte";
  import type { FilterView, Metadata, SavedView } from "../lib/types";

  let { id }: { id: string } = $props();

  const scope = `${app.session?.accountId}:${app.session?.serverId}`;
  const cacheKey = untrack(() => `${scope}:${id}`);
  const saved = libraryMemory.get(cacheKey);
  const pages = saved?.pages ?? new LibraryPages();
  let focused = saved?.focus;
  let grid = $state<VirtualGrid>();
  let selecting = $state(false);
  let selection = $state<Metadata[]>([]);
  let selected = $derived(new Set(selection.map((m) => m.ratingKey ?? "")));
  function toggleItem(item: Metadata) {
    selection = selected.has(item.ratingKey ?? "") ? selection.filter((m) => m.ratingKey !== item.ratingKey) : [...selection, item];
  }
  let viewName = $state("");
  let savedViews = $derived((app.settings?.ui.saved_views ?? []).filter((v) => v.scope === scope && v.library === id));

  const TYPES: Record<string, [number, string][]> = {
    show: [
      [2, "Shows"],
      [3, "Seasons"],
      [4, "Episodes"],
    ],
    artist: [
      [8, "Artists"],
      [9, "Albums"],
      [10, "Tracks"],
    ],
  };
  const LETTERS = ["#", ..."ABCDEFGHIJKLMNOPQRSTUVWXYZ"];

  let section = $derived(app.sections.find((s) => s.key === id));
  let kind = $derived(section?.type ?? "");
  let types = $derived(TYPES[kind] ?? []);
  let watchable = $derived(kind !== "artist" && kind !== "photo");
  let scan = $derived(app.activities.find((a) => a.sectionId === id));

  let tab = $state<"browse" | "recommended" | "collections" | "folders">(saved?.tab ?? "browse");

  // Browse state
  const firstType = () => TYPES[app.sections.find((s) => s.key === id)?.type ?? ""]?.[0]?.[0];
  let mediaType = $state<number | undefined>(saved?.query.mediaType || firstType());
  let sort = $state(saved?.query.sort ?? "titleSort");
  let desc = $state(saved?.query.desc ?? false);
  let unwatched = $state(saved?.query.unwatched ?? false);
  /** Active boolean filters, by key. */
  let toggles = $state<string[]>(saved?.query.toggles ?? []);
  /** Active value filters ("Genre: Drama"). */
  let picked = $state<{ filter: string; title: string; value: string; label: string }[]>(saved?.query.picked ?? []);

  const sorts = resource(() => api.sectionSorts(id));
  const filterDefs = resource(() => api.sectionFilters(id, mediaType));
  // The Unwatched chip below covers the "unwatched" boolean filter.
  let booleans = $derived((filterDefs.data ?? []).filter((f) => f.kind === "boolean" && f.filter !== "unwatched"));
  let valueFilters = $derived((filterDefs.data ?? []).filter((f) => f.kind !== "boolean"));
  let pairs = $derived<[string, string][]>([
    ...toggles.map((f): [string, string] => [f, "1"]),
    ...picked.map((p): [string, string] => [p.filter, p.value]),
  ]);
  let filtered = $derived(unwatched || pairs.length > 0);

  let total = $derived(pages.total);
  let loading = $derived(pages.loading > 0);
  let error = $derived(pages.error);

  let sortValue = $derived.by(() => {
    if (!desc) return sort;
    const s = sorts.data?.find((d) => d.key === sort);
    return s?.descKey ?? `${sort}:desc`;
  });

  let request = $derived({ kind: mediaType, sort: sortValue, unwatched, filters: pairs });
  $effect(() => {
    if (tab !== "browse") return;
    const q = request, revision = app.revision;
    untrack(() => {
      const key = JSON.stringify(q);
      if (pages.key && pages.key !== key) { selection = []; focused = undefined; }
      pages.configure(key, revision, (start, size) => api.sectionItems(id, { ...q, start, size }), id);
    });
  });
  function viewQuery() { return { mediaType: mediaType ?? 0, sort, desc, unwatched, toggles: $state.snapshot(toggles), picked: $state.snapshot(picked) }; }
  function saveView() {
    if (!viewName.trim()) return;
    const view: SavedView = { ...viewQuery(), id: crypto.randomUUID(), name: viewName.trim(), scope, library: id };
    app.updateUi({ saved_views: [...(app.settings?.ui.saved_views ?? []), view] });
    viewName = "";
  }
  function useView(view: SavedView) {
    mediaType = view.mediaType || undefined; sort = view.sort; desc = view.desc; unwatched = view.unwatched;
    toggles = [...view.toggles]; picked = view.picked.map((p) => ({ ...p })); selection = [];
    document.getElementById("content")?.scrollTo({ top: 0 });
  }
  // The command palette can open a saved view of this library.
  $effect(() => {
    const view = app.pendingView;
    if (!view || view.library !== id || view.scope !== scope) return;
    untrack(() => {
      tab = "browse";
      useView(view);
    });
    app.pendingView = null;
  });
  onDestroy(() => {
    pages.detach();
    rememberLibrary(cacheKey, { pages, query: viewQuery(), tab, trail: $state.snapshot(trail), focus: focused });
  });

  function setType(t: number) {
    mediaType = t;
    toggles = [];
    picked = [];
  }

  function toggle(f: FilterView) {
    toggles = toggles.includes(f.filter) ? toggles.filter((k) => k !== f.filter) : [...toggles, f.filter];
  }

  let shape = $derived<Shape | undefined>(
    mediaType === 4 ? "landscape" : mediaType !== undefined && mediaType >= 8 ? "square" : undefined,
  );
  let noun = $derived(types.find((t) => t[0] === mediaType)?.[1].toLowerCase().replace(/s$/, "") ?? "item");

  // Filter picker: filter names, then the chosen filter's values.
  let picker = $state<{ x: number; y: number } | null>(null);
  let choosing = $state<FilterView | null>(null);
  let query = $state("");
  const values = resource(() =>
    choosing ? api.sectionFilterValues(id, choosing.filter, mediaType) : Promise.resolve([]),
  );
  let shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const all = values.data ?? [];
    return q ? all.filter((v) => (v.title ?? "").toLowerCase().includes(q)) : all;
  });

  function openPicker(e: MouseEvent) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    picker = { x: Math.min(r.left, window.innerWidth - 268), y: r.bottom + 6 };
    choosing = null;
    query = "";
  }

  function choose(f: FilterView) {
    choosing = f;
    query = "";
  }

  function pick(f: FilterView, value: string, label: string) {
    picker = null;
    if (picked.some((p) => p.filter === f.filter && p.value === value)) return;
    picked = [...picked, { filter: f.filter, title: f.title, value, label }];
  }

  // A–Z rail: only meaningful for an unfiltered title-ascending listing.
  let railOn = $derived(sort.startsWith("titleSort") && !desc && !filtered);
  const chars = resource(() => (tab === "browse" && railOn ? api.firstCharacters(id, mediaType) : Promise.resolve([])));
  /** Index of the first item under each letter present. */
  let starts = $derived.by(() => {
    const out = new Map<string, number>();
    let n = 0;
    for (const c of chars.data ?? []) {
      const letter = (c.title ?? c.key ?? "").toUpperCase();
      if (!out.has(letter)) out.set(letter, n);
      n += c.size ?? 0;
    }
    return out;
  });
  function jump(index: number) { void grid?.jump(index); }

  const hubs = resource(() => {
    app.revision;
    return tab === "recommended" ? api.sectionHubs(id) : Promise.resolve([]);
  });
  const collections = resource(() => (tab === "collections" ? api.collections(id) : Promise.resolve([])));

  // Folders: the trail below the section root; the last step is shown.
  let trail = $state<{ key?: string; title: string }[]>(saved?.trail ?? []);
  const folders = resource(() => {
    app.revision;
    return tab === "folders" ? api.sectionFolders(id, trail.at(-1)?.key) : Promise.resolve(null);
  });
</script>
<svelte:window onkeydown={(e) => e.key === "Escape" && (picker = null)} />

{#if !section}
  <Status title="Library not found" fill />
{:else}
  <div class="page">
    <div class="page-head">
      <div class="flex min-w-0 items-center gap-3.5">
        <h1>{section.title}</h1>
        {#if scan}
          <Badge variant="outline" class="gap-1.5 text-muted-foreground tabular-nums" title={scan.title}>
            <Spinner class="size-3" />Scanning… {Math.round(scan.progress)}%
          </Badge>
        {/if}
      </div>
      <div class="flex items-center gap-2">
        <div class="tabs">
          <button class={["tab", { active: tab === "browse" }]} onclick={() => (tab = "browse")}>Library</button>
          <button class={["tab", { active: tab === "recommended" }]} onclick={() => (tab = "recommended")}>Recommended</button>
          <button class={["tab", { active: tab === "collections" }]} onclick={() => (tab = "collections")}>Collections</button>
          <button class={["tab", { active: tab === "folders" }]} onclick={() => (tab = "folders")}>Folders</button>
        </div>
        {#if app.owned}
          <Button
            variant="outline"
            size="icon"
            title="Scan library files"
            onclick={() => act(api.sectionAction(id, "scan"), "Scanning library…")}><RefreshCw /></Button
          >
        {/if}
      </div>
    </div>

    {#if tab === "browse"}
      <div class="mb-6 space-y-3 border-b pb-4">
        <div class="flex flex-wrap items-center gap-2">
          {#if types.length > 1}
            <div class="tabs">
              {#each types as [t, label] (t)}
                <button class={["tab", { active: mediaType === t }]} onclick={() => setType(t)}>{label}</button>
              {/each}
            </div>
            <Separator orientation="vertical" class="mx-1 data-[orientation=vertical]:h-5" />
          {/if}
          <NativeSelect.Root bind:value={sort} size="sm" class="min-w-36">
            {#if sorts.data?.length}
              {#each sorts.data as s (s.key)}<NativeSelect.Option value={s.key}>{s.title}</NativeSelect.Option>{/each}
            {:else}
              <NativeSelect.Option value="titleSort">Title</NativeSelect.Option>
              <NativeSelect.Option value="addedAt">Date added</NativeSelect.Option>
              <NativeSelect.Option value="originallyAvailableAt">Release date</NativeSelect.Option>
              <NativeSelect.Option value="rating">Rating</NativeSelect.Option>
            {/if}
          </NativeSelect.Root>
          <Button variant="outline" size="icon-sm" title={desc ? "Descending" : "Ascending"} onclick={() => (desc = !desc)}>
            {#if desc}<ArrowDownWideNarrow />{:else}<ArrowUpNarrowWide />{/if}
          </Button>
          {#if valueFilters.length}
            <Button variant="outline" size="sm" class="border-dashed" onclick={openPicker}><ListFilter />Filter</Button>
          {/if}
          {#if watchable || booleans.length}<Separator orientation="vertical" class="mx-1 data-[orientation=vertical]:h-5" />{/if}
          {#if watchable}
            <Button
              variant={unwatched ? "default" : "outline"}
              size="sm"
              class={["rounded-full", { active: unwatched, "border-dashed": !unwatched }]}
              onclick={() => (unwatched = !unwatched)}>Unwatched</Button
            >
          {/if}
          {#each booleans as f (f.filter)}
            {@const on = toggles.includes(f.filter)}
            <Button
              variant={on ? "default" : "outline"}
              size="sm"
              class={["rounded-full", { active: on, "border-dashed": !on }]}
              onclick={() => toggle(f)}>{f.title}</Button
            >
          {/each}
          <Button variant="outline" size="sm" aria-pressed={selecting} class="aria-pressed:border-tint/50 aria-pressed:bg-tint/10 aria-pressed:text-tint" onclick={() => { selecting = !selecting; if (!selecting) selection = []; }}>Select titles</Button>
          <span class="flex-1"></span>
          {#if total != null}<span class="text-sm text-muted-foreground tabular-nums">{plural(total, noun)}</span>{/if}
        </div>

        {#if picked.length}
          <div class="flex flex-wrap items-center gap-2">
            {#each picked as p, i (`${p.filter}=${p.value}`)}
              <Button
                variant="secondary"
                size="sm"
                class="rounded-full"
                title="Remove filter"
                onclick={() => (picked = picked.filter((_, j) => j !== i))}
              >
                <span class="text-muted-foreground">{p.title}:</span>{p.label}<X class="text-muted-foreground" />
              </Button>
            {/each}
            <Button variant="ghost" size="sm" onclick={() => (picked = [])}>Clear</Button>
          </div>
        {/if}

        <div class="flex flex-wrap items-center gap-2">
          <Input class="h-7 w-52" aria-label="Saved view name" placeholder="Name this view" bind:value={viewName} onkeydown={(e) => e.key === "Enter" && saveView()} />
          <Button variant="outline" size="sm" disabled={!viewName.trim()} onclick={saveView}>Save view</Button>
          {#each savedViews as view (view.id)}
            <ButtonGroup>
              <Button variant="secondary" size="sm" onclick={() => useView(view)}>{view.name}</Button>
              <Button variant="secondary" size="icon-sm" title={`Remove saved view ${view.name}`} onclick={() => app.updateUi({ saved_views: app.settings!.ui.saved_views.filter((v) => v.id !== view.id) })}><X /></Button>
            </ButtonGroup>
          {/each}
        </div>
      </div>
      {#if selecting}<BulkActions items={selection} clear={() => (selection = [])} />{/if}
      {#if error}<Status {error} retry={() => pages.refresh()} stale={total !== null && total > 0} />{/if}
      {#if total === 0 && !loading}
        <Status title="No matches" text="Try a different filter." />
      {:else if total === null}
        {#if !error}<Status loading />{/if}
      {:else}
        <div class={["browse", railOn && "grid grid-cols-[1fr_auto] gap-3.5"]} aria-busy={loading}>
          <VirtualGrid bind:this={grid} total={total ?? 0} at={(index) => pages.at(index)} load={(start, end) => pages.ensure(start, end)} {shape} {selecting} {selected} toggle={toggleItem} onfocus={(index) => (focused = index)} restoreFocus={saved?.focus} />
          {#if railOn && chars.data?.length}
            <nav class="sticky top-4 -mr-6 flex flex-col self-start" aria-label="Jump to letter">
              {#each LETTERS as l (l)}
                {@const at = starts.get(l)}
                <button
                  class="h-[17px] w-[18px] cursor-pointer text-center font-mono text-[10.5px] leading-[17px] text-muted-foreground hover:text-foreground disabled:cursor-default disabled:opacity-30 disabled:hover:text-muted-foreground"
                  disabled={at === undefined}
                  onclick={() => at !== undefined && jump(at)}>{l}</button
                >
              {/each}
            </nav>
          {/if}
        </div>
      {/if}
    {:else if tab === "recommended"}
      {#if hubs.error && hubs.data !== undefined}<Status error={hubs.error} retry={() => hubs.reload()} stale />{/if}
      {#if hubs.data === undefined}
        <Status loading={!hubs.error} error={hubs.error} retry={() => hubs.reload()} />
      {:else if !hubs.data.some((h) => h.Metadata.length)}
        <Status title="No recommendations yet" />
      {:else}
        {#each hubs.data.filter((h) => h.Metadata.length) as hub, i (`${hub.hubIdentifier}:${i}`)}
          <HubRow {hub} />
        {/each}
      {/if}
    {:else if tab === "collections"}
      {#if collections.error && collections.data !== undefined}<Status error={collections.error} retry={() => collections.reload()} stale />{/if}
      {#if collections.data === undefined}
        <Status loading={!collections.error} error={collections.error} retry={() => collections.reload()} />
      {:else if collections.data.length === 0}
        <Status title="No collections" text="Collections you create in Plex show up here." />
      {:else}
        <div class="poster-grid">
          {#each collections.data as item (item.ratingKey)}<Card {item} />{/each}
        </div>
      {/if}
    {:else}
      {#if folders.error && folders.data}<Status error={folders.error} retry={() => folders.reload()} stale />{/if}
      <nav class="-ml-2.5 mb-5 flex flex-wrap items-center gap-1" aria-label="Folder path">
        <Button variant="ghost" size="sm" class={trail.length === 0 ? "text-foreground" : "text-muted-foreground"} onclick={() => (trail = [])}>{section.title}</Button>
        {#each trail as step, i (i)}
          <ChevronRight size={12} class="text-muted-foreground" />
          <Button variant="ghost" size="sm" class={i === trail.length - 1 ? "text-foreground" : "text-muted-foreground"} onclick={() => (trail = trail.slice(0, i + 1))}
            >{step.title}</Button
          >
        {/each}
      </nav>
      {#if !folders.data}
        <Status loading={!folders.error} error={folders.error} retry={() => folders.reload()} />
      {:else if folders.data.directories.length === 0 && folders.data.items.length === 0}
        <Status title="Empty folder" />
      {:else}
        {#if folders.data.directories.length}
          <div class="rows mb-8">
            {#each folders.data.directories as d, i (`${d.key}:${i}`)}
              <button
                class="list-row clickable w-full text-left"
                onclick={() => (trail = [...trail, { key: d.key, title: d.title ?? "Folder" }])}
              >
                <Folder size={15} class="text-muted-foreground" />
                <span class="t">{d.title}</span>
                <span class="end"><ChevronRight size={14} /></span>
              </button>
            {/each}
          </div>
        {/if}
        {#if folders.data.items.length}
          <div class="poster-grid">
            {#each folders.data.items as item, i (`${item.ratingKey}:${i}`)}<Card {item} />{/each}
          </div>
        {/if}
      {/if}
    {/if}
  </div>
{/if}

{#if picker}
  <div class="scrim" role="presentation" onclick={() => (picker = null)}></div>
  <div class="menu picker" style:left="{picker.x}px" style:top="{picker.y}px">
    {#if !choosing}
      {#each valueFilters as f (f.filter)}
        <button onclick={() => choose(f)}>
          <span class="grow">{f.title}</span>
          {#if picked.some((p) => p.filter === f.filter)}<span class="size-1.5 rounded-full bg-tint"></span>{/if}
          <ChevronRight size={14} />
        </button>
      {/each}
    {:else}
      {@const f = choosing}
      <button class="back" onclick={() => (choosing = null)}><ChevronLeft size={14} />{f.title}</button>
      <hr />
      {#if (values.data?.length ?? 0) > 12}
        <!-- svelte-ignore a11y_autofocus -->
        <Input class="mx-1 mt-0.5 mb-1.5 h-7 w-[calc(100%-8px)]" placeholder="Search {f.title.toLowerCase()}" autofocus bind:value={query} />
      {/if}
      <div class="max-h-80 overflow-y-auto">
        {#if values.loading}
          <div class="grid place-items-center px-2 py-3"><Spinner class="text-muted-foreground" /></div>
        {:else if values.error}
          <div class="grid place-items-center px-2 py-3 text-xs text-muted-foreground">{values.error}</div>
        {:else if shown.length === 0}
          <div class="grid place-items-center px-2 py-3 text-xs text-muted-foreground">Nothing to choose</div>
        {:else}
          {#each shown as v, i (`${v.key}:${i}`)}
            {@const on = picked.some((p) => p.filter === f.filter && p.value === v.key)}
            <button onclick={() => v.key != null && pick(f, v.key, v.title ?? v.key)}>
              <span class="grow ellipsis">{v.title}</span>
              {#if on}<Check size={14} />{/if}
            </button>
          {/each}
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  .picker {
    width: 260px;
  }
  .picker .back {
    color: var(--text-2);
    gap: 4px;
  }
</style>
