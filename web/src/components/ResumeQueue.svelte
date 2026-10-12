<script lang="ts">
  // Offers the queue that was playing when Patina last stopped. Nothing plays until Resume is pressed.
  import { Play } from "@lucide/svelte";
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
  <div class="resume" role="status">
    <Play size={14} />
    <div class="grow ellipsis">
      Resume <strong>{title}</strong>{#if sub}<span class="faint"> · {sub}</span>{/if}
      {#if saved.positionMs > 0}<span class="faint"> at {clock(saved.positionMs)}</span>{/if}
      {#if saved.count > 1}<span class="faint"> · {saved.index + 1} of {saved.count} in the queue</span>{/if}
    </div>
    <button class="btn sm primary" onclick={resume}>Resume</button>
    <button class="btn sm quiet" onclick={() => (app.savedQueue = null)}>Not now</button>
    <button class="btn sm quiet" onclick={forget}>Forget</button>
  </div>
{/if}

<style>
  .resume {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px var(--gutter);
    background: var(--surface);
    border-bottom: 1px solid var(--line);
  }
</style>
