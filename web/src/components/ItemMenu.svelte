<script lang="ts">
  // Right-click / "…" menu for any item.
  import {
    ArrowDownToLine,
    Bookmark,
    Captions,
    CircleCheck,
    Circle,
    Crosshair,
    Download,
    CloudDownload,
    Eraser,
    ExternalLink,
    EyeOff,
    FolderPlus,
    Image,
    Layers,
    ListEnd,
    ListPlus,
    ListStart,
    Pencil,
    Play,
    Radio,
    RefreshCw,
    RotateCcw,
    ScanLine,
    Shuffle,
    SlidersHorizontal,
    Trash,
    Tv,
    User,
    Wallpaper,
  } from "@lucide/svelte";
  import { api } from "../lib/api";
  import { menu as menunav } from "../lib/menu";
  import { download, downloaded, enqueue, play, setWatched } from "../lib/actions.svelte";
  import * as plex from "../lib/plex";
  import { nav } from "../lib/router.svelte";
  import { act, app, captureScope } from "../lib/state.svelte";
  import type { Metadata } from "../lib/types";

  let el: HTMLDivElement | undefined = $state();
  let width = $state(0);
  let height = $state(0);

  let menu = $derived(app.menu);
  let item = $derived(menu?.item);
  let x = $derived(menu ? Math.max(8, Math.min(menu.x, window.innerWidth - width - 8)) : 0);
  let y = $derived(menu ? Math.max(8, Math.min(menu.y, window.innerHeight - height - 8)) : 0);

  const close = () => (app.menu = null);
  // Run before closing: `item` and the template's {@const}s are read lazily,
  // so they are gone once the menu closes.
  const run = (f: () => unknown) => () => {
    try {
      f();
    } finally {
      close();
    }
  };

  const LIBRARY = ["movie", "show", "season", "episode", "artist", "album", "track", "collection"];

  async function remove(target: Metadata) {
    const rk = target.ratingKey ?? "";
    const guard = captureScope();
    const collection = plex.kind(target) === "collection";
    const ok = await app.confirm(`Delete “${plex.title(target)}”?`, {
      text: collection
        ? "The collection is removed; its items stay in the library."
        : "This deletes the media files from disk. It can't be undone.",
      action: "Delete",
      danger: true,
    });
    // The rating key belongs to the server the menu opened on.
    if (!ok || !guard.valid()) return;
    const onPage = nav.route.name === "item" && nav.route.ratingKey === rk;
    if (await act(collection ? api.deleteCollection(rk) : api.deleteItem(rk), "Deleted", true)) {
      if (onPage) nav.back();
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

{#if menu && item}
  {@const kind = plex.kind(item)}
  {@const resume = plex.progress(item) != null}
  {@const watched = plex.isWatched(item)}
  {@const rk = item.ratingKey ?? ""}
  {@const discover = app.hasTv ? plex.discoverId(item) : null}
  {@const machine = app.session?.machineIdentifier}
  {@const queueable = plex.canPlay(item) && !["photo", "photoalbum"].includes(kind)}
  {@const saved = downloaded(item)}
  {@const inLibrary = LIBRARY.includes(kind)}
  {@const foreign = item.patinaServer}
  {@const where = foreign ? (app.session?.servers.find((s) => s.id === foreign)?.name ?? "another server") : ""}
  <div
    class="scrim"
    role="presentation"
    onclick={close}
    oncontextmenu={(e) => {
      e.preventDefault();
      close();
    }}
  ></div>
  <div class="menu" use:menunav={close} aria-label={plex.fullTitle(item)} bind:this={el} bind:clientWidth={width} bind:clientHeight={height} style:left="{x}px" style:top="{y}px">
    <div class="head">
      <b>{plex.fullTitle(item)}</b>
      {#if plex.subtitle(item)}<span>{plex.subtitle(item)}</span>{/if}
      {#if where}<span>On {where}</span>{/if}
    </div>

    {#if plex.canPlay(item)}
      <button onclick={run(() => play(item))}><Play />{resume ? "Resume" : "Play"}</button>
      {#if resume}
        <button onclick={run(() => play(item, { resume: false }))}><RotateCcw />Play from beginning</button>
      {/if}
      {#if plex.isContainer(item)}
        <button onclick={run(() => play(item, { shuffle: true }))}><Shuffle />Shuffle</button>
      {/if}
      {#if queueable}
        <button onclick={run(() => enqueue(item, true))}><ListStart />Play next</button>
        <button onclick={run(() => enqueue(item, false))}><ListEnd />Add to queue</button>
      {/if}
      {#if ["artist", "album", "track"].includes(kind)}
        <button onclick={run(() => play(item, { radio: true }))}><Radio />Start radio</button>
      {/if}
      <hr />
    {/if}

    {#if (kind === "episode" || kind === "track") && item.grandparentRatingKey}
      {@const show = item.grandparentRatingKey}
      <button onclick={run(() => nav.push({ name: "item", ratingKey: show, server: foreign }))}>
        {#if kind === "track"}<User />Go to artist{:else}<Tv />Go to show{/if}
      </button>
    {/if}
    {#if ["episode", "track", "season"].includes(kind) && item.parentRatingKey}
      {@const parent = item.parentRatingKey}
      <button onclick={run(() => nav.push({ name: "item", ratingKey: parent, server: foreign }))}>
        <Layers />{kind === "track" ? "Go to album" : kind === "season" ? "Go to show" : "Go to season"}
      </button>
    {/if}

    {#if plex.hasWatchState(item)}
      <button onclick={run(() => setWatched(item, !watched))}>
        {#if watched}<Circle />Mark as unwatched{:else}<CircleCheck />Mark as watched{/if}
      </button>
    {/if}
    <!-- What follows edits the connected server, so it is not offered for an item found on another one. -->
    {#if !foreign}
    {#if resume}
      <button onclick={run(() => act(api.clearProgress(rk)))}><Eraser />Clear progress</button>
    {/if}
    {#if kind === "episode" || (kind === "movie" && resume)}
      <button onclick={run(() => act(api.removeFromContinueWatching(rk)))}>
        <EyeOff />Remove from Continue Watching
      </button>
    {/if}
    {#if kind !== "playlist" && kind !== "collection"}
      <button onclick={run(() => (app.playlistTarget = item))}><ListPlus />Add to playlist…</button>
    {/if}
    {#if app.owned && ["movie", "show", "artist", "album"].includes(kind)}
      <button onclick={run(() => (app.collectionTarget = item))}><FolderPlus />Add to collection…</button>
    {/if}
    {#if plex.supportsTracks(item)}
      <button onclick={run(() => (app.tracksTarget = item))}><Captions />Audio & subtitles…</button>
      <button onclick={run(() => (app.prefsTarget = item))}><SlidersHorizontal />Playback preferences…</button>
    {/if}
    {#if discover}
      <button onclick={run(() => act(api.setWatchlisted(discover, true), "Added to watchlist"))}>
        <Bookmark />Add to watchlist
      </button>
    {/if}
    {#if ["movie", "episode", "track", "show", "season", "album", "artist"].includes(kind)}
      {#if saved?.state === "done"}
        <button disabled><ArrowDownToLine />Downloaded</button>
      {:else if saved?.state === "paused" || saved?.state === "failed"}
        <button onclick={run(() => act(api.retryDownload(saved!.id)))}><Download />Resume / retry download</button>
      {:else if saved}
        <button disabled><Download />Downloading…</button>
      {:else}
        <button onclick={run(() => download(item))}><Download />Download</button>
      {/if}
    {/if}
    {#if ["show", "season", "playlist"].includes(kind)}
      <button onclick={run(() => (app.keepTarget = item))}><CloudDownload />Keep next unwatched downloaded…</button>
    {/if}

    {#if app.owned && kind !== "playlist"}
      <hr />
      {#if inLibrary}
        <button onclick={run(() => (app.editTarget = item))}><Pencil />Edit…</button>
        {#if kind !== "track"}
          <button onclick={run(() => (app.artworkTarget = { item, kind: "poster" }))}><Image />Choose poster…</button>
        {/if}
        {#if ["movie", "show", "artist", "album", "collection"].includes(kind)}
          <button onclick={run(() => (app.artworkTarget = { item, kind: "art" }))}><Wallpaper />Choose background…</button>
        {/if}
        {#if ["movie", "show", "artist", "album"].includes(kind)}
          <button onclick={run(() => (app.matchTarget = item))}><Crosshair />Fix match…</button>
        {/if}
      {/if}
      <button onclick={run(() => act(api.refreshMetadata(rk), "Refreshing metadata…"))}><RefreshCw />Refresh metadata</button>
      <button onclick={run(() => act(api.analyzeMetadata(rk), "Analyzing…"))}><ScanLine />Analyze</button>
      {#if inLibrary}
        <button class="danger" onclick={run(() => remove(item))}><Trash />Delete…</button>
      {/if}
    {/if}
    {#if machine}
      <button
        onclick={run(() =>
          api.openUrl(
            `https://app.plex.tv/desktop#!/server/${machine}/details?key=${encodeURIComponent(`/library/metadata/${rk}`)}`,
          ),
        )}><ExternalLink />Open in Plex Web</button
      >
    {/if}
    {/if}
  </div>
{/if}

<style>
  button:disabled {
    color: var(--text-3);
    cursor: default;
  }
  button:disabled:hover {
    background: none;
  }
  .danger :global(svg) {
    color: inherit;
  }
</style>
