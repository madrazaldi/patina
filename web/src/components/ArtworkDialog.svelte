<script lang="ts">
  // Poster / background picker, shown while `app.artworkTarget` is set.
  import { Check } from "@lucide/svelte";
  import * as Alert from "$lib/components/ui/alert";
  import { Button } from "$lib/components/ui/button";
  import * as Empty from "$lib/components/ui/empty";
  import { Spinner } from "$lib/components/ui/spinner";
  import Img from "./Img.svelte";
  import { modal } from "../lib/dialog";
  import { api } from "../lib/api";
  import { img } from "../lib/images";
  import * as plex from "../lib/plex";
  import { act, app, captureScope, message } from "../lib/state.svelte";
  import type { Artwork } from "../lib/types";

  let target = $derived(app.artworkTarget);
  // The account and server the dialog opened on: responses and writes for any other one are dropped.
  let guard = captureScope();
  let choices = $state<Artwork[] | null>(null);
  let error = $state<string>();
  let busy = $state<string | null>(null);

  let art = $derived(target?.kind === "art");
  let square = $derived(!art && !!target && ["artist", "album"].includes(plex.kind(target.item)));

  $effect(() => {
    const t = target;
    choices = null;
    error = undefined;
    busy = null;
    if (!t) return;
    const opened = (guard = captureScope());
    api.artwork(t.item.ratingKey ?? "", t.kind).then(
      (l) => app.artworkTarget === t && opened.valid() && (choices = l),
      (e) => app.artworkTarget === t && opened.valid() && (error = message(e)),
    );
  });

  function close() {
    if (!busy) app.artworkTarget = null;
  }

  async function pick(a: Artwork) {
    const t = target;
    const url = a.key ?? a.ratingKey;
    if (!t || !url || busy) return;
    if (!guard.valid()) { app.artworkTarget = null; return; }
    busy = url;
    const ok = await act(api.setArtwork(t.item.ratingKey ?? "", t.kind, url), art ? "Background updated" : "Poster updated", true);
    busy = null;
    if (ok) app.artworkTarget = null;
  }
</script>

{#if target}
  <dialog class="modal wide" use:modal={close} aria-labelledby="artwork-title">
    <h2 id="artwork-title">{art ? "Choose background" : "Choose poster"}</h2>
    <p class="desc">{plex.fullTitle(target.item)}</p>

    {#if error}
      <Alert.Root variant="destructive"><Alert.Description class="whitespace-pre-wrap select-text">{error}</Alert.Description></Alert.Root>
    {:else if choices === null}
      <div class="flex justify-center py-6"><Spinner class="text-muted-foreground" /></div>
    {:else if choices.length === 0}
      <Empty.Root class="border border-dashed p-6">
        <Empty.Header>
          <Empty.Title>Nothing to choose</Empty.Title>
          <Empty.Description>No {art ? "backgrounds" : "posters"} available.</Empty.Description>
        </Empty.Header>
      </Empty.Root>
    {:else}
      <div class={["choices", { art, square }]}>
        {#each choices as a, i (a.key ?? a.ratingKey ?? i)}
          {@const url = a.key ?? a.ratingKey}
          <button class={["choice", { selected: !!a.selected }]} data-initial-focus={i === 0 ? "" : undefined} aria-pressed={!!a.selected} aria-label={a.selected ? `${a.provider ?? "Artwork"} (current)` : (a.provider ?? "Artwork")} disabled={!!busy} onclick={() => pick(a)}>
            <span class="frame">
              <Img src={art ? img(a.thumb ?? a.key, 480, 270) : square ? img(a.thumb ?? a.key, 300, 300) : img(a.thumb ?? a.key, 240, 360)} />
              {#if a.selected}<span class="mark"><Check size={12} strokeWidth={2.4} /></span>{/if}
              {#if busy && busy === url}<span class="wait"><Spinner /></span>{/if}
            </span>
            <span class="provider truncate">{a.selected ? "Current" : (a.provider ?? "")}</span>
          </button>
        {/each}
      </div>
    {/if}

    <div class="foot"><Button variant="outline" disabled={!!busy} onclick={close}>Cancel</Button></div>
  </dialog>
{/if}

<style>
  .choices {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(104px, 1fr));
    gap: 16px 12px;
    max-height: 56vh;
    overflow: auto;
    padding: 2px;
  }
  .choices.art {
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  }
  .choice {
    min-width: 0;
    border-radius: var(--radius-lg);
    outline: none;
    padding: 0;
    border: 0;
    background: none;
    text-align: left;
    cursor: pointer;
  }
  .choice:focus-visible .frame {
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--ring) 50%, transparent);
  }
  .choice:disabled {
    cursor: default;
  }
  .frame {
    position: relative;
    display: block;
    aspect-ratio: 2 / 3;
    border-radius: var(--radius-lg);
    overflow: hidden;
    background: var(--muted);
    box-shadow: inset 0 0 0 1px var(--border);
  }
  .art .frame {
    aspect-ratio: 16 / 9;
  }
  .square .frame {
    aspect-ratio: 1;
  }
  .frame :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition:
      opacity 0.3s var(--ease),
      filter 0.15s var(--ease);
  }
  .choice:not(:disabled):hover .frame :global(img) {
    filter: brightness(0.75);
  }
  .selected .frame {
    outline: 2px solid var(--tint);
    outline-offset: 2px;
  }
  .mark {
    position: absolute;
    top: 6px;
    right: 6px;
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: 9999px;
    background: var(--popover);
    color: var(--tint);
  }
  .wait {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgba(12, 12, 12, 0.5);
  }
  .provider {
    display: block;
    margin-top: 6px;
    font-size: 12px;
    color: var(--muted-foreground);
  }
  .selected .provider {
    color: var(--tint);
  }
</style>
