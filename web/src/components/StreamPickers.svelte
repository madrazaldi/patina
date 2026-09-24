<script lang="ts">
  // Version / audio / subtitle choosers. Changes are saved to Plex at once,
  // and playback follows them.
  import { api } from "../lib/api";
  import * as plex from "../lib/plex";
  import { act } from "../lib/state.svelte";
  import type { Metadata } from "../lib/types";

  let { item, mediaIndex = $bindable(0) }: { item: Metadata; mediaIndex?: number } = $props();

  let media = $derived(item.Media[mediaIndex] ?? item.Media[0]);
  let part = $derived(media?.Part[0]);
  let audio = $derived(part ? plex.audioStreams(part) : []);
  let subs = $derived(part ? plex.subtitleStreams(part) : []);
  let selectedAudio = $derived(audio.find((s) => s.selected)?.id ?? audio[0]?.id);
  let selectedSub = $derived(subs.find((s) => s.selected)?.id ?? 0);

  function select(a?: number, s?: number) {
    if (part?.id == null) return;
    act(api.selectStreams(part.id, a, s), undefined, true);
  }
</script>

{#if part}
  <div class="pickers">
    {#if item.Media.length > 1}
      <label>
        <span>Version</span>
        <select class="select" value={String(mediaIndex)} onchange={(e) => (mediaIndex = Number(e.currentTarget.value))}>
          {#each item.Media as m, i (i)}<option value={String(i)}>{plex.mediaLabel(m) || `Version ${i + 1}`}</option>{/each}
        </select>
      </label>
    {/if}
    {#if audio.length}
      <label>
        <span>Audio</span>
        <select class="select" value={String(selectedAudio)} onchange={(e) => select(Number(e.currentTarget.value))}>
          {#each audio as s (s.id)}<option value={String(s.id)}>{plex.streamLabel(s)}</option>{/each}
        </select>
      </label>
    {/if}
    {#if plex.kind(item) !== "track"}
      <label>
        <span>Subtitles</span>
        <select class="select" value={String(selectedSub)} onchange={(e) => select(undefined, Number(e.currentTarget.value))}>
          <option value="0">None</option>
          {#each subs as s (s.id)}
            <option value={String(s.id)}>{plex.streamLabel(s)}{plex.isExternal(s) ? " · external" : ""}</option>
          {/each}
        </select>
      </label>
    {/if}
  </div>
{/if}

<style>
  .pickers {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    margin-top: 26px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 200px;
    max-width: 340px;
  }
  label span {
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
  }
</style>
