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
  import type { Download, Metadata, SmartOverview, SmartPolicyView } from "../lib/types";

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

  // Smart downloads: policies that keep the next unwatched items downloaded
  // (crates/core/src/smart.rs). Reloaded whenever downloads change.
  let smart = $state<SmartOverview>();
  $effect(() => {
    app.downloads;
    api.smartDownloads().then(
      (o) => (smart = o),
      () => {},
    );
  });
  const GIB = 1024 ** 3;
  let smartBusy = $state(false);

  const patch = (p: SmartPolicyView, change: { keep?: number; deleteWatched?: boolean; enabled?: boolean }) =>
    act(api.updateSmartDownload(p.id, change));

  async function dropPolicy(p: SmartPolicyView) {
    const ok = await app.confirm(`Stop keeping “${p.title}” downloaded?`, {
      text: p.downloads
        ? `The ${plural(p.downloads, "download")} it fetched stay on this computer until you delete them.`
        : "Nothing it fetched is on this computer.",
      action: "Stop keeping",
    });
    if (ok) await act(api.removeSmartDownload(p.id, false));
  }

  async function runNow() {
    smartBusy = true;
    try {
      const r = await api.runSmartDownloads();
      const bits = [
        r.added ? `${plural(r.added, "item")} queued` : "",
        r.removed ? `${plural(r.removed, "download")} removed` : "",
        r.overBudget ? `${r.overBudget} left out (storage budget)` : "",
        ...r.errors,
      ].filter(Boolean);
      app.toast(bits.join(" · ") || "Downloads already match the policies");
    } catch (e) {
      app.error(e);
    }
    smartBusy = false;
  }

  function setBudget(text: string) {
    const gb = Number(text);
    const value = text.trim() === "" || !(gb > 0) ? null : Math.round(gb * GIB);
    return act(api.setDownloadBudget(value));
  }

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

  {#if smart}
    <div class="section-head">
      <h2>Smart downloads</h2>
      <div class="row-flex">
        <span class="faint small">Keep the next unwatched episodes downloaded</span>
        {#if smart.policies.some((p) => p.active)}
          <button class="more" disabled={smartBusy} onclick={runNow}>Run now</button>
        {/if}
      </div>
    </div>
    <div class="rows">
      {#each smart.policies as p (p.id)}
        <div class={["setting", "policy", { dim: !p.enabled || !p.active }]}>
          <div class="label">
            <b>{p.title}</b>
            <span class="num">
              {p.target} · {p.serverName}{p.active ? "" : " (another account or server)"} · {plural(p.downloads, "download")}{p.bytes ? `, ${bytes(p.bytes)}` : ""}{p.enabled ? "" : " · paused"}
            </span>
          </div>
          <div class="control">
            <label class="inline" title="Unwatched items to keep">
              Keep
              <input
                class="input num"
                type="number"
                min="1"
                max="50"
                style="width: 64px"
                value={p.keep}
                onchange={(e) => {
                  const n = Math.floor(Number(e.currentTarget.value));
                  if (n >= 1 && n <= 50) patch(p, { keep: n });
                  else e.currentTarget.value = String(p.keep);
                }}
              />
            </label>
            <label class="inline" title="Delete downloads once they are watched">
              <input type="checkbox" checked={p.deleteWatched} onchange={(e) => patch(p, { deleteWatched: e.currentTarget.checked })} />
              Remove watched
            </label>
            <button class="icon-btn" title={p.enabled ? "Pause" : "Resume"} onclick={() => patch(p, { enabled: !p.enabled })}>
              {#if p.enabled}<Pause />{:else}<Play />{/if}
            </button>
            <button class="icon-btn" title="Stop keeping downloaded" onclick={() => dropPolicy(p)}><Trash size={14} /></button>
          </div>
        </div>
      {/each}
      <div class="setting">
        <div class="label">
          <b>Storage budget</b>
          <span class="num">
            {bytes(smart.used)} used by downloads{smart.budget != null ? ` of ${bytes(smart.budget)}` : ""}. Smart downloads stop adding when the budget is full.
          </span>
        </div>
        <div class="control">
          <input
            class="input num"
            type="number"
            min="0"
            step="1"
            style="width: 90px"
            placeholder="No limit"
            value={smart.budget != null ? Math.round((smart.budget / GIB) * 10) / 10 : ""}
            onchange={(e) => setBudget(e.currentTarget.value)}
          />
          GB
        </div>
      </div>
      {#if smart.policies.length === 0}
        <div class="setting">
          <div class="label"><span>Open a show, season or playlist and choose “Keep next unwatched downloaded”.</span></div>
        </div>
      {/if}
    </div>
  {/if}

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
  .policy.dim b {
    color: var(--text-2);
  }
  .inline {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--text-2);
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
