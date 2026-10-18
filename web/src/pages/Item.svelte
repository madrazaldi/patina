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
  import { Badge } from "$lib/components/ui/badge";
  import { Button } from "$lib/components/ui/button";
  import * as ButtonGroup from "$lib/components/ui/button-group";
  import * as Card from "$lib/components/ui/card";
  import { Label } from "$lib/components/ui/label";
  import * as NativeSelect from "$lib/components/ui/native-select";
  import { Separator } from "$lib/components/ui/separator";
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
  {@const metaBits = [...ratings(item).map((r) => r.label), ...metaLine(item)]}

  <div class="relative">
    <div class="backdrop pointer-events-none absolute inset-x-0 top-0 h-[520px] overflow-hidden opacity-50">
      <Img src={img(plex.backdrop(item), 1920, 1080, server)} />
      <div class="to-background absolute inset-0 bg-linear-to-b from-background/10 via-background/60"></div>
    </div>

    <div class="page relative pt-32">
      <header class="flex items-end gap-9">
        <div
          class={[
            "bg-muted ring-border relative shrink-0 overflow-hidden rounded-xl shadow-2xl ring-1 [&_img]:size-full [&_img]:object-cover",
            shape === "landscape" ? "aspect-video w-[380px]" : shape === "square" ? "aspect-square w-60" : "aspect-[2/3] w-[210px]",
          ]}
        >
          {#if poster}<Img src={poster} class={spoiler ? "blur-lg brightness-75" : ""} />{/if}
        </div>
        <div class="min-w-0 flex-1 pb-1">
          {#if crumbs(item).length}
            <div class="text-muted-foreground mb-3 flex items-center gap-2 text-sm">
              {#each crumbs(item) as [label, rk], i (rk)}
                {#if i > 0}<span class="text-muted-foreground/50">/</span>{/if}
                <button class="link no-underline hover:underline" onclick={() => nav.push({ name: "item", ratingKey: rk, server })}>{label}</button>
              {/each}
            </div>
          {/if}
          <h1 class="text-4xl leading-[1.08] font-semibold tracking-tight text-balance">{plex.title(item)}</h1>
          {#if item.originalTitle && item.originalTitle !== item.title}
            <div class="text-muted-foreground mt-1.5 text-lg">{item.originalTitle}</div>
          {/if}
          <div class="text-muted-foreground mt-3.5 flex flex-wrap items-center gap-x-2 gap-y-2 text-sm">
            {#each metaBits as m, i (i)}
              {#if i > 0}<span class="text-muted-foreground/50">·</span>{/if}<span>{m}</span>
            {/each}
            {#if item.contentRating}<Badge variant="outline" class="ml-1">{item.contentRating}</Badge>{/if}
            {#if mediaLabel}<Badge variant="secondary">{mediaLabel}</Badge>{/if}
            {#if server}<Badge variant="outline" title="This copy is on another server of your account">On {serverName}</Badge>{/if}
          </div>

          <div class="mt-6 flex flex-wrap items-center gap-2">
            {#if plex.canPlay(item)}
              <Button size="lg" class="h-10 gap-2 px-5" onclick={() => play(item, { mediaIndex })}>
                <Play fill="currentColor" />{progress != null ? "Resume" : "Play"}
                {#if left}<span class="font-normal opacity-60">{left}</span>{/if}
              </Button>
              {#if progress != null}
                <Button variant="outline" size="lg" class="h-10 px-4" onclick={() => play(item, { resume: false, mediaIndex })}><RotateCcw />From start</Button>
              {/if}
              {#if plex.isContainer(item)}
                <Button variant="outline" size="lg" class="h-10 px-4" onclick={() => play(item, { shuffle: true })}><Shuffle />Shuffle</Button>
              {/if}
              {#if kind === "artist" || kind === "album"}
                <Button variant="outline" size="lg" class="h-10 px-4" onclick={() => play(item, { radio: true })}><Radio />Radio</Button>
              {/if}
            {/if}
            <ButtonGroup.Root class="ml-1">
              {#if plex.hasWatchState(item)}
                <Button
                  variant="outline"
                  size="icon-lg"
                  class={["size-10", { on: watched }, watched && "text-tint"]}
                  title={watched ? "Mark as unwatched" : "Mark as watched"}
                  onclick={() => setWatched(item, !watched)}><Check /></Button
                >
              {/if}
              {#if discover}
                {@const id = discover}
                <Button
                  variant="outline"
                  size="icon-lg"
                  class={["size-10", { on: watchlisted }, watchlisted && "text-tint"]}
                  title={watchlisted ? "Remove from watchlist" : "Add to watchlist"}
                  onclick={() => toggleWatchlist(id)}
                >
                  {#if watchlisted}<BookmarkCheck />{:else}<Bookmark />{/if}
                </Button>
              {/if}
              {#if plex.supportsTracks(item) && !server}
                <Button variant="outline" size="icon-lg" class="size-10" title="Audio & subtitles for many episodes" onclick={() => (app.tracksTarget = item)}>
                  <Captions />
                </Button>
                <Button variant="outline" size="icon-lg" class="size-10" title="Playback preferences for this show" onclick={() => (app.prefsTarget = item)}>
                  <SlidersHorizontal />
                </Button>
              {/if}
              {#if plex.canPlay(item) && kind !== "photo" && kind !== "photoalbum"}
                <Button variant="outline" size="icon-lg" class="size-10" title="Play next" onclick={() => enqueue(item, true)}><ListStart /></Button>
              {/if}
              {#if kind !== "collection" && kind !== "playlist" && !server}
                <Button variant="outline" size="icon-lg" class="size-10" title="Add to playlist" onclick={() => (app.playlistTarget = item)}><ListPlus /></Button>
              {/if}
              {#if ["movie", "episode", "track", "show", "season", "album", "artist"].includes(kind) && !server}
                {@const busy = saved?.state === "queued" || saved?.state === "downloading"}
                <Button
                  variant="outline"
                  size="icon-lg"
                  class={["size-10", { on: saved?.state === "done", busy }, saved?.state === "done" && "text-tint", busy && "disabled:opacity-100"]}
                  title={downloadLabel(saved)}
                  disabled={busy}
                  onclick={() => onDownload(item, saved)}
                >
                  {#if saved?.state === "done"}
                    <Check />
                  {:else if busy}
                    <span class="text-muted-foreground text-[10.5px] font-semibold tabular-nums">{saved && saved.size > 0 ? `${Math.floor((saved.received / saved.size) * 100)}%` : "…"}</span>
                  {:else}
                    <Download />
                  {/if}
                </Button>
              {/if}
              {#if ["show", "season", "playlist"].includes(kind) && !server}
                <Button variant="outline" size="icon-lg" class="size-10" title="Keep next unwatched downloaded" onclick={() => (app.keepTarget = item)}>
                  <CloudDownload />
                </Button>
              {/if}
              <Button variant="outline" size="icon-lg" class="size-10" title="More" onclick={(e) => openMenu(item, e)}><Ellipsis /></Button>
            </ButtonGroup.Root>
            {#if ["movie", "show", "episode", "season", "album", "track", "artist"].includes(kind) && !server}
              <StarRating
                value={item.userRating ?? 0}
                onrate={(v) => act(api.rate(item.ratingKey ?? "", v), undefined, true)}
              />
            {/if}
          </div>
        </div>
      </header>

      <div class={["mt-10 grid items-start gap-10", info.tags.length || info.plain.length ? "lg:grid-cols-[minmax(0,1fr)_360px]" : ""]}>
        <div class="min-w-0 max-w-[72ch]">
          {#if item.tagline}<div class="text-muted-foreground mb-3 text-lg italic">{item.tagline}</div>{/if}
          {#if summary}
            <button
              class={[
                "cursor-text border-0 bg-transparent p-0 text-left text-[15px] leading-relaxed transition-[filter] select-text",
                summary.length > 420 && !expanded ? "line-clamp-5 cursor-pointer" : "block",
                spoiler && "blur-[5px] hover:blur-none",
              ]}
              onclick={() => (expanded = !expanded)}>{summary}</button
            >
          {/if}

          {#if ["movie", "episode", "clip", "track"].includes(kind) && item.Media.length}
            <StreamPickers {item} bind:mediaIndex />
          {/if}
        </div>

        {#if info.tags.length || info.plain.length}
          <Card.Root size="sm" class="bg-card/60">
            <Card.Content>
              <dl class="kv grid-cols-[92px_minmax(0,1fr)] text-sm">
                {#each info.tags as [label, list] (label)}
                  <dt>{label}</dt>
                  <dd>
                    {#each list.slice(0, 8) as t, i (i)}
                      {@const r = tagRoute(item, t)}
                      {#if i > 0}{", "}{/if}{#if r}<button class="link text-foreground no-underline hover:underline" onclick={() => nav.push(r)}>{t.tag}</button>{:else}{t.tag}{/if}
                    {/each}
                  </dd>
                {/each}
                {#each info.plain as [label, value] (label)}
                  <dt>{label}</dt>
                  <dd class="selectable">{value}</dd>
                {/each}
              </dl>
            </Card.Content>
          </Card.Root>
        {/if}
      </div>

      {#if kind === "collection" && app.owned && !server}
        {@const mode = item.collectionMode ?? -1}
        {@const order = Number(item.collectionSort ?? 0)}
        <div class="mt-8 flex flex-wrap items-end gap-4">
          <div class="flex w-48 flex-col gap-1.5">
            <Label class="text-muted-foreground text-xs" for="collection-mode">Show in library</Label>
            <NativeSelect.Root id="collection-mode" class="w-full" value={String(mode)} onchange={(e) => setCollection(item, { mode: Number(e.currentTarget.value) })}>
              <NativeSelect.Option value="-1">Library default</NativeSelect.Option>
              <NativeSelect.Option value="0">Hide collection</NativeSelect.Option>
              <NativeSelect.Option value="1">Hide items</NativeSelect.Option>
              <NativeSelect.Option value="2">Show both</NativeSelect.Option>
            </NativeSelect.Root>
          </div>
          <div class="flex w-48 flex-col gap-1.5">
            <Label class="text-muted-foreground text-xs" for="collection-sort">Sort</Label>
            <NativeSelect.Root id="collection-sort" class="w-full" value={String(order)} onchange={(e) => setCollection(item, { sort: Number(e.currentTarget.value) })}>
              <NativeSelect.Option value="0">Release date</NativeSelect.Option>
              <NativeSelect.Option value="1">Title</NativeSelect.Option>
              <NativeSelect.Option value="2">Custom</NativeSelect.Option>
            </NativeSelect.Root>
          </div>
          <div class="flex-1"></div>
          <Button variant="destructive" onclick={() => deleteCollection(item)}><Trash />Delete collection</Button>
        </div>
      {/if}

      {#if ["show", "season", "artist", "album", "collection", "photoalbum"].includes(kind)}
        <Separator class="mt-10" />
        <Children {item} sort={item.collectionSort} />
      {/if}

      {#if item.Role.length}
        <div class="section-head"><h2>{kind === "album" || kind === "artist" ? "Featuring" : "Cast"}</h2></div>
        <div class="-mx-(--gutter) flex gap-4 overflow-x-auto px-(--gutter) pb-1 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
          {#each item.Role.slice(0, 40) as role, i (i)}
            {@const r = tagRoute(item, role)}
            <button class="group flex w-24 flex-none cursor-pointer flex-col items-center border-0 bg-transparent p-0 text-center disabled:cursor-default" disabled={!r} onclick={() => r && nav.push(r)}>
              <span class="bg-muted text-muted-foreground ring-border mb-2.5 grid size-[76px] place-items-center overflow-hidden rounded-full text-sm font-semibold ring-1 transition-shadow group-enabled:group-hover:ring-2 group-enabled:group-hover:ring-tint/60 [&_img]:size-full [&_img]:object-cover">
                {#if role.thumb}<Img src={img(role.thumb, 160, 160, server)} />{:else}{initials(role.tag ?? "")}{/if}
              </span>
              <span class="ellipsis w-full text-xs font-medium">{role.tag}</span>
              {#if role.role}<span class="text-muted-foreground ellipsis w-full text-xs">{role.role}</span>{/if}
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
  .backdrop :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: center 25%;
  }
</style>
