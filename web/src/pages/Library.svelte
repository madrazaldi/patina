<script lang="ts">
  import { ArrowDownWideNarrow, ArrowUpNarrowWide, RefreshCw } from "@lucide/svelte";
  import Card from "../components/Card.svelte";
  import HubRow from "../components/HubRow.svelte";
  import More from "../components/More.svelte";
  import Status from "../components/Status.svelte";
  import { api } from "../lib/api";
  import { plural } from "../lib/format";
  import type { Shape } from "../lib/plex";
  import { resource } from "../lib/resource.svelte";
  import { act, app, message } from "../lib/state.svelte";
  import type { Metadata } from "../lib/types";

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

  let section = $derived(app.sections.find((s) => s.key === id));
  let kind = $derived(section?.type ?? "");
  let types = $derived(TYPES[kind] ?? []);

  let tab = $state<"browse" | "recommended" | "collections">("browse");

  // Browse state
  const firstType = () => TYPES[app.sections.find((s) => s.key === id)?.type ?? ""]?.[0]?.[0];
  let mediaType = $state<number | undefined>(firstType());
  let sort = $state("titleSort");
  let desc = $state(false);
  let unwatched = $state(false);
  let genre = $state("");

  const sorts = resource(() => api.sectionSorts(id));
  const genres = resource(() => api.sectionFilterValues(id, "genre", mediaType));

  let items = $state<Metadata[]>([]);
  let total = $state<number | null>(null);
  let loading = $state(false);
  let error = $state<string>();
  let generation = 0;

  let sortValue = $derived.by(() => {
    if (!desc) return sort;
    const s = sorts.data?.find((d) => d.key === sort);
    return s?.descKey ?? `${sort}:desc`;
  });

  function load(start: number) {
    const gen = generation;
    loading = true;
    api
      .sectionItems(id, {
        kind: mediaType,
        sort: sortValue,
        unwatched,
        genre: genre || undefined,
        start,
        size: PAGE,
      })
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
  }

  // Reload from the top whenever the query (or watch state) changes.
  $effect(() => {
    if (tab !== "browse") return;
    [mediaType, sortValue, unwatched, genre, app.revision];
    generation++;
    items = [];
    total = null;
    error = undefined;
    load(0);
  });

  let shape = $derived<Shape | undefined>(
    mediaType === 4 ? "landscape" : mediaType !== undefined && mediaType >= 8 ? "square" : undefined,
  );
  let noun = $derived(types.find((t) => t[0] === mediaType)?.[1].toLowerCase().replace(/s$/, "") ?? "item");

  const hubs = resource(() => {
    app.revision;
    return tab === "recommended" ? api.sectionHubs(id) : Promise.resolve([]);
  });
  const collections = resource(() => (tab === "collections" ? api.collections(id) : Promise.resolve([])));
</script>

{#if !section}
  <Status title="Library not found" fill />
{:else}
  <div class="page">
    <div class="page-head">
      <h1>{section.title}</h1>
      <div class="row-flex">
        <div class="tabs">
          <button class={["tab", { active: tab === "browse" }]} onclick={() => (tab = "browse")}>Library</button>
          <button class={["tab", { active: tab === "recommended" }]} onclick={() => (tab = "recommended")}>Recommended</button>
          <button class={["tab", { active: tab === "collections" }]} onclick={() => (tab = "collections")}>Collections</button>
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
              <button
                class={["tab", { active: mediaType === t }]}
                onclick={() => {
                  mediaType = t;
                  genre = "";
                }}>{label}</button
              >
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
        <select class="select" bind:value={genre}>
          <option value="">All genres</option>
          {#each genres.data ?? [] as g (g.key)}<option value={g.key}>{g.title}</option>{/each}
        </select>
        {#if kind !== "artist" && kind !== "photo"}
          <button class={["chip", { active: unwatched }]} onclick={() => (unwatched = !unwatched)}>Unwatched</button>
        {/if}
        <span class="grow"></span>
        {#if total != null}<span class="faint num">{plural(total, noun)}</span>{/if}
      </div>

      {#if error}
        <Status {error} />
      {:else if items.length === 0 && !loading}
        <Status title="No matches" text="Try a different filter." />
      {:else}
        <div class={["grid", { wide: shape === "landscape" }]}>
          {#each items as item, i (`${item.ratingKey}:${i}`)}
            <Card {item} {shape} />
          {/each}
        </div>
        {#if loading || (total != null && items.length < total)}
          <More {loading} onmore={() => load(items.length)} />
        {/if}
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
    {:else}
      {#if collections.data === undefined || collections.loading}
        <Status loading={!collections.error} error={collections.error} />
      {:else if collections.data.length === 0}
        <Status title="No collections" text="Collections you create in Plex show up here." />
      {:else}
        <div class="grid">
          {#each collections.data as item (item.ratingKey)}<Card {item} />{/each}
        </div>
      {/if}
    {/if}
  </div>
{/if}

<style>
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
</style>
