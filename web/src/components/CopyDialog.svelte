<script lang="ts">
  // Choose which server's copy of a title to use, from a search across servers.
  import { Info, Play, RotateCcw } from "@lucide/svelte";
  import { Badge } from "$lib/components/ui/badge";
  import { Button } from "$lib/components/ui/button";
  import { play } from "../lib/actions.svelte";
  import { modal } from "../lib/dialog";
  import * as plex from "../lib/plex";
  import { nav, routeFor } from "../lib/router.svelte";
  import { captureScope } from "../lib/state.svelte";
  import type { SearchGroup, ServerCopy } from "../lib/types";

  let { group, onclose }: { group: SearchGroup; onclose: () => void } = $props();
  // Playing or opening is dropped if the account or server changed while the dialog was open.
  const guard = captureScope();

  function use(copy: ServerCopy, resume: boolean) {
    onclose();
    if (guard.valid()) play(copy.item, { resume });
  }
  function details(copy: ServerCopy) {
    onclose();
    if (guard.valid()) nav.push(routeFor(copy.item));
  }
</script>

<dialog class="modal wide" use:modal={onclose} aria-labelledby="copies-title">
  <h2 id="copies-title">{group.title}{group.year ? ` (${group.year})` : ""}</h2>
  <p class="desc">{group.copies.length === 1 ? "On one server" : `On ${group.copies.length} servers`}. Choose the copy to play.</p>

  <ul class="divide-y rounded-xl border">
    {#each group.copies as copy, i (`${copy.serverId}:${copy.item.ratingKey}`)}
      {@const resumable = plex.progress(copy.item) != null}
      <li class="flex items-center gap-3.5 p-3">
        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-2 text-sm font-medium">{copy.serverName}{#if copy.current} <Badge variant="secondary">connected</Badge>{/if}</div>
          {#if copy.library}<div class="text-muted-foreground text-xs">{copy.library}</div>{/if}
          {#each copy.versions as v, j (j)}<div class="text-muted-foreground mt-0.5 text-xs tabular-nums">{v.label}</div>{/each}
          {#if !copy.versions.length}<div class="text-muted-foreground text-xs">No file information</div>{/if}
        </div>
        <div class="flex items-center gap-1.5">
          <Button data-initial-focus={i === 0 ? true : undefined} onclick={() => use(copy, true)}>
            <Play fill="currentColor" />{resumable ? "Resume" : "Play"}
          </Button>
          {#if resumable}<Button variant="outline" size="icon" title="Play from start" aria-label={`Play from start on ${copy.serverName}`} onclick={() => use(copy, false)}><RotateCcw /></Button>{/if}
          <Button variant="outline" size="icon" title="Details" aria-label={`Details on ${copy.serverName}`} onclick={() => details(copy)}><Info /></Button>
        </div>
      </li>
    {/each}
  </ul>
  <div class="foot"><Button variant="outline" onclick={onclose}>Cancel</Button></div>
</dialog>
