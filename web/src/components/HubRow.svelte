<script lang="ts">
  // A Plex hub (Continue Watching, Recently Added, ...) rendered as a row.
  import { play } from "../lib/actions.svelte";
  import { duration } from "../lib/format";
  import Row from "./Row.svelte";
  import { browseRoute } from "../lib/router.svelte";
  import type { Hub } from "../lib/types";

  let { hub, compact = false }: { hub: Hub; compact?: boolean } = $props();

  let ident = $derived((hub.hubIdentifier ?? "").toLowerCase());
  let continueStyle = $derived(["continue", "ondeck", "inprogress"].some((s) => ident.includes(s)));
  let landscape = $derived(continueStyle || hub.type === "clip" || hub.type === "episode");
  let title = $derived(hub.title ?? "");
  let more = $derived(hub.more && hub.key ? browseRoute(hub.key, title) : undefined);
</script>

{#if compact && continueStyle}
  <section><div class="section-head"><h2>{title}</h2></div>
    <div class="rows">{#each hub.Metadata as item (item.ratingKey)}
      <button class="list-row compact clickable" onclick={() => play(item)}>
        <span class="grow">{item.grandparentTitle ? `${item.grandparentTitle} · ` : ""}{item.title}</span>
        <span class="faint">{duration(Math.max(0, (item.duration ?? 0) - (item.viewOffset ?? 0)))} left</span>
        <span>Resume</span>
      </button>
    {/each}</div>
  </section>
{:else}
<Row {title} items={hub.Metadata} shape={landscape ? "landscape" : undefined} useArt={continueStyle} {more} />

{/if}
<style>.compact { width: 100%; text-align: left; background: none; color: var(--text); gap: 20px; }</style>
