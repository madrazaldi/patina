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
{#if folders.error && folders.data}<Status error={folders.error} retry={() => folders.reload()} stale />{/if}

<svelte:window onkeydown={(e) => e.key === "Escape" && (picker = null)} />

{#if !section}
  <Status title="Library not found" fill />
{:else}
  <div class="page">
    <div class="page-head">
      <div class="title">
        <h1>{section.title}</h1>
        {#if scan}
          <span class="scanning faint num" title={scan.title}>
            <span class="spinner"></span>Scanning… {Math.round(scan.progress)}%
          </span>
        {/if}
      </div>
      <div class="row-flex">
        <div class="tabs">
          <button class={["tab", { active: tab === "browse" }]} onclick={() => (tab = "browse")}>Library</button>
          <button class={["tab", { active: tab === "recommended" }]} onclick={() => (tab = "recommended")}>Recommended</button>
          <button class={["tab", { active: tab === "collections" }]} onclick={() => (tab = "collections")}>Collections</button>
          <button class={["tab", { active: tab === "folders" }]} onclick={() => (tab = "folders")}>Folders</button>
        </div>
        {#if app.owned}
          <button
            class="icon-btn"
            title="Scan library files"
            onclick={() => act(api.sectionAction(id, "scan"), "Scanning library…")}><RefreshCw /></button
          >
        {/if}
      </div>
    </div>

    {#if tab === "browse"}
      <div class="toolbar">
        {#if types.length > 1}
          <div class="tabs">
            {#each types as [t, label] (t)}
              <button class={["tab", { active: mediaType === t }]} onclick={() => setType(t)}>{label}</button>
            {/each}
          </div>
          <span class="sep"></span>
        {/if}
        <select class="select" bind:value={sort}>
          {#if sorts.data?.length}
            {#each sorts.data as s (s.key)}<option value={s.key}>{s.title}</option>{/each}
          {:else}
            <option value="titleSort">Title</option>
            <option value="addedAt">Date added</option>
            <option value="originallyAvailableAt">Release date</option>
            <option value="rating">Rating</option>
          {/if}
        </select>
        <button class="icon-btn" title={desc ? "Descending" : "Ascending"} onclick={() => (desc = !desc)}>
          {#if desc}<ArrowDownWideNarrow />{:else}<ArrowUpNarrowWide />{/if}
        </button>
        {#if valueFilters.length}
          <button class="btn quiet sm" onclick={openPicker}><ListFilter size={14} />Filter</button>
        {/if}
        {#if watchable || booleans.length}<span class="sep"></span>{/if}
        {#if watchable}
          <button class={["chip", { active: unwatched }]} onclick={() => (unwatched = !unwatched)}>Unwatched</button>
        {/if}
        {#each booleans as f (f.filter)}
          <button class={["chip", { active: toggles.includes(f.filter) }]} onclick={() => toggle(f)}>{f.title}</button>
        {/each}
        <button class="btn sm" aria-pressed={selecting} onclick={() => { selecting = !selecting; if (!selecting) selection = []; }}>Select titles</button>
        <span class="grow"></span>
        {#if total != null}<span class="faint num">{plural(total, noun)}</span>{/if}

        {#if picked.length}
          <div class="active">
            {#each picked as p, i (`${p.filter}=${p.value}`)}
              <button
                class="chip"
                title="Remove filter"
                onclick={() => (picked = picked.filter((_, j) => j !== i))}
              >
                <span class="faint">{p.title}:</span>{p.label}<X size={12} />
              </button>
            {/each}
            <button class="link clear" onclick={() => (picked = [])}>Clear</button>
          </div>
        {/if}
      </div>

      <div class="saved-views">
        <input class="input" aria-label="Saved view name" placeholder="Name this view" bind:value={viewName} onkeydown={(e) => e.key === "Enter" && saveView()} />
        <button class="btn sm" disabled={!viewName.trim()} onclick={saveView}>Save view</button>
        {#each savedViews as view (view.id)}
          <span class="row-flex">
            <button class="chip" onclick={() => useView(view)}>{view.name}</button>
            <button class="icon-btn" title={`Remove saved view ${view.name}`} onclick={() => app.updateUi({ saved_views: app.settings!.ui.saved_views.filter((v) => v.id !== view.id) })}><X size={12} /></button>
          </span>
        {/each}
      </div>
      {#if selecting}<BulkActions items={selection} clear={() => (selection = [])} />{/if}
      {#if error}<Status {error} retry={() => pages.refresh()} stale={total !== null && total > 0} />{/if}
      {#if total === 0 && !loading}
        <Status title="No matches" text="Try a different filter." />
      {:else if total === null}
        {#if !error}<Status loading />{/if}
      {:else}
        <div class={["browse", { railed: railOn }]} aria-busy={loading}>
          <VirtualGrid bind:this={grid} total={total ?? 0} at={(index) => pages.at(index)} load={(start, end) => pages.ensure(start, end)} {shape} {selecting} {selected} toggle={toggleItem} onfocus={(index) => (focused = index)} restoreFocus={saved?.focus} />
          {#if railOn && chars.data?.length}
            <nav class="rail" aria-label="Jump to letter">
              {#each LETTERS as l (l)}
                {@const at = starts.get(l)}
                <button disabled={at === undefined} onclick={() => at !== undefined && jump(at)}>{l}</button>
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
        <div class="grid">
          {#each collections.data as item (item.ratingKey)}<Card {item} />{/each}
        </div>
      {/if}
    {:else}
      <nav class="crumbs" aria-label="Folder path">
        <button class={{ current: trail.length === 0 }} onclick={() => (trail = [])}>{section.title}</button>
        {#each trail as step, i (i)}
          <ChevronRight size={12} />
          <button class={{ current: i === trail.length - 1 }} onclick={() => (trail = trail.slice(0, i + 1))}
            >{step.title}</button
          >
        {/each}
      </nav>
      {#if !folders.data}
        <Status loading={!folders.error} error={folders.error} retry={() => folders.reload()} />
      {:else if folders.data.directories.length === 0 && folders.data.items.length === 0}
        <Status title="Empty folder" />
      {:else}
        {#if folders.data.directories.length}
          <div class="rows folders">
            {#each folders.data.directories as d, i (`${d.key}:${i}`)}
              <button
                class="list-row clickable"
                onclick={() => (trail = [...trail, { key: d.key, title: d.title ?? "Folder" }])}
              >
                <Folder size={15} />
                <span class="t">{d.title}</span>
                <span class="end"><ChevronRight size={14} /></span>
              </button>
            {/each}
          </div>
        {/if}
        {#if folders.data.items.length}
          <div class="grid">
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
          {#if picked.some((p) => p.filter === f.filter)}<span class="dot"></span>{/if}
          <ChevronRight size={14} />
        </button>
      {/each}
    {:else}
      {@const f = choosing}
      <button class="back" onclick={() => (choosing = null)}><ChevronLeft size={14} />{f.title}</button>
      <hr />
      {#if (values.data?.length ?? 0) > 12}
        <!-- svelte-ignore a11y_autofocus -->
        <input class="input search" placeholder="Search {f.title.toLowerCase()}" autofocus bind:value={query} />
      {/if}
      <div class="values">
        {#if values.loading}
          <div class="pad"><div class="spinner"></div></div>
        {:else if values.error}
          <div class="pad faint">{values.error}</div>
        {:else if shown.length === 0}
          <div class="pad faint">Nothing to choose</div>
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
  .saved-views { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin-bottom: 18px; }
  .title {
    display: flex;
    align-items: baseline;
    gap: 14px;
    min-width: 0;
  }
  .scanning {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 12px;
    white-space: nowrap;
  }
  .scanning .spinner {
    width: 11px;
    height: 11px;
    border-width: 1px;
  }
  .toolbar {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
    margin: -8px 0 26px;
    padding-bottom: 14px;
    border-bottom: 1px solid var(--line);
  }
  .toolbar .tabs .tab {
    padding: 4px 0;
  }
  .sep {
    width: 1px;
    height: 18px;
    margin: 0 6px;
    background: var(--line-2);
  }
  .active {
    display: flex;
    flex-basis: 100%;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .active .chip {
    color: var(--text);
    gap: 5px;
  }
  .active .chip :global(svg) {
    color: var(--text-3);
  }
  .clear {
    margin-left: 4px;
    font-size: 12px;
  }

  .browse.railed {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 14px;
  }
  .browse :global(.grid > *) {
    scroll-margin-top: 24px;
  }
  .rail {
    position: sticky;
    top: 16px;
    align-self: start;
    display: flex;
    flex-direction: column;
    margin-right: -24px;
  }
  .rail button {
    width: 18px;
    height: 17px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-3);
    font: 10.5px/17px var(--mono);
    text-align: center;
    cursor: pointer;
  }
  .rail button:hover:not(:disabled) {
    color: var(--text);
  }
  .rail button:disabled {
    opacity: 0.3;
    cursor: default;
  }

  .crumbs {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    margin: -8px 0 20px;
    padding-bottom: 14px;
    border-bottom: 1px solid var(--line);
    color: var(--text-3);
  }
  .crumbs button {
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-2);
    cursor: pointer;
  }
  .crumbs button:hover {
    color: var(--text);
  }
  .crumbs button.current {
    color: var(--text);
    cursor: default;
  }
  .folders {
    margin: -20px 0 32px;
  }
  .folders > :first-child {
    border-top: 0;
  }
  .folders button {
    width: 100%;
    min-height: 40px;
    border-left: 0;
    border-right: 0;
    border-bottom: 0;
    background: none;
    color: var(--text);
    text-align: left;
  }
  .folders button :global(svg) {
    color: var(--text-3);
  }

  .picker {
    width: 260px;
  }
  .picker .back {
    color: var(--text-2);
    gap: 4px;
  }
  .picker .search {
    width: calc(100% - 8px);
    height: 28px;
    margin: 2px 4px 5px;
  }
  .values {
    max-height: 320px;
    overflow-y: auto;
  }
  .pad {
    display: grid;
    place-items: center;
    padding: 12px 9px;
    font-size: 12px;
  }
  .dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--accent);
  }
</style>
