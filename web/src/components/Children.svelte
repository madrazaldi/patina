<script lang="ts">
  // Seasons, episodes, albums, tracks or collection items below a detail header.
  import { Play } from "@lucide/svelte";
  import Card from "./Card.svelte";
  import Img from "./Img.svelte";
  import Row from "./Row.svelte";
  import Status from "./Status.svelte";
  import { openMenu, play } from "../lib/actions.svelte";
  import { api } from "../lib/api";
  import { airDate, clock, duration } from "../lib/format";
  import { img } from "../lib/images";
  import * as plex from "../lib/plex";
  import { resource } from "../lib/resource.svelte";
  import { nav } from "../lib/router.svelte";
  import { app } from "../lib/state.svelte";
  import type { Metadata } from "../lib/types";

  let { item }: { item: Metadata } = $props();

  let kind = $derived(plex.kind(item));
  const children = resource(() => {
    app.revision;
    return api.children(item.ratingKey ?? "", kind === "collection");
  });

  const open = (m: Metadata) => nav.push({ name: "item", ratingKey: m.ratingKey ?? "" });
</script>

{#if children.data === undefined}
  <Status loading={!children.error} error={children.error} />
{:else if children.data.length}
  {@const list = children.data}
  {#if kind === "show"}
    <Row title="Seasons" items={list} />
  {:else if kind === "season"}
    <div class="section-head"><h2>Episodes</h2></div>
    <div class="rows">
      {#each list as ep (ep.ratingKey)}
        {@const spoiler = app.spoilers && !plex.isWatched(ep)}
        {@const p = plex.progress(ep)}
        <div
          class="episode"
          role="button"
          tabindex="0"
          onclick={() => open(ep)}
          onkeydown={(e) => e.key === "Enter" && open(ep)}
          oncontextmenu={(e) => openMenu(ep, e)}
        >
          <div class="n num">{ep.index ?? ""}</div>
          <div class="thumb">
            <Img src={img(ep.thumb, 480, 270)} class={spoiler ? "spoiler" : ""} />
            {#if p != null}<div class="progress"><i style:width="{p * 100}%"></i></div>{/if}
            <button
              class="play"
              title="Play"
              onclick={(e) => {
                e.stopPropagation();
                play(ep);
              }}><Play size={14} fill="currentColor" /></button
            >
          </div>
          <div class="body">
            <h3>{plex.title(ep)}</h3>
            <div class="when">
              {[ep.originallyAvailableAt && airDate(ep.originallyAvailableAt), ep.duration && duration(ep.duration)]
                .filter(Boolean)
                .join(" · ")}
              {#if plex.isWatched(ep)}<span class="seen">· Watched</span>{/if}
            </div>
            {#if ep.summary}<p class={{ spoiler }}>{ep.summary}</p>{/if}
          </div>
        </div>
      {/each}
    </div>
  {:else if kind === "album"}
    <div class="section-head"><h2>Tracks</h2></div>
    <div class="rows">
      {#each list as t (t.ratingKey)}
        <div
          class="list-row clickable track"
          role="button"
          tabindex="0"
          onclick={() => play(t)}
          onkeydown={(e) => e.key === "Enter" && play(t)}
          oncontextmenu={(e) => openMenu(t, e)}
        >
          <span class="faint num idx">{t.index ?? ""}</span>
          <div class="grow">
            <div class="t">{plex.title(t)}</div>
            {#if t.originalTitle}<div class="s">{t.originalTitle}</div>{/if}
          </div>
          <div class="end">{t.duration ? clock(t.duration) : ""}</div>
        </div>
      {/each}
    </div>
  {:else}
    <div class="section-head"><h2>{kind === "artist" ? "Albums" : `${list.length} items`}</h2></div>
    <div class="grid">
      {#each list as m (m.ratingKey)}<Card item={m} />{/each}
    </div>
  {/if}
{/if}

<style>
  .episode {
    display: grid;
    grid-template-columns: 28px 220px 1fr;
    gap: 20px;
    align-items: start;
    padding: 16px 0;
    cursor: pointer;
  }
  .episode:hover h3 {
    text-decoration: underline;
    text-decoration-color: var(--line-2);
    text-underline-offset: 3px;
  }
  .n {
    padding-top: 2px;
    color: var(--text-3);
    text-align: right;
  }
  .thumb {
    position: relative;
    aspect-ratio: 16 / 9;
    border-radius: var(--r);
    overflow: hidden;
    background: var(--surface);
  }
  .thumb :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .thumb :global(img.spoiler) {
    filter: blur(14px) brightness(0.7);
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
  .play {
    position: absolute;
    left: 8px;
    bottom: 10px;
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    padding: 0 0 0 2px;
    border: 0;
    border-radius: 50%;
    background: var(--text);
    color: #0c0c0c;
    opacity: 0;
    cursor: pointer;
    transition: opacity 0.15s var(--ease);
  }
  .episode:hover .play {
    opacity: 1;
  }
  h3 {
    font-size: 14px;
    font-weight: 500;
  }
  .when {
    margin-top: 3px;
    font-size: 12px;
    color: var(--text-3);
  }
  .seen {
    color: var(--accent);
  }
  .body p {
    margin-top: 8px;
    max-width: 75ch;
    color: var(--text-2);
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
    transition: filter 0.2s;
  }
  .body p.spoiler {
    filter: blur(5px);
  }
  .body p.spoiler:hover {
    filter: none;
  }
  .track {
    grid-template-columns: 28px 1fr auto;
  }
  .idx {
    text-align: right;
  }
</style>
