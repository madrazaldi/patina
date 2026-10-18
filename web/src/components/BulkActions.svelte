<script lang="ts">
  import * as Alert from "$lib/components/ui/alert";
  import { Button } from "$lib/components/ui/button";
  import { Spinner } from "$lib/components/ui/spinner";
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
<div class="mb-4 flex flex-wrap items-center gap-2 rounded-lg border bg-card px-3 py-2" aria-busy={busy}>
  {#if busy}<Spinner class="text-muted-foreground" />{/if}
  <span class="mr-2 text-sm font-medium tabular-nums">{items.length} selected{#if busy} · {done} complete{/if}</span>
  <Button variant="outline" size="sm" disabled={busy || !items.length} onclick={() => run("queue")}>Add to queue</Button>
  <Button variant="outline" size="sm" disabled={busy || !items.length} onclick={() => run("download")}>Download</Button>
  <Button variant="outline" size="sm" disabled={busy || !items.length} onclick={() => run("watched")}>Mark watched</Button>
  <Button variant="outline" size="sm" disabled={busy || !items.length} onclick={playlist}>Add to playlist</Button>
  <Button variant="ghost" size="sm" class="ml-auto" disabled={busy || !items.length} onclick={clear}>Clear selection</Button>
</div>
{#if failures.length}
  <Alert.Root variant="destructive" role="status" class="mb-4">
    <Alert.Description class="whitespace-pre-wrap select-text">{done} completed. {failures.join("\n")}</Alert.Description>
  </Alert.Root>
{/if}
