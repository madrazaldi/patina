<script lang="ts">
  // "Add to playlist" dialog, shown while `app.playlistTarget` is set.
  import { Plus } from "@lucide/svelte";
  import Img from "./Img.svelte";
  import { modal } from "../lib/dialog";
  import { api } from "../lib/api";
  import { img } from "../lib/images";
  import * as plex from "../lib/plex";
  import { act, app, captureScope } from "../lib/state.svelte";
  import type { Metadata } from "../lib/types";

  let item = $derived(app.playlistTarget);
  // The account and server the dialog opened on: responses and writes for any other one are dropped.
  let guard = captureScope();
  let lists = $state<Metadata[] | null>(null);
  let error = $state<string>();
  let name = $state("");

  $effect(() => {
    const target = item;
    lists = null;
    error = undefined;
    name = "";
    if (!target) return;
    const opened = (guard = captureScope());
    api.playlists(plex.playlistKind(target)).then(
      (l) => app.playlistTarget === target && opened.valid() && (lists = l),
      (e) => app.playlistTarget === target && opened.valid() && (error = String(e)),
    );
  });

  const close = () => { app.playlistTarget = null; app.playlistItems = []; };

  function add(target: { playlist?: string; title?: string }, ok: string) {
    if (!item || !guard.valid()) return close();
    const rk = app.playlistItems.length ? app.playlistItems.map((m) => m.ratingKey).join(",") : item.ratingKey ?? "";
    const kind = plex.playlistKind(item);
    close();
    act(api.addToPlaylist(rk, kind, target), ok);
  }

  function create() {
    const title = name.trim();
    if (title) add({ title }, `Created playlist “${title}”`);
  }
</script>

{#if item}
  <dialog class="modal" use:modal={close} aria-labelledby="playlist-title">
    <h2 id="playlist-title">Add to playlist</h2>
    <p class="desc">{app.playlistItems.length ? `${app.playlistItems.length} selected titles` : plex.fullTitle(item)}</p>

    {#if error}
      <div class="error-box">{error}</div>
    {:else if lists === null}
      <div class="spinner"></div>
    {:else if lists.length === 0}
      <p class="faint">No playlists of this kind yet.</p>
    {:else}
      <div class="rows lists">
        {#each lists as p (p.ratingKey)}
          <button class="list-row clickable" onclick={() => add({ playlist: p.ratingKey }, `Added to “${p.title}”`)}>
            <div class="thumb square"><Img src={img(p.composite ?? p.thumb, 96, 96)} /></div>
            <div class="grow">
              <div class="t">{p.title}</div>
              <div class="s">{plex.subtitle(p)}</div>
            </div>
            <div class="end"><Plus /></div>
          </button>
        {/each}
      </div>
    {/if}

    <div class="new">
      <input
        class="input grow"
        placeholder="New playlist"
        bind:value={name}
        onkeydown={(e) => e.key === "Enter" && create()}
      />
      <button class="btn" disabled={!name.trim()} onclick={create}>Create</button>
    </div>
    <div class="foot"><button class="btn quiet" onclick={close}>Cancel</button></div>
  </dialog>
{/if}

<style>
  .lists {
    max-height: 300px;
    overflow: auto;
  }
  .lists button {
    width: 100%;
    border-left: 0;
    border-right: 0;
    border-bottom: 0;
    background: none;
    text-align: left;
  }
  .new {
    display: flex;
    gap: 8px;
    margin-top: 18px;
  }
</style>
