<script lang="ts">
  // Downloaded items for offline playback, grouped by show / artist. Works
  // without a server session (the Gate renders it when the server is down),
  // so it only ever talks to the download commands.
  import { FolderOpen, Pause, Play, RotateCw, Trash } from "@lucide/svelte";
  import Img from "../components/Img.svelte";
  import Status from "../components/Status.svelte";
  import { api } from "../lib/api";
  import { bytes, plural } from "../lib/format";
  import { downloadImg } from "../lib/images";
  import * as plex from "../lib/plex";
  import { nav } from "../lib/router.svelte";
  import { act, app } from "../lib/state.svelte";
  import type { Download, Metadata } from "../lib/types";

  /** Offline: never link to server pages. */
  let { offline = false }: { offline?: boolean } = $props();

  interface Group {
    key: string;
    title: string;
    kind: string;
    items: Download[];
  }

  function groupOf(d: Download): [key: string, title: string] {
    const k = plex.kind(d.item);
    const scope = [d.account, d.serverId, k];
    if (k === "episode" || k === "track") {
      const t = d.item.grandparentTitle ?? "Unknown";
      return [JSON.stringify([...scope, t]), t];
    }
    return [JSON.stringify(scope), k === "movie" ? "Movies" : "Other"];
  }

  // Episodes by season/episode, tracks by album/disc/track, the rest by title.
  function sortKey(m: Metadata): (string | number)[] {
    switch (plex.kind(m)) {
      case "episode":
        return [m.parentIndex ?? 0, m.index ?? 0];
      case "track":
        return [m.parentTitle ?? "", m.parentIndex ?? 0, m.index ?? 0];
      default:
        return [m.titleSort ?? m.title ?? ""];
    }
  }

  function compare(a: (string | number)[], b: (string | number)[]) {
    for (let i = 0; i < a.length; i++) {
      const x = a[i];
      const y = b[i];
      const c = typeof x === "string" ? x.localeCompare(String(y)) : x - (y as number);
      if (c) return c;
    }
    return 0;
  }

  // Movies first, then shows and artists by name, anything else last.
  const rank = (g: Group) => (g.kind === "movie" ? 0 : ["episode", "track"].includes(g.kind) ? 1 : 2);

  let groups = $derived.by(() => {
    const map = new Map<string, Group>();
    for (const d of app.downloads) {
      const [key, title] = groupOf(d);
      let g = map.get(key);
      if (!g) map.set(key, (g = { key, title, kind: plex.kind(d.item), items: [] }));
      g.items.push(d);
    }
    const list = [...map.values()];
    for (const g of list) g.items.sort((a, b) => compare(sortKey(a.item), sortKey(b.item)));
    return list.sort((a, b) => rank(a) - rank(b) || a.title.localeCompare(b.title));
  });

  let onDisk = $derived(app.downloads.reduce((n, d) => n + (d.state === "done" ? d.size : d.received), 0));
  let active = $derived(app.downloads.filter((d) => d.state === "queued" || d.state === "downloading").length);

  /** Inside a group the show / artist name is already in the heading. */
  function rowTitle(m: Metadata) {
    switch (plex.kind(m)) {
      case "episode":
        return `${plex.episodeCode(m)} · ${plex.title(m)}`;
      case "track":
        return plex.title(m);
      default:
        return plex.fullTitle(m);
    }
  }

  /** "Album · 1.2 GB of 2.9 GB · Server" */
  function rowSub(d: Download) {
    const m = d.item;
    const size = d.state === "downloading" ? `${bytes(d.received)} of ${bytes(d.size)}` : d.size > 0 ? bytes(d.size) : "";
    return [plex.kind(m) === "track" ? m.parentTitle : "", size, d.serverName].filter(Boolean).join(" · ");
  }

  function groupSub(g: Group) {
    const n = g.items.length;
    const size = g.items.reduce((s, d) => s + d.size, 0);
    const what = g.kind === "episode" ? "episode" : g.kind === "track" ? "track" : g.kind === "movie" ? "movie" : "item";
    return `${plural(n, what)} · ${bytes(size)}`;
  }

  const linkable = (d: Download) => !offline && !!d.item.ratingKey && app.session?.serverId === d.serverId;
  const pct = (d: Download) => (d.size > 0 ? Math.min(100, Math.floor((d.received / d.size) * 100)) : 0);

  const playIds = (ids: string[]) => act(api.playDownloads(ids));

  async function remove(d: Download) {
    const done = d.state === "done";
    const ok = await app.confirm(done ? "Delete download?" : "Remove from downloads?", {
      text: done
        ? `${plex.fullTitle(d.item)} will be removed from this computer.`
        : `${plex.fullTitle(d.item)} will stop downloading.`,
      action: done ? "Delete" : "Remove",
      danger: true,
    });
    if (ok) act(api.removeDownload(d.id));
  }
</script>

<div class="page">
  <div class="page-head">
    <div>
      <h1>Downloads</h1>
      <div class="sub num">
        {#if app.downloads.length}
          {plural(app.downloads.length, "item")} · {bytes(onDisk)} on disk{#if active} · {active} in progress{/if}
        {:else}
          Saved on this computer for offline playback.
        {/if}
      </div>
    </div>
    <button class="btn" onclick={() => act(api.openDownloadsFolder())}><FolderOpen size={14} />Open folder</button>
  </div>

  {#if app.downloads.length === 0}
    <Status
      title="Nothing downloaded"
      text="Choose “Download” from any movie, episode, season or album menu to keep a copy on this computer and play it without a connection."
    />
  {:else}
    {#each groups as g (g.key)}
      {@const done = g.items.filter((d) => d.state === "done").map((d) => d.id)}
      <div class="section-head">
        <h2>{g.title}</h2>
        <div class="row-flex">
          <span class="faint small num">{groupSub(g)}</span>
          {#if (g.kind === "episode" || g.kind === "track") && done.length}
            <button class="more" onclick={() => playIds(done)}>Play all</button>
          {/if}
        </div>
      </div>
      <div class="rows">
        {#each g.items as d (d.id)}
          {@const m = d.item}
          {@const rk = m.ratingKey}
          <div class={["list-row", "download", d.state]}>
            <div class={["thumb", { square: plex.kind(m) === "track" }]}>
              {#if d.poster}<Img src={downloadImg(d.id)} />{/if}
            </div>
            <div class="grow">
              {#if linkable(d) && rk}
                <button class="t title-link" onclick={() => nav.push({ name: "item", ratingKey: rk })}>{rowTitle(m)}</button>
              {:else}
                <div class="t">{rowTitle(m)}</div>
              {/if}
              {#if d.state === "failed"}
                <div class="s err" title={d.error}>{d.error ?? "Download failed"}</div>
              {:else}
                <div class="s num">{rowSub(d)}</div>
              {/if}
              {#if d.state === "downloading"}
                <div class="bar"><i style:width="{pct(d)}%"></i></div>
              {/if}
            </div>
            <div class="end">
              {#if d.state === "queued" || d.state === "downloading"}
                <button class="icon-btn" title="Pause download" onclick={() => act(api.pauseDownload(d.id))}><Pause /></button>
              {:else if d.state === "paused"}
                <span class="faint">Paused</span>
                <button class="icon-btn" title="Resume download" onclick={() => act(api.retryDownload(d.id))}><Play /></button>
              {/if}
              {#if d.state === "downloading"}
                <span class="state num">{pct(d)}%</span>
              {:else if d.state === "queued"}
                <span class="state">Waiting</span>
              {:else if d.state === "failed"}
                <button class="btn sm quiet" onclick={() => act(api.retryDownload(d.id))}><RotateCw size={13} />Retry</button>
              {:else if d.state === "done"}
                <button class="icon-btn" title="Play" onclick={() => playIds([d.id])}><Play size={15} /></button>
              {/if}
              <button class="icon-btn" title={d.state === "done" ? "Delete download" : "Remove"} onclick={() => remove(d)}>
                <Trash size={14} />
              </button>
            </div>
          </div>
        {/each}
      </div>
    {/each}
  {/if}
</div>

<style>
  .small {
    font-size: 12px;
  }
  .section-head .row-flex {
    gap: 14px;
  }
  .download .grow {
    min-width: 0;
  }
  .download .t,
  .download .s {
    display: block;
  }
  .title-link {
    max-width: 100%;
    padding: 0;
    border: 0;
    background: none;
    text-align: left;
    cursor: pointer;
  }
  .title-link:hover {
    text-decoration: underline;
    text-decoration-color: var(--line-2);
    text-underline-offset: 3px;
  }
  .download.queued .t,
  .download.failed .t {
    color: var(--text-2);
  }
  .err {
    color: color-mix(in srgb, var(--danger) 65%, var(--text-3));
  }
  .bar {
    max-width: 360px;
    margin-top: 6px;
  }
  .state {
    min-width: 52px;
    padding-right: 4px;
    font-size: 12px;
    text-align: right;
  }
</style>
