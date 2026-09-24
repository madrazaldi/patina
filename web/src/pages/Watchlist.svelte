<script lang="ts">
  // plex.tv watchlist. Each entry links to the server's own copy when there is one.
  import { X } from "@lucide/svelte";
  import Img from "../components/Img.svelte";
  import Status from "../components/Status.svelte";
  import { api } from "../lib/api";
  import { img } from "../lib/images";
  import * as plex from "../lib/plex";
  import { resource } from "../lib/resource.svelte";
  import { nav } from "../lib/router.svelte";
  import { act, app } from "../lib/state.svelte";
  import type { Metadata } from "../lib/types";

  const list = resource(() => {
    app.revision;
    return api.watchlist();
  });

  /** guid → local rating key (null when the server doesn't have it). */
  let local = $state<Record<string, string | null>>({});
  const asked = new Set<string>();
  $effect(() => {
    for (const m of list.data ?? []) {
      const guid = m.guid;
      if (!guid || asked.has(guid)) continue;
      asked.add(guid);
      api.findByGuid(guid).then(
        (rk) => (local[guid] = rk),
        () => (local[guid] = null),
      );
    }
  });

  function open(m: Metadata) {
    const rk = m.guid ? local[m.guid] : null;
    if (rk) nav.push({ name: "item", ratingKey: rk });
    else app.toast("Not available on this server");
  }
</script>

<div class="page">
  <div class="page-head">
    <div>
      <h1>Watchlist</h1>
      <div class="sub">Synced with your Plex account. Titles this server doesn't have are dimmed.</div>
    </div>
  </div>
  {#if list.data === undefined}
    <Status loading={!list.error} error={list.error} />
  {:else if list.data.length === 0}
    <Status title="Your watchlist is empty" text="Add movies and shows from their detail page." />
  {:else}
    <div class="grid">
      {#each list.data as m (m.ratingKey)}
        {@const missing = m.guid != null && local[m.guid] === null}
        <div class="entry" class:missing role="button" tabindex="0" onclick={() => open(m)} onkeydown={(e) => e.key === "Enter" && open(m)}>
          <div class="art">
            <Img src={img(m.thumb, 300, 450)} />
            <button
              class="remove"
              title="Remove from watchlist"
              onclick={(e) => {
                e.stopPropagation();
                act(api.setWatchlisted(m.ratingKey ?? "", false), "Removed from watchlist", true);
              }}><X size={14} /></button
            >
          </div>
          <div class="title ellipsis">{plex.title(m)}</div>
          <div class="sub ellipsis">{missing ? "Not on this server" : plex.subtitle(m)}</div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .entry {
    min-width: 0;
    cursor: pointer;
  }
  .entry.missing .art {
    opacity: 0.4;
  }
  .art {
    position: relative;
    aspect-ratio: 2 / 3;
    border-radius: var(--r);
    overflow: hidden;
    background: var(--surface);
  }
  .art :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .remove {
    position: absolute;
    top: 6px;
    right: 6px;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: 0;
    border-radius: 3px;
    background: rgba(12, 12, 12, 0.82);
    color: var(--text);
    opacity: 0;
    cursor: pointer;
  }
  .entry:hover .remove {
    opacity: 1;
  }
  .title {
    margin-top: 8px;
    font-weight: 500;
  }
  .sub {
    font-size: 12px;
    color: var(--text-3);
  }
</style>
