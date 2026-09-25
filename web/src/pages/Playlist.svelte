<script lang="ts">
  import { Ellipsis, GripVertical, ListStart, Play, Shuffle, X } from "@lucide/svelte";
  import Img from "../components/Img.svelte";
  import Status from "../components/Status.svelte";
  import { enqueue, openMenu, play } from "../lib/actions.svelte";
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

  // Local order, overwritten optimistically while a move is saved.
  let order = $derived(data.data?.items ?? []);
  let reorderable = $derived(data.data != null && !data.data.playlist.smart);

  // Drag-and-drop: the dragged row, and where it would land.
  let dragging = $state<number | null>(null);
  let target = $state<{ index: number; after: boolean } | null>(null);

  function over(e: DragEvent, index: number) {
    if (dragging === null) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    target = { index, after: e.clientY > r.top + r.height / 2 };
  }

  function drop(e: DragEvent) {
    e.preventDefault();
    const from = dragging;
    const to = target;
    dragging = target = null;
    if (from === null || !to) return;
    let at = to.index + (to.after ? 1 : 0);
    if (from < at) at--;
    if (at === from) return;
    const next = [...order];
    const [moved] = next.splice(from, 1);
    next.splice(at, 0, moved);
    const itemId = moved.playlistItemID;
    if (itemId == null) return;
    order = next;
    api
      .movePlaylistItem(id, itemId, next[at - 1]?.playlistItemID ?? null)
      .catch((err) => app.error(err))
      .finally(() => data.reload());
  }

  // Rename / describe.
  let editing = $state<{ title: string; summary: string } | null>(null);

  async function save() {
    if (!editing) return;
    const { title, summary } = editing;
    if (!title.trim()) return;
    editing = null;
    if (await act(api.updatePlaylist(id, title.trim(), summary.trim()), "Playlist updated")) data.reload();
  }

  async function clear() {
    const ok = await app.confirm("Clear this playlist?", {
      text: "Every item is removed; the playlist itself stays.",
      action: "Clear",
      danger: true,
    });
    if (ok && (await act(api.clearPlaylist(id), "Playlist cleared"))) data.reload();
  }

  async function remove() {
    const ok = await app.confirm("Delete this playlist?", {
      text: "The playlist is removed from the server. Its items are not affected.",
      action: "Delete",
      danger: true,
    });
    if (ok && (await act(api.deletePlaylist(id), "Playlist deleted"))) nav.replace({ name: "playlists" });
  }
</script>

{#if data.error && data.data != null}<Status error={data.error} retry={() => data.reload()} stale />{/if}
{#if data.data === undefined || (data.data === null && data.error)}
  <Status loading={!data.error} error={data.error} retry={() => data.reload()} fill />
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
          <button
            class="btn quiet lg"
            onclick={() => (editing = { title: playlist.title ?? "", summary: playlist.summary ?? "" })}>Edit</button
          >
          {#if !playlist.smart && items.length}
            <button class="btn quiet lg" onclick={clear}>Clear</button>
          {/if}
          <button class="btn quiet lg danger" onclick={remove}>Delete</button>
        </div>
      </div>
    </header>

    {#if order.length === 0}
      <Status title="This playlist is empty" />
    {:else}
      <div class="rows" role="presentation" ondragleave={(e) => e.currentTarget === e.target && (target = null)}>
        {#each order as item, i (item.playlistItemID ?? i)}
          <div
            class={[
              "list-row clickable entry",
              {
                dragging: dragging === i,
                "drop-before": target?.index === i && !target.after,
                "drop-after": target?.index === i && target.after,
              },
            ]}
            role="button"
            tabindex="0"
            draggable={reorderable}
            ondragstart={(e) => {
              dragging = i;
              if (e.dataTransfer) {
                e.dataTransfer.effectAllowed = "move";
                e.dataTransfer.setData("text/plain", String(item.playlistItemID ?? ""));
              }
            }}
            ondragend={() => (dragging = target = null)}
            ondragover={(e) => over(e, i)}
            ondrop={drop}
            onclick={() => nav.push(routeFor(item))}
            onkeydown={(e) => e.key === "Enter" && nav.push(routeFor(item))}
            oncontextmenu={(e) => openMenu(item, e)}
          >
            <span class={["n faint num", { grip: reorderable }]}>
              <span class="i">{i + 1}</span>
              {#if reorderable}<GripVertical size={13} />{/if}
            </span>
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
              <button
                class="icon-btn"
                title="Play next"
                onclick={(e) => {
                  e.stopPropagation();
                  enqueue(item, true);
                }}><ListStart size={15} /></button
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

{#if editing}
  <div class="modal-backdrop" role="presentation" onclick={() => (editing = null)}>
    <div
      class="modal"
      role="dialog"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.key === "Escape" && (editing = null)}
    >
      <h2>Edit playlist</h2>
      <label class="field">
        <span>Title</span>
        <!-- svelte-ignore a11y_autofocus -->
        <input class="input" autofocus bind:value={editing.title} onkeydown={(e) => e.key === "Enter" && save()} />
      </label>
      <label class="field">
        <span>Description</span>
        <textarea class="input" rows="4" bind:value={editing.summary}></textarea>
      </label>
      <div class="foot">
        <button class="btn quiet" onclick={() => (editing = null)}>Cancel</button>
        <button class="btn primary" disabled={!editing.title.trim()} onclick={save}>Save</button>
      </div>
    </div>
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
    display: flex;
    justify-content: flex-end;
  }
  /* The number turns into a grip on hover when rows can be dragged. */
  .n :global(svg) {
    display: none;
  }
  .entry:hover .n.grip .i {
    display: none;
  }
  .entry:hover .n.grip :global(svg) {
    display: block;
    cursor: grab;
  }
  .entry.dragging {
    opacity: 0.4;
  }
  .entry.drop-before {
    box-shadow: inset 0 1px 0 var(--accent);
  }
  .entry.drop-after {
    box-shadow: inset 0 -1px 0 var(--accent);
  }
  .dur {
    margin-right: 8px;
    font-size: 12px;
  }

  .field {
    display: grid;
    gap: 6px;
    margin-top: 16px;
  }
  .field span {
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  textarea.input {
    height: auto;
    padding: 7px 10px;
    resize: vertical;
    line-height: 1.45;
  }
</style>
