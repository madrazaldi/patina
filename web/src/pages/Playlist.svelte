<script lang="ts">
  import { Ellipsis, Play, Shuffle, X } from "@lucide/svelte";
  import Img from "../components/Img.svelte";
  import Status from "../components/Status.svelte";
  import { openMenu, play } from "../lib/actions.svelte";
  import { api } from "../lib/api";
  import { duration } from "../lib/format";
  import { img } from "../lib/images";
  import * as plex from "../lib/plex";
  import { resource } from "../lib/resource.svelte";
  import { nav, routeFor } from "../lib/router.svelte";
  import { act, app } from "../lib/state.svelte";

  let { id }: { id: string } = $props();

  const data = resource(() => {
    app.revision;
    return api.playlist(id);
  });

  async function remove() {
    if (await act(api.deletePlaylist(id), "Playlist deleted")) nav.replace({ name: "playlists" });
  }
</script>

{#if data.data === undefined}
  <Status loading={!data.error} error={data.error} fill />
{:else if data.data === null}
  <Status title="Playlist not found" fill />
{:else}
  {@const { playlist, items } = data.data}
  {@const total = items.reduce((n, m) => n + (m.duration ?? 0), 0)}
  <div class="page">
    <header class="head">
      <div class="cover"><Img src={img(playlist.composite ?? playlist.thumb, 480, 480)} /></div>
      <div>
        <div class="kicker">{playlist.smart ? "Smart playlist" : "Playlist"}</div>
        <h1 class="serif">{playlist.title}</h1>
        <div class="muted">{items.length} items · {duration(total)}</div>
        {#if playlist.summary}<p class="summary">{playlist.summary}</p>{/if}
        <div class="actions">
          <button class="btn primary lg" disabled={!items.length} onclick={() => play(playlist)}>
            <Play size={14} fill="currentColor" />Play
          </button>
          <button class="btn lg" disabled={!items.length} onclick={() => play(playlist, { shuffle: true })}>
            <Shuffle size={14} />Shuffle
          </button>
          <button class="btn quiet lg danger" onclick={remove}>Delete</button>
        </div>
      </div>
    </header>

    {#if items.length === 0}
      <Status title="This playlist is empty" />
    {:else}
      <div class="rows">
        {#each items as item, i (item.playlistItemID ?? i)}
          <div
            class="list-row clickable entry"
            role="button"
            tabindex="0"
            onclick={() => nav.push(routeFor(item))}
            onkeydown={(e) => e.key === "Enter" && nav.push(routeFor(item))}
            oncontextmenu={(e) => openMenu(item, e)}
          >
            <span class="n faint num">{i + 1}</span>
            <div class="thumb"><Img src={img(plex.poster(item), 96, 144)} /></div>
            <div class="grow">
              <div class="t">{plex.fullTitle(item)}</div>
              <div class="s">{plex.subtitle(item)}</div>
            </div>
            <div class="end">
              {#if item.duration}<span class="dur">{duration(item.duration)}</span>{/if}
              <button
                class="icon-btn"
                title="Play"
                onclick={(e) => {
                  e.stopPropagation();
                  play(item);
                }}><Play size={14} /></button
              >
              {#if !playlist.smart && item.playlistItemID != null}
                {@const itemId = item.playlistItemID}
                <button
                  class="icon-btn"
                  title="Remove from playlist"
                  onclick={(e) => {
                    e.stopPropagation();
                    act(api.removeFromPlaylist(id, itemId), undefined, true);
                  }}><X /></button
                >
              {/if}
              <button class="icon-btn" title="More" onclick={(e) => openMenu(item, e)}><Ellipsis /></button>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
{/if}

<style>
  .head {
    display: grid;
    grid-template-columns: 200px 1fr;
    gap: 32px;
    align-items: end;
    margin-bottom: 36px;
  }
  .cover {
    aspect-ratio: 1;
    border-radius: var(--r);
    overflow: hidden;
    background: var(--surface);
  }
  .cover :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .kicker {
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  h1 {
    margin: 8px 0 8px;
    font-size: 40px;
    font-weight: 400;
    line-height: 1.05;
  }
  .summary {
    margin-top: 10px;
    max-width: 70ch;
    color: var(--text-2);
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: 20px;
  }
  .entry {
    grid-template-columns: 28px 32px 1fr auto;
  }
  .n {
    text-align: right;
  }
  .dur {
    margin-right: 8px;
    font-size: 12px;
  }
</style>
