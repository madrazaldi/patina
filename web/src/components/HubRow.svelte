<script lang="ts">
  // A Plex hub (Continue Watching, Recently Added, ...) rendered as a row.
  import { Play } from "@lucide/svelte";
  import { Badge } from "$lib/components/ui/badge";
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
      <button class="flex w-full cursor-pointer items-center gap-5 px-4 py-2.5 text-left text-sm transition-colors hover:bg-muted/50" onclick={() => play(item)}>
        <span class="min-w-0 flex-1 truncate font-medium">{item.grandparentTitle ? `${item.grandparentTitle} · ` : ""}{item.title}</span>
        <span class="text-xs text-muted-foreground tabular-nums">{duration(Math.max(0, (item.duration ?? 0) - (item.viewOffset ?? 0)))} left</span>
        <Badge variant="secondary"><Play fill="currentColor" />Resume</Badge>
      </button>
    {/each}</div>
  </section>
{:else}
<Row {title} items={hub.Metadata} shape={landscape ? "landscape" : undefined} useArt={continueStyle} {more} />
{/if}
