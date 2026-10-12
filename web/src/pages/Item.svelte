<script lang="ts">
  import {
    Bookmark,
    BookmarkCheck,
    Captions,
    Check,
    CloudDownload,
    Download,
    Ellipsis,
    ListPlus,
    ListStart,
    Play,
    Radio,
    RotateCcw,
    Shuffle,
    SlidersHorizontal,
    Trash,
  } from "@lucide/svelte";
  import Children from "../components/Children.svelte";
  import HubRow from "../components/HubRow.svelte";
  import Img from "../components/Img.svelte";
  import Row from "../components/Row.svelte";
  import StarRating from "../components/StarRating.svelte";
  import Status from "../components/Status.svelte";
  import StreamPickers from "../components/StreamPickers.svelte";
  import { download, downloaded, enqueue, openMenu, play, setWatched } from "../lib/actions.svelte";
  import { api } from "../lib/api";
  import { ago, airDate, bitrate, bytes, date, duration, initials, remaining } from "../lib/format";
  import { img } from "../lib/images";
  import * as plex from "../lib/plex";
  import { resource } from "../lib/resource.svelte";
  import { browseRoute, nav, type Route } from "../lib/router.svelte";
  import { act, app } from "../lib/state.svelte";
  import type { Download as Saved, Hub, Metadata, Tag } from "../lib/types";

  // `server` is the machine id of another account server the item was found on (cross-server search).
  let { ratingKey, server }: { ratingKey: string; server?: string } = $props();
  let serverName = $derived(server ? (app.session?.servers.find((s) => s.id === server)?.name ?? "another server") : "");

  const data = resource(() => {
    app.revision;
    return api.metadata(ratingKey, server);
  });

  // More hubs (sonically similar, similar artists, ...), fetched once the kind is known.
  let loadedKind = $derived(data.data ? plex.kind(data.data) : null);
  const extra = resource(() => {
    const k = loadedKind;
    app.revision;
    return k ? api.itemHubs(ratingKey, k, server) : Promise.resolve([] as Hub[]);
  });

  let mediaIndex = $state(0);
  let expanded = $state(false);

  // Start on the version the automatic playback preferences pick (resolution, codec, HDR).
  let versionOf = $state("");
  $effect(() => {
    const item = data.data;
    if (!item || (item.Media?.length ?? 0) < 2 || !["movie", "episode"].includes(plex.kind(item))) return;
    const key = item.ratingKey ?? "";
    if (versionOf === key) return;
    versionOf = key;
    api.preferredVersion($state.snapshot(item) as Metadata).then(
      (i) => versionOf === key && i < item.Media.length && (mediaIndex = i),
      () => {},
    );
  });

  // Watchlist state lives on plex.tv.
  let discover = $derived(data.data && !server && app.hasTv && ["movie", "show"].includes(plex.kind(data.data)) ? plex.discoverId(data.data) : null);
  let watchlisted = $state<boolean | null>(null);
  $effect(() => {
    const id = discover;
    watchlisted = null;
    if (id) api.watchlisted(id).then((on) => discover === id && (watchlisted = on), () => {});
  });

  async function toggleWatchlist(id: string) {
    const on = !watchlisted;
    if (await act(api.setWatchlisted(id, on), on ? "Added to watchlist" : "Removed from watchlist")) watchlisted = on;
  }

  /** Item-page hubs not already shown under Related. */
  function extraHubs(item: Metadata, hubs: Hub[]) {
    const related = item.Related?.Hub ?? [];
    const seen = (h: Hub) =>
      related.some(
        (r) =>
          (h.hubIdentifier != null && r.hubIdentifier === h.hubIdentifier) ||
          (h.title != null && r.title?.toLowerCase() === h.title.toLowerCase()),
      );
    return hubs.filter((h) => h.Metadata.length && !seen(h));
  }

  function downloadLabel(d: Saved | undefined) {
    if (!d) return "Download";
    if (d.state === "done") return "Downloaded — show downloads";
    if (d.state === "paused") return "Download paused — resume";
    if (d.state === "failed") return `Download failed${d.error ? `: ${d.error}` : ""} — retry`;
    return d.state === "queued" ? "Waiting to download" : "Downloading";
  }

  function onDownload(item: Metadata, d: Saved | undefined) {
    if (d?.state === "done") nav.push({ name: "downloads" });
    else if (d?.state === "failed" || d?.state === "paused") act(api.retryDownload(d.id));
    else if (!d) download(item);
  }

  async function deleteCollection(item: Metadata) {
    const ok = await app.confirm(`Delete “${plex.title(item)}”?`, {
      text: "The collection is removed; its items stay in the library.",
      action: "Delete",
      danger: true,
    });
    if (ok && (await act(api.deleteCollection(item.ratingKey ?? ""), "Collection deleted", true))) nav.back();
  }

  const setCollection = (item: Metadata, s: { mode?: number; sort?: number }) =>
    act(api.collectionSettings(item.ratingKey ?? "", s), "Collection updated", true);

  function tagRoute(item: Metadata, tag: Tag): Route | null {
    if (item.librarySectionID == null || !tag.filter || server) return null;
    return browseRoute(`/library/sections/${item.librarySectionID}/all?${tag.filter}`, tag.tag ?? "");
  }

  function crumbs(item: Metadata): [string, string][] {
    const out: [string, string][] = [];
    const k = plex.kind(item);
    if ((k === "episode" || k === "track") && item.grandparentRatingKey)
      out.push([item.grandparentTitle ?? "", item.grandparentRatingKey]);
    if (["episode", "track", "season", "album"].includes(k) && item.parentRatingKey)
      out.push([item.parentTitle ?? "", item.parentRatingKey]);
    return out;
  }

  function metaLine(item: Metadata) {
    const k = plex.kind(item);
    const out: string[] = [];
    if (k === "episode") {
      out.push(plex.episodeCode(item));
      if (item.originallyAvailableAt) out.push(airDate(item.originallyAvailableAt));
    } else if (item.year != null) out.push(String(item.year));
    if (item.duration != null && k !== "show" && k !== "season") out.push(duration(item.duration));
    if (k === "show" && item.childCount != null) out.push(`${item.childCount} season${item.childCount === 1 ? "" : "s"}`);
    if (item.leafCount != null && ["show", "season", "album"].includes(k))
      out.push(`${item.leafCount} ${k === "album" ? "tracks" : "episodes"}`);
    return out;
  }

  function ratings(item: Metadata) {
    const out: { label: string; title: string }[] = [];
    const rt = (i?: string) => !!i?.includes("rottentomatoes");
    if (item.rating != null)
      out.push({ title: "Critic rating", label: rt(item.ratingImage) ? `${Math.round(item.rating * 10)}% critics` : `★ ${item.rating.toFixed(1)}` });
    if (item.audienceRating != null)
      out.push({
        title: "Audience rating",
        label: rt(item.audienceRatingImage) ? `${Math.round(item.audienceRating * 10)}% audience` : `★ ${item.audienceRating.toFixed(1)}`,
      });
    return out;
  }

  function facts(item: Metadata) {
    const tags: [string, Tag[]][] = (
      [
        ["Directed by", item.Director],
        ["Written by", item.Writer],
        ["Genres", item.Genre],
        ["Collections", item.Collection],
        ["Moods", item.Mood],
        ["Styles", item.Style],
        ["Country", item.Country],
        ["Labels", item.Label],
      ] as [string, Tag[]][]
    ).filter(([, l]) => l.length);
    const plain: [string, string][] = [];
    if (item.studio) plain.push(["Studio", item.studio]);
    const m = item.Media[0];
    if (m) {
      const p = m.Part[0];
      const bits = [
        m.container?.toUpperCase(),
        m.width && m.height ? `${m.width}×${m.height}` : null,
        m.bitrate ? bitrate(m.bitrate) : null,
        p?.size ? bytes(p.size) : null,
      ].filter(Boolean);
      if (bits.length) plain.push(["File", bits.join(" · ")]);
      const a = p && plex.selectedAudio(p);
      if (a) plain.push(["Audio", `${(a.codec ?? "").toUpperCase()} ${a.channels != null ? plex.channelLabel(a.channels) : ""}`.trim()]);
      if (app.owned && p?.file) plain.push(["Path", p.file]);
    }
    if (item.addedAt) plain.push(["Added", date(item.addedAt)]);
    if (item.lastViewedAt) {
      const n = item.viewCount ?? 0;
      plain.push(["Last watched", `${ago(item.lastViewedAt)} · ${n} play${n === 1 ? "" : "s"}`]);
    }
    return { tags, plain };
  }
</script>

{#if data.error && data.data !== undefined}<Status error={data.error} retry={() => data.reload()} stale />{/if}
  {#if data.data === undefined}
  <Status loading={!data.error} error={data.error} retry={() => data.reload()} fill />
{:else if data.data === null}
  <Status title="Not found" text="This item no longer exists on the server." fill />
{:else}
  {@const item = data.data}
  {@const kind = plex.kind(item)}
  {@const shape = kind === "episode" || kind === "clip" ? "landscape" : ["album", "artist", "playlist", "photoalbum", "track"].includes(kind) ? "square" : "poster"}
  {@const poster =
    shape === "landscape"
      ? img(item.thumb, 840, 472, server)
      : shape === "square"
        ? img(item.thumb ?? item.parentThumb, 600, 600, server)
        : img(item.thumb, 500, 750, server)}
  {@const spoiler = app.spoilers && kind === "episode" && !plex.isWatched(item)}
  {@const progress = plex.progress(item)}
  {@const left = remaining(item)}
  {@const media = item.Media[mediaIndex] ?? item.Media[0]}
  {@const mediaLabel = media ? plex.mediaLabel(media) : ""}
  {@const summary = item.summary ?? ""}
  {@const info = facts(item)}
  {@const watched = plex.isWatched(item)}
  {@const saved = downloaded(item)}

  <div class="detail">
    <div class="backdrop"><Img src={img(plex.backdrop(item), 1920, 1080, server)} /></div>

    <div class="page inner">
      <header class={["head", `is-${shape}`]}>
        <div class="poster">
          {#if poster}<Img src={poster} class={spoiler ? "spoiler" : ""} />{/if}
        </div>
        <div class="info">
          {#if crumbs(item).length}
            <div class="crumbs">
              {#each crumbs(item) as [label, rk], i (rk)}
                {#if i > 0}<span class="faint">/</span>{/if}
                <button class="link" onclick={() => nav.push({ name: "item", ratingKey: rk, server })}>{label}</button>
              {/each}
            </div>
          {/if}
          <h1 class="serif">{plex.title(item)}</h1>
          {#if item.originalTitle && item.originalTitle !== item.title}
            <div class="original serif">{item.originalTitle}</div>
          {/if}
          <div class="meta">
            {#each [...ratings(item).map((r) => r.label), ...metaLine(item)] as m, i (i)}
              {#if i > 0}<span class="dot">·</span>{/if}<span>{m}</span>
            {/each}
            {#if item.contentRating}<span class="tag">{item.contentRating}</span>{/if}
            {#if mediaLabel}<span class="tag">{mediaLabel}</span>{/if}
            {#if server}<span class="tag" title="This copy is on another server of your account">On {serverName}</span>{/if}
          </div>

          <div class="actions">
            {#if plex.canPlay(item)}
              <button class="btn primary lg" onclick={() => play(item, { mediaIndex })}>
                <Play size={14} fill="currentColor" />{progress != null ? "Resume" : "Play"}
                {#if left}<span class="left">{left}</span>{/if}
              </button>
              {#if progress != null}
                <button class="btn lg" onclick={() => play(item, { resume: false, mediaIndex })}><RotateCcw size={14} />From start</button>
              {/if}
              {#if plex.isContainer(item)}
                <button class="btn lg" onclick={() => play(item, { shuffle: true })}><Shuffle size={14} />Shuffle</button>
              {/if}
              {#if kind === "artist" || kind === "album"}
                <button class="btn lg" onclick={() => play(item, { radio: true })}><Radio size={14} />Radio</button>
              {/if}
            {/if}
            <span class="spacer"></span>
            {#if plex.hasWatchState(item)}
              <button
                class={["icon-btn outlined", { on: watched }]}
                title={watched ? "Mark as unwatched" : "Mark as watched"}
                onclick={() => setWatched(item, !watched)}><Check /></button
              >
            {/if}
            {#if discover}
              {@const id = discover}
              <button
                class={["icon-btn outlined", { on: watchlisted }]}
                title={watchlisted ? "Remove from watchlist" : "Add to watchlist"}
                onclick={() => toggleWatchlist(id)}
              >
                {#if watchlisted}<BookmarkCheck />{:else}<Bookmark />{/if}
              </button>
            {/if}
            {#if plex.supportsTracks(item) && !server}
              <button class="icon-btn outlined" title="Audio & subtitles for many episodes" onclick={() => (app.tracksTarget = item)}>
                <Captions />
              </button>
              <button class="icon-btn outlined" title="Playback preferences for this show" onclick={() => (app.prefsTarget = item)}>
                <SlidersHorizontal />
              </button>
            {/if}
            {#if plex.canPlay(item) && kind !== "photo" && kind !== "photoalbum"}
              <button class="icon-btn outlined" title="Play next" onclick={() => enqueue(item, true)}><ListStart /></button>
            {/if}
            {#if kind !== "collection" && kind !== "playlist" && !server}
              <button class="icon-btn outlined" title="Add to playlist" onclick={() => (app.playlistTarget = item)}><ListPlus /></button>
            {/if}
            {#if ["movie", "episode", "track", "show", "season", "album", "artist"].includes(kind) && !server}
              {@const busy = saved?.state === "queued" || saved?.state === "downloading"}
              <button
                class={["icon-btn outlined", { on: saved?.state === "done", busy }]}
                title={downloadLabel(saved)}
                disabled={busy}
                onclick={() => onDownload(item, saved)}
              >
                {#if saved?.state === "done"}
                  <Check />
                {:else if busy}
                  <span class="pct num">{saved && saved.size > 0 ? `${Math.floor((saved.received / saved.size) * 100)}%` : "…"}</span>
                {:else}
                  <Download />
                {/if}
              </button>
            {/if}
            {#if ["show", "season", "playlist"].includes(kind) && !server}
              <button class="icon-btn outlined" title="Keep next unwatched downloaded" onclick={() => (app.keepTarget = item)}>
                <CloudDownload />
              </button>
            {/if}
            <button class="icon-btn outlined" title="More" onclick={(e) => openMenu(item, e)}><Ellipsis /></button>
            {#if ["movie", "show", "episode", "season", "album", "track", "artist"].includes(kind) && !server}
              <StarRating
                value={item.userRating ?? 0}
                onrate={(v) => act(api.rate(item.ratingKey ?? "", v), undefined, true)}
              />
            {/if}
          </div>
        </div>
      </header>

      <div class="body">
        {#if item.tagline}<div class="tagline serif">{item.tagline}</div>{/if}
        {#if summary}
          <button
            class={["summary", { clamp: summary.length > 420 && !expanded, spoiler }]}
            onclick={() => (expanded = !expanded)}>{summary}</button
          >
        {/if}

        {#if ["movie", "episode", "clip", "track"].includes(kind) && item.Media.length}
          <StreamPickers {item} bind:mediaIndex />
        {/if}

        {#if info.tags.length || info.plain.length}
          <dl class="kv facts">
            {#each info.tags as [label, list] (label)}
              <dt>{label}</dt>
              <dd>
                {#each list.slice(0, 8) as t, i (i)}
                  {@const r = tagRoute(item, t)}
                  {#if i > 0}{", "}{/if}{#if r}<button class="link plain" onclick={() => nav.push(r)}>{t.tag}</button>{:else}{t.tag}{/if}
                {/each}
              </dd>
            {/each}
            {#each info.plain as [label, value] (label)}
              <dt>{label}</dt>
              <dd class="selectable">{value}</dd>
            {/each}
          </dl>
        {/if}
      </div>

      {#if kind === "collection" && app.owned && !server}
        {@const mode = item.collectionMode ?? -1}
        {@const order = Number(item.collectionSort ?? 0)}
        <div class="collection-tools">
          <label>
            <span>Show in library</span>
            <select class="select" value={String(mode)} onchange={(e) => setCollection(item, { mode: Number(e.currentTarget.value) })}>
              <option value="-1">Library default</option>
              <option value="0">Hide collection</option>
              <option value="1">Hide items</option>
              <option value="2">Show both</option>
            </select>
          </label>
          <label>
            <span>Sort</span>
            <select class="select" value={String(order)} onchange={(e) => setCollection(item, { sort: Number(e.currentTarget.value) })}>
              <option value="0">Release date</option>
              <option value="1">Title</option>
              <option value="2">Custom</option>
            </select>
          </label>
          <span class="grow"></span>
          <button class="btn quiet danger" onclick={() => deleteCollection(item)}><Trash size={14} />Delete collection</button>
        </div>
      {/if}

      {#if ["show", "season", "artist", "album", "collection", "photoalbum"].includes(kind)}
        <Children {item} sort={item.collectionSort} />
      {/if}

      {#if item.Role.length}
        <div class="section-head"><h2>{kind === "album" || kind === "artist" ? "Featuring" : "Cast"}</h2></div>
        <div class="cast">
          {#each item.Role.slice(0, 40) as role, i (i)}
            {@const r = tagRoute(item, role)}
            <button class="person" disabled={!r} onclick={() => r && nav.push(r)}>
              <span class="face">
                {#if role.thumb}<Img src={img(role.thumb, 160, 160, server)} />{:else}{initials(role.tag ?? "")}{/if}
              </span>
              <span class="name ellipsis">{role.tag}</span>
              {#if role.role}<span class="role ellipsis">{role.role}</span>{/if}
            </button>
          {/each}
        </div>
      {/if}

      {#if item.Extras?.Metadata.length && !server}
        <Row title="Extras" items={item.Extras.Metadata} shape="landscape" />
      {/if}
      {#each (item.Related?.Hub ?? []).filter((h) => h.Metadata.length) as hub, i (`${hub.hubIdentifier}:${i}`)}
        <HubRow {hub} />
      {/each}
      {#each extraHubs(item, extra.data ?? []) as hub, i (`x:${hub.hubIdentifier}:${i}`)}
        <HubRow {hub} />
      {/each}
    </div>
  </div>
{/if}

<style>
  .detail {
    position: relative;
  }
  .backdrop {
    position: absolute;
    inset: 0 0 auto;
    height: 460px;
    overflow: hidden;
    opacity: 0.4;
  }
  .backdrop :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: center 25%;
  }
  .backdrop::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, rgba(12, 12, 12, 0.2) 0%, var(--bg) 100%);
  }
  .inner {
    position: relative;
    padding-top: 150px;
  }
  .head {
    display: grid;
    grid-template-columns: 210px minmax(0, 1fr);
    gap: 36px;
    align-items: end;
  }
  .head.is-landscape {
    grid-template-columns: 380px minmax(0, 1fr);
  }
  .head.is-square {
    grid-template-columns: 240px minmax(0, 1fr);
  }
  .poster {
    aspect-ratio: 2 / 3;
    border-radius: var(--r);
    overflow: hidden;
    background: var(--surface);
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.06);
  }
  .is-landscape .poster {
    aspect-ratio: 16 / 9;
  }
  .is-square .poster {
    aspect-ratio: 1;
  }
  .poster :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .poster :global(img.spoiler) {
    filter: blur(16px) brightness(0.7);
  }
  .crumbs {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 10px;
    font-size: 12.5px;
  }
  .crumbs .link {
    text-decoration: none;
  }
  .crumbs .link:hover {
    text-decoration: underline;
  }
  h1 {
    font-size: 46px;
    font-weight: 400;
    line-height: 1.04;
    letter-spacing: -0.015em;
  }
  .original {
    margin-top: 6px;
    font-size: 18px;
    color: var(--text-2);
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-top: 14px;
    color: var(--text-2);
  }
  .dot {
    color: var(--text-3);
  }
  .meta .tag {
    margin-left: 4px;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 24px;
  }
  .left {
    margin-left: 4px;
    font-weight: 400;
    opacity: 0.6;
  }
  .spacer {
    width: 8px;
  }
  .pct {
    font-size: 10.5px;
    font-weight: 600;
    color: var(--text-2);
  }
  .icon-btn.busy:disabled {
    opacity: 1;
  }
  .collection-tools {
    display: flex;
    align-items: flex-end;
    gap: 14px;
    margin-top: 32px;
  }
  .collection-tools label {
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 170px;
  }
  .collection-tools label span {
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .body {
    max-width: 880px;
    margin-top: 32px;
  }
  .tagline {
    margin-bottom: 12px;
    font-size: 18px;
    font-style: italic;
    color: var(--text-2);
  }
  .summary {
    display: block;
    max-width: 72ch;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text);
    font-size: 14.5px;
    line-height: 1.65;
    text-align: left;
    cursor: text;
    user-select: text;
    -webkit-user-select: text;
    transition: filter 0.2s;
  }
  .summary.clamp {
    display: -webkit-box;
    -webkit-line-clamp: 5;
    line-clamp: 5;
    -webkit-box-orient: vertical;
    overflow: hidden;
    cursor: pointer;
  }
  .summary.spoiler {
    filter: blur(5px);
  }
  .summary.spoiler:hover {
    filter: none;
  }
  .facts {
    margin-top: 30px;
    padding-top: 20px;
    border-top: 1px solid var(--line);
  }
  .link.plain {
    text-decoration: none;
  }
  .link.plain:hover {
    text-decoration: underline;
  }
  .cast {
    display: flex;
    gap: 16px;
    overflow-x: auto;
    margin: 0 calc(var(--gutter) * -1);
    padding: 0 var(--gutter) 4px;
    scrollbar-width: none;
  }
  .cast::-webkit-scrollbar {
    display: none;
  }
  .person {
    display: flex;
    flex-direction: column;
    align-items: center;
    flex: none;
    width: 92px;
    padding: 0;
    border: 0;
    background: none;
    text-align: center;
    cursor: pointer;
  }
  .person:disabled {
    cursor: default;
  }
  .face {
    display: grid;
    place-items: center;
    width: 72px;
    height: 72px;
    margin-bottom: 8px;
    border-radius: 50%;
    overflow: hidden;
    background: var(--surface);
    color: var(--text-3);
    font-weight: 600;
  }
  .face :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .name {
    width: 100%;
    font-size: 12px;
  }
  .role {
    width: 100%;
    font-size: 11.5px;
    color: var(--text-3);
  }
  .person:not(:disabled):hover .name {
    text-decoration: underline;
    text-underline-offset: 3px;
  }
</style>
