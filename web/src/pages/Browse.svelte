<script lang="ts">
  // Any server listing by key: "See all" for hubs, and tag filters (genre, actor, ...).
  import Card from "../components/Card.svelte";
  import More from "../components/More.svelte";
  import Status from "../components/Status.svelte";
  import { api } from "../lib/api";
  import { shapeFor } from "../lib/plex";
  import { message } from "../lib/state.svelte";
  import type { Metadata } from "../lib/types";

  let { path, title }: { path: string; title: string } = $props();

  const PAGE = 100;
  let items = $state<Metadata[]>([]);
  let done = $state(false);
  let loading = $state(false);
  let error = $state<string>();

  function load() {
    if (loading) return;
    error = undefined;
    loading = true;
    api.listing(path, items.length, PAGE).then(
      (mc) => {
        items.push(...mc.Metadata);
        // Some endpoints ignore paging; stop when a page comes back short.
        done = mc.Metadata.length < PAGE || (mc.totalSize != null && items.length >= mc.totalSize);
        loading = false;
      },
      (e) => {
        error = message(e);
        loading = false;
      },
    );
  }
  load();

  let wide = $derived(items.length > 0 && shapeFor(items[0]) === "landscape");
</script>

<div class="page">
  <div class="page-head"><h1>{title}</h1></div>
  {#if error}
    <Status {error} retry={load} stale={items.length > 0} />
  {/if}
  {#if items.length === 0 && !error}
    <Status loading={loading} title="Nothing here" />
  {:else if items.length > 0}
    <div class={["poster-grid", { wide }]}>
      {#each items as item, i (`${item.ratingKey}:${item.key}:${i}`)}<Card {item} />{/each}
    </div>
    {#if !done && !error}<More {loading} onmore={load} />{/if}
  {/if}
</div>
