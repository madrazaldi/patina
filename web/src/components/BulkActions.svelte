<script lang="ts">
  import { api } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { playlistKind } from "../lib/plex";
  import type { Metadata } from "../lib/types";
  let { items, clear }: { items: Metadata[]; clear: () => void } = $props();
  let busy = $state(false), done = $state(0);
  let failures = $state<string[]>([]);
  async function run(action: "queue" | "download" | "watched") {
    const list = $state.snapshot(items), scope = app.scope;
    busy = true; done = 0; failures = [];
    if (action === "watched") {
      // One request, so it is announced once and undone together.
      try {
        await api.setWatchedMany(list.map((item) => item.ratingKey!), true);
        if (app.scope === scope) clear();
      } catch (e) { failures.push(String(e)); }
      finally { busy = false; }
      return;
    }
    try {
      for (const item of list) {
        if (app.scope !== scope) { failures.push("Stopped because the account or server changed."); break; }
        try {
          if (action === "queue") {
            await api.queueItems(list);
            done = list.length;
            break;
          } else await api.download(item.ratingKey!);
          done++;
        } catch (e) { failures.push(`${item.title}: ${String(e)}`); if (action === "queue") break; }
      }
      if (!failures.length) { app.toast(`${done} titles updated`); clear(); }
    } finally { busy = false; }
  }
  function playlist() {
    if (!items.length) return;
    if (new Set(items.map(playlistKind)).size !== 1) { app.error("Select only video or only music for a playlist."); return; }
    app.playlistItems = $state.snapshot(items);
    app.playlistTarget = items[0];
  }
</script>
<div class="bulk" aria-busy={busy}>
  <span class="num">{items.length} selected{#if busy} · {done} complete{/if}</span>
  <button class="btn sm" disabled={busy || !items.length} onclick={() => run("queue")}>Add to queue</button>
  <button class="btn sm" disabled={busy || !items.length} onclick={() => run("download")}>Download</button>
  <button class="btn sm" disabled={busy || !items.length} onclick={() => run("watched")}>Mark watched</button>
  <button class="btn sm" disabled={busy || !items.length} onclick={playlist}>Add to playlist</button>
  <button class="btn quiet sm" disabled={busy || !items.length} onclick={clear}>Clear selection</button>
</div>
{#if failures.length}<div class="error-box" role="status">{done} completed. {failures.join("\n")}</div>{/if}
<style>
  .bulk { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; padding: 12px; margin-bottom: 16px; background: var(--surface); border: 1px solid var(--line); }
</style>
