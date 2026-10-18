<script lang="ts">
  // Offers the queue that was playing when Patina last stopped. Nothing plays until Resume is pressed.
  import { Play } from "@lucide/svelte";
  import * as Alert from "$lib/components/ui/alert";
  import { Button } from "$lib/components/ui/button";
  import { api } from "../lib/api";
  import { clock } from "../lib/format";
  import * as plex from "../lib/plex";
  import { app } from "../lib/state.svelte";

  let saved = $derived(app.savedQueue);

  async function resume() {
    try {
      await api.resumeSaved();
      app.savedQueue = null;
    } catch (e) {
      app.error(e);
    }
  }

  async function forget() {
    app.savedQueue = null;
    await api.forgetSaved().catch((e) => app.error(e));
  }
</script>

{#if saved && !app.nowPlaying}
  {@const [title, sub] = plex.playingLines(saved.current)}
  <div class="flex-none px-4 pt-3">
    <Alert.Root role="status" class="flex items-center gap-3 py-2.5 has-data-[slot=alert-action]:pr-2.5">
      <Play class="size-4 shrink-0 text-tint" fill="currentColor" />
      <div class="min-w-0 flex-1 truncate">
        Resume <strong class="font-medium">{title}</strong>{#if sub}<span class="text-muted-foreground"> · {sub}</span>{/if}
        {#if saved.positionMs > 0}<span class="text-muted-foreground"> at {clock(saved.positionMs)}</span>{/if}
        {#if saved.count > 1}<span class="text-muted-foreground"> · {saved.index + 1} of {saved.count} in the queue</span>{/if}
      </div>
      <Alert.Action class="static flex shrink-0 items-center gap-1.5">
        <Button size="sm" onclick={resume}>Resume</Button>
        <Button size="sm" variant="ghost" onclick={() => (app.savedQueue = null)}>Not now</Button>
        <Button size="sm" variant="ghost" onclick={forget}>Forget</Button>
      </Alert.Action>
    </Alert.Root>
  </div>
{/if}
