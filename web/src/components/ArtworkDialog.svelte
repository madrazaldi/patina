<script lang="ts">
  // Poster / background picker, shown while `app.artworkTarget` is set.
  import { Check } from "@lucide/svelte";
  import Img from "./Img.svelte";
  import { api } from "../lib/api";
  import { img } from "../lib/images";
  import * as plex from "../lib/plex";
  import { act, app, message } from "../lib/state.svelte";
  import type { Artwork } from "../lib/types";

  let target = $derived(app.artworkTarget);
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
    api.artwork(t.item.ratingKey ?? "", t.kind).then(
      (l) => app.artworkTarget === t && (choices = l),
      (e) => (error = message(e)),
    );
  });

  function close() {
    if (!busy) app.artworkTarget = null;
  }

  async function pick(a: Artwork) {
    const t = target;
    const url = a.key ?? a.ratingKey;
    if (!t || !url || busy) return;
    busy = url;
    const ok = await act(api.setArtwork(t.item.ratingKey ?? "", t.kind, url), art ? "Background updated" : "Poster updated", true);
    busy = null;
    if (ok) app.artworkTarget = null;
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && target && close()} />

{#if target}
  <div class="modal-backdrop" role="presentation" onclick={close}>
    <div class="modal wide" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      <h2>{art ? "Choose background" : "Choose poster"}</h2>
      <p class="desc">{plex.fullTitle(target.item)}</p>

      {#if error}
        <div class="error-box">{error}</div>
      {:else if choices === null}
        <div class="spinner"></div>
      {:else if choices.length === 0}
        <p class="faint">No {art ? "backgrounds" : "posters"} available.</p>
      {:else}
        <div class={["choices", { art, square }]}>
          {#each choices as a, i (a.key ?? a.ratingKey ?? i)}
            {@const url = a.key ?? a.ratingKey}
            <button class={["choice", { selected: !!a.selected }]} disabled={!!busy} onclick={() => pick(a)}>
              <span class="frame">
                <Img src={art ? img(a.thumb ?? a.key, 480, 270) : square ? img(a.thumb ?? a.key, 300, 300) : img(a.thumb ?? a.key, 240, 360)} />
                {#if a.selected}<span class="mark"><Check size={12} strokeWidth={2.4} /></span>{/if}
                {#if busy && busy === url}<span class="wait"><span class="spinner"></span></span>{/if}
              </span>
              <span class="provider ellipsis">{a.selected ? "Current" : (a.provider ?? "")}</span>
            </button>
          {/each}
        </div>
      {/if}

      <div class="foot"><button class="btn quiet" disabled={!!busy} onclick={close}>Cancel</button></div>
    </div>
  </div>
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
    padding: 0;
    border: 0;
    background: none;
    text-align: left;
    cursor: pointer;
  }
  .choice:disabled {
    cursor: default;
  }
  .frame {
    position: relative;
    display: block;
    aspect-ratio: 2 / 3;
    border-radius: var(--r);
    overflow: hidden;
    background: var(--surface);
    box-shadow: inset 0 0 0 1px var(--line);
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
    outline: 1px solid var(--accent);
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
    border-radius: 3px;
    background: rgba(12, 12, 12, 0.82);
    color: var(--accent);
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
    font-size: 11.5px;
    color: var(--text-3);
  }
  .selected .provider {
    color: var(--accent);
  }
</style>
