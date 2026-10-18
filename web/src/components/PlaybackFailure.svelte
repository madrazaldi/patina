<script lang="ts">
  // Shown when playback failed: why, and what to try (again, another version, a lower quality).
  import { TriangleAlert, X } from "@lucide/svelte";
  import * as Alert from "$lib/components/ui/alert";
  import { Button } from "$lib/components/ui/button";
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
  <div class="flex-none px-4 pt-3">
    <Alert.Root variant="destructive" class="flex items-start gap-3 py-3 has-data-[slot=alert-action]:pr-2.5">
      <TriangleAlert class="mt-0.5 size-4 shrink-0" />
      <div class="min-w-0 flex-1">
        <Alert.Title class="truncate"><span class="font-medium">Couldn't play {title}.</span></Alert.Title>
        <Alert.Description class="truncate">{f.reason}</Alert.Description>
        {#if f.detail}<div class="text-muted-foreground mt-0.5 truncate text-xs">{f.detail}</div>{/if}
        <div class="mt-2.5 flex flex-wrap items-center gap-1.5">
          {#if f.canRetry}
            <Button variant="outline" size="sm" class="text-foreground" onclick={() => act({ kind: "retry" })}>Try again{f.positionMs > 0 ? ` from ${clock(f.positionMs)}` : ""}</Button>
          {/if}
          {#each f.item.Media as m, i (i)}
            {#if i !== f.mediaIndex}
              <Button variant="outline" size="sm" class="text-foreground" onclick={() => act({ kind: "version", index: i })}>Try {label(m, i)}</Button>
            {/if}
          {/each}
          {#if f.lowerQuality}
            <Button variant="outline" size="sm" class="text-foreground" onclick={() => act({ kind: "lowerQuality" })}>Try {f.lowerQuality.label}</Button>
          {/if}
        </div>
      </div>
      <Alert.Action class="static shrink-0">
        <Button variant="ghost" size="icon-sm" title="Dismiss" onclick={dismiss}><X /></Button>
      </Alert.Action>
    </Alert.Root>
  </div>
{/if}
