<script lang="ts">
  import { Ellipsis, GripVertical, ListStart, Play, Shuffle, X } from "@lucide/svelte";
  import { Badge } from "$lib/components/ui/badge";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import { Textarea } from "$lib/components/ui/textarea";
  import Img from "../components/Img.svelte";
  import Status from "../components/Status.svelte";
  import { enqueue, openMenu, play } from "../lib/actions.svelte";
  import { modal } from "../lib/dialog";
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
  {@const cutOff = data.data.total > items.length}
  <div class="page">
    <header class="mb-9 flex items-end gap-8">
      <div class="bg-muted ring-border aspect-square w-[200px] shrink-0 overflow-hidden rounded-xl shadow-xl ring-1 [&_img]:size-full [&_img]:object-cover">
        <Img src={img(playlist.composite ?? playlist.thumb, 480, 480)} />
      </div>
      <div class="min-w-0 flex-1">
        <Badge variant="secondary">{playlist.smart ? "Smart playlist" : "Playlist"}</Badge>
        <h1 class="mt-3 mb-2 text-4xl leading-[1.08] font-semibold tracking-tight text-balance">{playlist.title}</h1>
        <div class="text-muted-foreground text-sm">
          {cutOff ? `Showing the first ${items.length.toLocaleString()} of ${data.data.total.toLocaleString()} items` : `${items.length} items`} · {duration(total)}
        </div>
        {#if playlist.summary}<p class="text-muted-foreground mt-2.5 max-w-[70ch]">{playlist.summary}</p>{/if}
        <div class="mt-5 flex flex-wrap items-center gap-2">
          <Button size="lg" class="h-10 gap-2 px-5" disabled={!items.length} onclick={() => play(playlist)}>
            <Play fill="currentColor" />Play
          </Button>
          <Button variant="outline" size="lg" class="h-10 px-4" disabled={!items.length} onclick={() => play(playlist, { shuffle: true })}>
            <Shuffle />Shuffle
          </Button>
          <Button
            variant="ghost"
            size="lg"
            class="h-10 px-4"
            onclick={() => (editing = { title: playlist.title ?? "", summary: playlist.summary ?? "" })}>Edit</Button
          >
          {#if items.length}
            <Button variant="ghost" size="lg" class="h-10 px-4" onclick={() => (app.keepTarget = playlist)}>Keep downloaded</Button>
          {/if}
          {#if !playlist.smart && items.length}
            <Button variant="ghost" size="lg" class="h-10 px-4" onclick={clear}>Clear</Button>
          {/if}
          <Button variant="destructive" size="lg" class="h-10 px-4" onclick={remove}>Delete</Button>
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
            <span class={["n text-muted-foreground num", { grip: reorderable }]}>
              <span class="i">{i + 1}</span>
              {#if reorderable}<GripVertical size={13} />{/if}
            </span>
            <div class="thumb"><Img src={img(plex.poster(item), 96, 144)} /></div>
            <div class="grow">
              <div class="t">{plex.fullTitle(item)}</div>
              <div class="s">{plex.subtitle(item)}</div>
            </div>
            <div class="end">
              {#if item.duration}<span class="mr-2 text-xs">{duration(item.duration)}</span>{/if}
              <Button
                variant="ghost"
                size="icon-sm"
                class="text-muted-foreground"
                title="Play"
                onclick={(e) => {
                  e.stopPropagation();
                  play(item);
                }}><Play /></Button
              >
              <Button
                variant="ghost"
                size="icon-sm"
                class="text-muted-foreground"
                title="Play next"
                onclick={(e) => {
                  e.stopPropagation();
                  enqueue(item, true);
                }}><ListStart /></Button
              >
              {#if !playlist.smart && item.playlistItemID != null}
                {@const itemId = item.playlistItemID}
                <Button
                  variant="ghost"
                  size="icon-sm"
                  class="text-muted-foreground"
                  title="Remove from playlist"
                  onclick={(e) => {
                    e.stopPropagation();
                    act(api.removeFromPlaylist(id, itemId), undefined, true);
                  }}><X /></Button
                >
              {/if}
              <Button variant="ghost" size="icon-sm" class="text-muted-foreground" title="More" onclick={(e) => openMenu(item, e)}><Ellipsis /></Button>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
{/if}

{#if editing}
  <dialog class="modal" use:modal={() => (editing = null)} aria-labelledby="playlist-edit-title">
    <h2 id="playlist-edit-title">Edit playlist</h2>
    <div class="mt-4 grid gap-2">
      <Label class="text-muted-foreground text-xs" for="playlist-edit-name">Title</Label>
      <Input id="playlist-edit-name" data-initial-focus bind:value={editing.title} onkeydown={(e) => e.key === "Enter" && save()} />
    </div>
    <div class="mt-4 grid gap-2">
      <Label class="text-muted-foreground text-xs" for="playlist-edit-summary">Description</Label>
      <Textarea id="playlist-edit-summary" rows={4} bind:value={editing.summary} />
    </div>
    <div class="foot">
      <Button variant="ghost" onclick={() => (editing = null)}>Cancel</Button>
      <Button disabled={!editing.title.trim()} onclick={save}>Save</Button>
    </div>
  </dialog>
{/if}

<style>
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
    box-shadow: inset 0 1px 0 var(--tint);
  }
  .entry.drop-after {
    box-shadow: inset 0 -1px 0 var(--tint);
  }
</style>
