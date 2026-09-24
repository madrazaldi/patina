<script lang="ts">
  // Right-click / "…" menu for any item.
  import {
    Bookmark,
    Captions,
    CircleCheck,
    Circle,
    ExternalLink,
    Layers,
    ListPlus,
    Play,
    RefreshCw,
    RotateCcw,
    ScanLine,
    Shuffle,
    Tv,
    User,
  } from "@lucide/svelte";
  import { api } from "../lib/api";
  import { play, setWatched } from "../lib/actions.svelte";
  import * as plex from "../lib/plex";
  import { nav } from "../lib/router.svelte";
  import { act, app } from "../lib/state.svelte";

  let el: HTMLDivElement | undefined = $state();
  let width = $state(0);
  let height = $state(0);

  let menu = $derived(app.menu);
  let item = $derived(menu?.item);
  let x = $derived(menu ? Math.max(8, Math.min(menu.x, window.innerWidth - width - 8)) : 0);
  let y = $derived(menu ? Math.max(8, Math.min(menu.y, window.innerHeight - height - 8)) : 0);

  const close = () => (app.menu = null);
  const run = (f: () => unknown) => () => {
    close();
    f();
  };
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

{#if menu && item}
  {@const kind = plex.kind(item)}
  {@const resume = plex.progress(item) != null}
  {@const watched = plex.isWatched(item)}
  {@const rk = item.ratingKey ?? ""}
  {@const discover = app.hasTv ? plex.discoverId(item) : null}
  {@const machine = app.session?.machineIdentifier}
  <div
    class="scrim"
    role="presentation"
    onclick={close}
    oncontextmenu={(e) => {
      e.preventDefault();
      close();
    }}
  ></div>
  <div class="menu" bind:this={el} bind:clientWidth={width} bind:clientHeight={height} style:left="{x}px" style:top="{y}px">
    <div class="head">
      <b>{plex.fullTitle(item)}</b>
      {#if plex.subtitle(item)}<span>{plex.subtitle(item)}</span>{/if}
    </div>

    {#if plex.canPlay(item)}
      <button onclick={run(() => play(item))}><Play />{resume ? "Resume" : "Play"}</button>
      {#if resume}
        <button onclick={run(() => play(item, { resume: false }))}><RotateCcw />Play from beginning</button>
      {/if}
      {#if plex.isContainer(item)}
        <button onclick={run(() => play(item, { shuffle: true }))}><Shuffle />Shuffle</button>
      {/if}
      <hr />
    {/if}

    {#if (kind === "episode" || kind === "track") && item.grandparentRatingKey}
      {@const show = item.grandparentRatingKey}
      <button onclick={run(() => nav.push({ name: "item", ratingKey: show }))}>
        {#if kind === "track"}<User />Go to artist{:else}<Tv />Go to show{/if}
      </button>
    {/if}
    {#if ["episode", "track", "season"].includes(kind) && item.parentRatingKey}
      {@const parent = item.parentRatingKey}
      <button onclick={run(() => nav.push({ name: "item", ratingKey: parent }))}>
        <Layers />{kind === "track" ? "Go to album" : kind === "season" ? "Go to show" : "Go to season"}
      </button>
    {/if}

    {#if plex.hasWatchState(item)}
      <button onclick={run(() => setWatched(item, !watched))}>
        {#if watched}<Circle />Mark as unwatched{:else}<CircleCheck />Mark as watched{/if}
      </button>
    {/if}
    {#if kind !== "playlist" && kind !== "collection"}
      <button onclick={run(() => (app.playlistTarget = item))}><ListPlus />Add to playlist…</button>
    {/if}
    {#if plex.supportsTracks(item)}
      <button onclick={run(() => (app.tracksTarget = item))}><Captions />Audio & subtitles…</button>
    {/if}
    {#if discover}
      <button onclick={run(() => act(api.setWatchlisted(discover, true), "Added to watchlist"))}>
        <Bookmark />Add to watchlist
      </button>
    {/if}

    {#if app.owned && kind !== "playlist"}
      <hr />
      <button onclick={run(() => act(api.refreshMetadata(rk), "Refreshing metadata…"))}><RefreshCw />Refresh metadata</button>
      <button onclick={run(() => act(api.analyzeMetadata(rk), "Analyzing…"))}><ScanLine />Analyze</button>
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
  </div>
{/if}
