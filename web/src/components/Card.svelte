<script lang="ts">
  // Poster / thumbnail card for any metadata item.
  import { Check, Ellipsis, Play } from "@lucide/svelte";
  import Img from "./Img.svelte";
  import { openMenu, play } from "../lib/actions.svelte";
  import { img } from "../lib/images";
  import * as plex from "../lib/plex";
  import { nav, routeFor } from "../lib/router.svelte";
  import { app } from "../lib/state.svelte";
  import type { Metadata } from "../lib/types";

  let {
    item,
    shape: forced,
    useArt = false,
    fixed = false,
  }: {
    item: Metadata;
    shape?: plex.Shape;
    /** Continue-watching style: backdrop art, show name as the title. */
    useArt?: boolean;
    /** Fixed width (inside horizontal rows) rather than filling a grid cell. */
    fixed?: boolean;
  } = $props();

  const SIZES = { poster: [300, 450], landscape: [560, 315], square: [360, 360] } as const;

  let shape = $derived(forced ?? plex.shapeFor(item));
  let kind = $derived(plex.kind(item));
  let src = $derived.by(() => {
    const [w, h] = SIZES[shape];
    let source: string | undefined;
    if (shape === "landscape" && useArt && kind !== "episode") source = plex.backdrop(item) ?? item.thumb;
    else if (shape === "landscape") source = item.thumb ?? plex.backdrop(item);
    else source = plex.poster(item);
    return img(source, w, h);
  });
  let lines = $derived(
    kind === "episode" && useArt
      ? [item.grandparentTitle ?? "", `${plex.episodeCode(item)} · ${plex.title(item)}`]
      : [plex.title(item), plex.subtitle(item)],
  );
  let unwatched = $derived(
    kind === "show" || kind === "season" ? (plex.unwatchedLeaves(item) ?? 0) : 0,
  );
  let watched = $derived(plex.hasWatchState(item) && unwatched === 0 && plex.isWatched(item));
  let progress = $derived(plex.progress(item));
  let spoiler = $derived(app.spoilers && kind === "episode" && !plex.isWatched(item));

  const open = () => nav.push(routeFor(item));
</script>

<div
  class={["card", shape, { fixed }]}
  role="button"
  tabindex="0"
  onclick={open}
  onkeydown={(e) => e.key === "Enter" && open()}
  oncontextmenu={(e) => openMenu(item, e)}
>
  <div class="art">
    {#if src}
      <Img {src} class={spoiler ? "spoiler" : ""} />
    {:else}
      <div class="placeholder serif">{plex.title(item)}</div>
    {/if}
    {#if unwatched > 0}
      <span class="badge num">{unwatched}</span>
    {:else if watched}
      <span class="badge watched" title="Watched"><Check size={12} strokeWidth={2.2} /></span>
    {/if}
    {#if progress != null}
      <div class="progress"><i style:width="{progress * 100}%"></i></div>
    {/if}
    <div class="hover">
      {#if plex.canPlay(item)}
        <button
          class="play"
          title="Play"
          onclick={(e) => {
            e.stopPropagation();
            play(item);
          }}><Play size={14} fill="currentColor" /></button
        >
      {/if}
      <button class="more" title="More" onclick={(e) => openMenu(item, e)}><Ellipsis /></button>
    </div>
  </div>
  <div class="title ellipsis" title={lines[0]}>{lines[0]}</div>
  {#if lines[1]}<div class="sub ellipsis">{lines[1]}</div>{/if}
</div>

<style>
  .card {
    min-width: 0;
    cursor: pointer;
    outline: none;
  }
  .card.fixed {
    flex: none;
    width: var(--poster);
  }
  .card.fixed.landscape {
    width: calc(var(--poster) * 1.75);
  }
  .art {
    position: relative;
    aspect-ratio: 2 / 3;
    border-radius: var(--r);
    overflow: hidden;
    background: var(--surface);
  }
  .landscape .art {
    aspect-ratio: 16 / 9;
  }
  .square .art {
    aspect-ratio: 1;
  }
  /* A hairline keeps dark posters from dissolving into the background. */
  .art::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: inherit;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.06);
    pointer-events: none;
  }
  .art :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition:
      opacity 0.3s var(--ease),
      filter 0.2s var(--ease);
  }
  .art :global(img.spoiler) {
    filter: blur(14px) brightness(0.7);
  }
  .card:hover .art :global(img) {
    filter: brightness(0.72);
  }
  .card:hover .art :global(img.spoiler) {
    filter: blur(14px) brightness(0.55);
  }
  .card:focus-visible .art {
    outline: 1px solid var(--accent);
    outline-offset: 2px;
  }
  .placeholder {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    padding: 12px;
    text-align: center;
    font-size: 15px;
    line-height: 1.25;
    color: var(--text-3);
  }
  .badge {
    position: absolute;
    top: 6px;
    right: 6px;
    display: grid;
    place-items: center;
    min-width: 20px;
    height: 20px;
    padding: 0 5px;
    border-radius: 3px;
    background: rgba(12, 12, 12, 0.82);
    font-size: 11px;
    font-weight: 600;
    color: var(--text);
  }
  .badge.watched {
    color: var(--accent);
    padding: 0;
  }
  .progress {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 3px;
    background: rgba(0, 0, 0, 0.55);
  }
  .progress i {
    display: block;
    height: 100%;
    background: var(--accent);
  }
  .hover {
    position: absolute;
    inset: 0;
    opacity: 0;
    transition: opacity 0.15s var(--ease);
  }
  .card:hover .hover,
  .card:focus-within .hover {
    opacity: 1;
  }
  .hover button {
    position: absolute;
    display: grid;
    place-items: center;
    border: 0;
    padding: 0;
    cursor: pointer;
  }
  .play {
    left: 8px;
    bottom: 10px;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    background: var(--text);
    color: #0c0c0c;
  }
  .play:hover {
    background: #fff;
  }
  .play :global(svg) {
    margin-left: 2px;
  }
  .more {
    top: 6px;
    left: 6px;
    width: 26px;
    height: 22px;
    border-radius: 3px;
    background: rgba(12, 12, 12, 0.82);
    color: var(--text);
  }
  .title {
    margin-top: 8px;
    font-weight: 500;
  }
  .sub {
    margin-top: 1px;
    font-size: 12px;
    color: var(--text-3);
  }
</style>
