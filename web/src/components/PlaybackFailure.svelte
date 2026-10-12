<script lang="ts">
  // Shown when playback failed: why, and what to try (again, another version, a lower quality).
  import { TriangleAlert, X } from "@lucide/svelte";
  import { api, type Recover } from "../lib/api";
  import { clock } from "../lib/format";
  import * as plex from "../lib/plex";
  import { app } from "../lib/state.svelte";
  import type { Media } from "../lib/types";

  let f = $derived(app.failure);

  const act = (action: Recover) => api.recoverPlayback(action).catch((e) => app.error(e));
  const dismiss = () => api.dismissFailure().catch((e) => app.error(e));

  const label = (m: Media, i: number) => {
    const res = m.videoResolution ? (/^\d+$/.test(m.videoResolution) ? `${m.videoResolution}p` : m.videoResolution.toUpperCase()) : m.height ? `${m.height}p` : `version ${i + 1}`;
    return m.videoCodec ? `${res} ${m.videoCodec.toUpperCase()}` : res;
  };
</script>

{#if f}
  {@const [title] = plex.playingLines(f.item)}
  <div class="failure" role="alert">
    <TriangleAlert size={16} />
    <div class="grow text">
      <div class="ellipsis"><strong>Couldn't play {title}.</strong> {f.reason}</div>
      {#if f.detail}<div class="ellipsis faint detail">{f.detail}</div>{/if}
    </div>
    {#if f.canRetry}
      <button class="btn sm" onclick={() => act({ kind: "retry" })}>Try again{f.positionMs > 0 ? ` from ${clock(f.positionMs)}` : ""}</button>
    {/if}
    {#each f.item.Media as m, i (i)}
      {#if i !== f.mediaIndex}
        <button class="btn sm" onclick={() => act({ kind: "version", index: i })}>Try {label(m, i)}</button>
      {/if}
    {/each}
    {#if f.lowerQuality}
      <button class="btn sm" onclick={() => act({ kind: "lowerQuality" })}>Try {f.lowerQuality.label}</button>
    {/if}
    <button class="icon-btn" title="Dismiss" onclick={dismiss}><X size={15} /></button>
  </div>
{/if}

<style>
  .failure {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px var(--gutter);
    background: var(--surface);
    border-bottom: 1px solid var(--line);
    color: var(--danger);
  }
  .text {
    min-width: 0;
    color: var(--text);
  }
  .detail {
    font-size: 12px;
  }
</style>
