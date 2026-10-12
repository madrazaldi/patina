<script lang="ts">
  // Choose which server's copy of a title to use, from a search across servers.
  import { Info, Play, RotateCcw } from "@lucide/svelte";
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

  <ul class="copies">
    {#each group.copies as copy, i (copy.serverId)}
      {@const resumable = plex.progress(copy.item) != null}
      <li class="copy">
        <div class="grow">
          <div class="t">{copy.serverName}{#if copy.current} <span class="faint">connected</span>{/if}</div>
          {#if copy.library}<div class="s">{copy.library}</div>{/if}
          {#each copy.versions as v, j (j)}<div class="version num">{v.label}</div>{/each}
          {#if !copy.versions.length}<div class="s">No file information</div>{/if}
        </div>
        <div class="acts">
          <button class="btn primary" data-initial-focus={i === 0 ? true : undefined} onclick={() => use(copy, true)}>
            <Play size={14} fill="currentColor" />{resumable ? "Resume" : "Play"}
          </button>
          {#if resumable}<button class="icon-btn outlined" title="Play from start" aria-label={`Play from start on ${copy.serverName}`} onclick={() => use(copy, false)}><RotateCcw /></button>{/if}
          <button class="icon-btn outlined" title="Details" aria-label={`Details on ${copy.serverName}`} onclick={() => details(copy)}><Info /></button>
        </div>
      </li>
    {/each}
  </ul>
  <div class="foot"><button class="btn quiet" onclick={onclose}>Cancel</button></div>
</dialog>

<style>
  .copies {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .copy {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 0;
    border-top: 1px solid var(--line);
  }
  .copy:first-child {
    border-top: 0;
  }
  .t {
    font-weight: 500;
  }
  .s {
    color: var(--text-3);
    font-size: 12px;
  }
  .version {
    margin-top: 2px;
    color: var(--text-2);
    font-size: 12px;
  }
  .acts {
    display: flex;
    align-items: center;
    gap: 6px;
  }
</style>
