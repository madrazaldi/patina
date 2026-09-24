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
  import { tick } from "svelte";
  import Card from "../components/Card.svelte";
  import HubRow from "../components/HubRow.svelte";
  import More from "../components/More.svelte";
  import Status from "../components/Status.svelte";
  import { api } from "../lib/api";
  import { plural } from "../lib/format";
  import type { Shape } from "../lib/plex";
  import { resource } from "../lib/resource.svelte";
  import { act, app, message } from "../lib/state.svelte";
  import type { FilterView, Metadata } from "../lib/types";

  let { id }: { id: string } = $props();

  const PAGE = 120;
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

  let tab = $state<"browse" | "recommended" | "collections" | "folders">("browse");

  // Browse state
  const firstType = () => TYPES[app.sections.find((s) => s.key === id)?.type ?? ""]?.[0]?.[0];
  let mediaType = $state<number | undefined>(firstType());
  let sort = $state("titleSort");
  let desc = $state(false);
  let unwatched = $state(false);
  /** Active boolean filters, by key. */
  let toggles = $state<string[]>([]);
  /** Active value filters ("Genre: Drama"). */
  let picked = $state<{ filter: string; title: string; value: string; label: string }[]>([]);

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

  let items = $state<Metadata[]>([]);
  let total = $state<number | null>(null);
  let loading = $state(false);
  let error = $state<string>();
  let generation = 0;
  let pending: Promise<void> = Promise.resolve();

  let sortValue = $derived.by(() => {
    if (!desc) return sort;
    const s = sorts.data?.find((d) => d.key === sort);
    return s?.descKey ?? `${sort}:desc`;
  });

  function load(start: number, size = PAGE) {
    const gen = generation;
    loading = true;
    pending = api
      .sectionItems(id, { kind: mediaType, sort: sortValue, unwatched, filters: pairs, start, size })
      .then(
        (mc) => {
          if (gen !== generation) return;
          total = mc.totalSize ?? mc.size ?? null;
          items.push(...mc.Metadata);
          loading = false;
        },
        (e) => {
          if (gen !== generation) return;
          error = message(e);
          loading = false;
        },
      );
    return pending;
  }

  // Reload from the top whenever the query (or watch state) changes.
  $effect(() => {
    if (tab !== "browse") return;
    [mediaType, sortValue, unwatched, pairs, app.revision];
    generation++;
    items = [];
    total = null;
    error = undefined;
    load(0);
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
  let grid: HTMLDivElement | undefined = $state();

  async function jump(index: number) {
    await pending;
    const gen = generation;
    if (index >= items.length && (total == null || items.length < total)) {
      await load(items.length, index - items.length + PAGE);
    }
    if (gen !== generation) return;
    await tick();
    grid?.children[Math.min(index, items.length - 1)]?.scrollIntoView({ block: "start", behavior: "smooth" });
  }

  const hubs = resource(() => {
    app.revision;
    return tab === "recommended" ? api.sectionHubs(id) : Promise.resolve([]);
  });
  const collections = resource(() => (tab === "collections" ? api.collections(id) : Promise.resolve([])));

  // Folders: the trail below the section root; the last step is shown.
  let trail = $state<{ key?: string; title: string }[]>([]);
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

      {#if error}
        <Status {error} />
      {:else if items.length === 0 && !loading}
        <Status title="No matches" text="Try a different filter." />
      {:else}
        <div class={["browse", { railed: railOn }]}>
          <div>
            <div class={["grid", { wide: shape === "landscape" }]} bind:this={grid}>
              {#each items as item, i (`${item.ratingKey}:${i}`)}
                <Card {item} {shape} />
              {/each}
            </div>
            {#if loading || (total != null && items.length < total)}
              <More {loading} onmore={() => load(items.length)} />
            {/if}
          </div>
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
      {#if hubs.data === undefined || hubs.loading}
        <Status loading={!hubs.error} error={hubs.error} />
      {:else if !hubs.data.some((h) => h.Metadata.length)}
        <Status title="No recommendations yet" />
      {:else}
        {#each hubs.data.filter((h) => h.Metadata.length) as hub, i (`${hub.hubIdentifier}:${i}`)}
          <HubRow {hub} />
        {/each}
      {/if}
    {:else if tab === "collections"}
      {#if collections.data === undefined || collections.loading}
        <Status loading={!collections.error} error={collections.error} />
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
      {#if !folders.data || folders.loading}
        <Status loading={!folders.error} error={folders.error} />
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
