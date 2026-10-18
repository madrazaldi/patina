<script lang="ts">
  // Version / audio / subtitle choosers. Changes are saved to Plex at once,
  // and playback follows them.
  import { Trash } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button";
  import { Label } from "$lib/components/ui/label";
  import * as NativeSelect from "$lib/components/ui/native-select";
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
  <div class="mt-6 flex flex-wrap items-end gap-4">
    {#if item.Media.length > 1}
      <div class="flex w-56 flex-col gap-1.5">
        <Label class="text-muted-foreground text-xs" for="pick-version">Version</Label>
        <NativeSelect.Root id="pick-version" class="w-full" value={String(mediaIndex)} onchange={(e) => (mediaIndex = Number(e.currentTarget.value))}>
          {#each item.Media as m, i (i)}<NativeSelect.Option value={String(i)}>{plex.mediaLabel(m) || `Version ${i + 1}`}</NativeSelect.Option>{/each}
        </NativeSelect.Root>
      </div>
    {/if}
    {#if audio.length}
      <div class="flex w-56 flex-col gap-1.5">
        <Label class="text-muted-foreground text-xs" for="pick-audio">Audio</Label>
        <NativeSelect.Root id="pick-audio" class="w-full" value={String(selectedAudio)} onchange={(e) => select(Number(e.currentTarget.value))}>
          {#each audio as s (s.id)}<NativeSelect.Option value={String(s.id)}>{plex.streamLabel(s)}</NativeSelect.Option>{/each}
        </NativeSelect.Root>
      </div>
    {/if}
    {#if plex.kind(item) !== "track"}
      <div class="flex w-56 flex-col gap-1.5">
        <div class="flex items-center justify-between gap-3">
          <Label class="text-muted-foreground text-xs" for="pick-subtitles">Subtitles</Label>
          {#if owner && ["movie", "episode"].includes(plex.kind(item))}
            <Button variant="link" size="xs" class="text-muted-foreground hover:text-foreground h-auto p-0 text-xs" onclick={() => (app.subtitleTarget = item)}>Search…</Button>
          {/if}
        </div>
        <div class="flex items-center gap-1">
          <NativeSelect.Root
            id="pick-subtitles"
            class="min-w-0 flex-1"
            aria-label="Subtitles"
            value={String(selectedSub)}
            onchange={(e) => select(undefined, Number(e.currentTarget.value))}
          >
            <NativeSelect.Option value="0">None</NativeSelect.Option>
            {#each subs as s (s.id)}
              <NativeSelect.Option value={String(s.id)}>{plex.streamLabel(s)}{plex.isExternal(s) ? " · external" : ""}</NativeSelect.Option>
            {/each}
          </NativeSelect.Root>
          {#if owner && external?.id != null}
            {@const id = external.id}
            {@const label = plex.streamLabel(external)}
            <Button variant="ghost" size="icon" class="text-muted-foreground" title="Delete this subtitle file" onclick={() => removeSub(id, label)}><Trash /></Button>
          {/if}
        </div>
      </div>
    {/if}
  </div>
{/if}
