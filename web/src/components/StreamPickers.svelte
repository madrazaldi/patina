<script lang="ts">
  // Version / audio / subtitle choosers. Changes are saved to Plex at once,
  // and playback follows them.
  import { Trash } from "@lucide/svelte";
  import { api } from "../lib/api";
  import * as plex from "../lib/plex";
  import { act, app } from "../lib/state.svelte";
  import type { Metadata } from "../lib/types";

  let { item, mediaIndex = $bindable(0) }: { item: Metadata; mediaIndex?: number } = $props();

  // An item found on another server: its tracks are chosen there, and the connected
  // server's owner rights (searching and deleting subtitle files) don't apply.
  let foreign = $derived(item.patinaServer);
  let owner = $derived(app.owned && !foreign);

  let media = $derived(item.Media[mediaIndex] ?? item.Media[0]);
  let part = $derived(media?.Part[0]);
  let audio = $derived(part ? plex.audioStreams(part) : []);
  let subs = $derived(part ? plex.subtitleStreams(part) : []);
  let selectedAudio = $derived(audio.find((s) => s.selected)?.id ?? audio[0]?.id);
  let selectedSub = $derived(subs.find((s) => s.selected)?.id ?? 0);
  let external = $derived(subs.find((s) => s.selected && plex.isExternal(s)));

  function select(a?: number, s?: number) {
    if (part?.id == null) return;
    act(api.selectStreams(part.id, a, s, foreign), undefined, true);
  }

  async function removeSub(id: number, label: string) {
    const ok = await app.confirm("Delete this subtitle?", { text: `${label} is removed from the server.`, action: "Delete", danger: true });
    if (ok) act(api.deleteStream(id), "Subtitle deleted", true);
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
      <div class="field">
        <div class="head">
          <span>Subtitles</span>
          {#if owner && ["movie", "episode"].includes(plex.kind(item))}
            <button class="link" onclick={() => (app.subtitleTarget = item)}>Search…</button>
          {/if}
        </div>
        <div class="control">
          <select
            class="select"
            aria-label="Subtitles"
            value={String(selectedSub)}
            onchange={(e) => select(undefined, Number(e.currentTarget.value))}
          >
            <option value="0">None</option>
            {#each subs as s (s.id)}
              <option value={String(s.id)}>{plex.streamLabel(s)}{plex.isExternal(s) ? " · external" : ""}</option>
            {/each}
          </select>
          {#if owner && external?.id != null}
            {@const id = external.id}
            {@const label = plex.streamLabel(external)}
            <button class="icon-btn" title="Delete this subtitle file" onclick={() => removeSub(id, label)}><Trash size={15} /></button>
          {/if}
        </div>
      </div>
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
  label,
  .field {
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 200px;
    max-width: 340px;
  }
  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
  }
  .head .link {
    font-size: 11.5px;
    text-decoration: none;
  }
  .head .link:hover {
    text-decoration: underline;
  }
  .control {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .control .select {
    flex: 1;
    min-width: 0;
  }
  label span,
  .head span {
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
  }
</style>
