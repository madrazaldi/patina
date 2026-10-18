<script lang="ts">
  // Downloaded items for offline playback, grouped by show / artist. Works
  // without a server session (the Gate renders it when the server is down),
  // so it only ever talks to the download commands.
  import { Download as DownloadIcon, FolderOpen, Pause, Play, RotateCw, Trash } from "@lucide/svelte";
  import Img from "../components/Img.svelte";
  import { Badge } from "$lib/components/ui/badge";
  import { Button } from "$lib/components/ui/button";
  import * as Card from "$lib/components/ui/card";
  import { Checkbox } from "$lib/components/ui/checkbox";
  import * as Empty from "$lib/components/ui/empty";
  import * as Field from "$lib/components/ui/field";
  import { Input } from "$lib/components/ui/input";
  import * as Item from "$lib/components/ui/item";
  import { Label } from "$lib/components/ui/label";
  import { Progress } from "$lib/components/ui/progress";
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
          {plural(app.downloads.length, "item")} · {bytes(onDisk)} on disk{#if active}{" · "}{active} in progress{/if}
        {:else}
          Saved on this computer for offline playback.
        {/if}
      </div>
    </div>
    <Button variant="outline" onclick={() => act(api.openDownloadsFolder())}><FolderOpen />Open folder</Button>
  </div>

  {#if smart}
    <Card.Root class="mb-2 gap-0 pb-0">
      <Card.Header class="border-b">
        <h2 class="text-base leading-snug font-semibold tracking-tight">Smart downloads</h2>
        <Card.Description>Keep the next unwatched episodes downloaded</Card.Description>
        {#if smart.policies.some((p) => p.active)}
          <Card.Action><Button variant="outline" size="sm" disabled={smartBusy} onclick={runNow}>Run now</Button></Card.Action>
        {/if}
      </Card.Header>
      <Card.Content class="divide-y px-0">
        {#each smart.policies as p (p.id)}
          <div class="flex items-center justify-between gap-6 px-4 py-3.5">
            <Field.Content>
              <Field.Title class={!p.enabled || !p.active ? "text-muted-foreground" : ""}>{p.title}</Field.Title>
              <Field.Description class="text-[13px] tabular-nums">
                {p.target} · {p.serverName}{p.active ? "" : " (another account or server)"} · {plural(p.downloads, "download")}{p.bytes ? `, ${bytes(p.bytes)}` : ""}{p.enabled ? "" : " · paused"}
              </Field.Description>
            </Field.Content>
            <div class="flex flex-none items-center gap-4">
              <Label class="text-muted-foreground text-xs font-normal" title="Unwatched items to keep">
                Keep
                <Input
                  class="w-16 tabular-nums"
                  type="number"
                  min="1"
                  max="50"
                  value={p.keep}
                  onchange={(e) => {
                    const n = Math.floor(Number(e.currentTarget.value));
                    if (n >= 1 && n <= 50) patch(p, { keep: n });
                    else e.currentTarget.value = String(p.keep);
                  }}
                />
              </Label>
              <div class="flex items-center gap-2" title="Delete downloads once they are watched">
                <Checkbox id="remove-watched-{p.id}" checked={p.deleteWatched} onCheckedChange={(on) => patch(p, { deleteWatched: on })} />
                <Label for="remove-watched-{p.id}" class="text-muted-foreground text-xs font-normal">Remove watched</Label>
              </div>
              <div class="flex items-center gap-0.5">
                <Button variant="ghost" size="icon" title={p.enabled ? "Pause" : "Resume"} onclick={() => patch(p, { enabled: !p.enabled })}>
                  {#if p.enabled}<Pause />{:else}<Play />{/if}
                </Button>
                <Button variant="ghost" size="icon" title="Stop keeping downloaded" onclick={() => dropPolicy(p)}><Trash /></Button>
              </div>
            </div>
          </div>
        {/each}
        <div class="flex items-center justify-between gap-6 px-4 py-3.5">
          <Field.Content>
            <Field.Title>Storage budget</Field.Title>
            <Field.Description class="text-[13px] tabular-nums">
              {bytes(smart.used)} used by downloads{smart.budget != null ? ` of ${bytes(smart.budget)}` : ""}. Smart downloads stop adding when the budget is full.
            </Field.Description>
          </Field.Content>
          <div class="text-muted-foreground flex flex-none items-center gap-2 text-sm">
            <Input
              class="w-24 tabular-nums"
              type="number"
              min="0"
              step="1"
              placeholder="No limit"
              value={smart.budget != null ? Math.round((smart.budget / GIB) * 10) / 10 : ""}
              onchange={(e) => setBudget(e.currentTarget.value)}
            />
            GB
          </div>
        </div>
        {#if smart.policies.length === 0}
          <div class="px-4 py-3.5">
            <Field.Description class="text-[13px]">Open a show, season or playlist and choose “Keep next unwatched downloaded”.</Field.Description>
          </div>
        {/if}
      </Card.Content>
    </Card.Root>
  {/if}

  {#if app.downloads.length === 0}
    <Empty.Root class="mt-8 border">
      <Empty.Header>
        <Empty.Media variant="icon"><DownloadIcon /></Empty.Media>
        <Empty.Title>Nothing downloaded</Empty.Title>
        <Empty.Description>
          Choose “Download” from any movie, episode, season or album menu to keep a copy on this computer and play it without a connection.
        </Empty.Description>
      </Empty.Header>
    </Empty.Root>
  {:else}
    {#each groups as g (g.key)}
      {@const done = g.items.filter((d) => d.state === "done").map((d) => d.id)}
      <div class="mt-9 mb-3 flex items-center justify-between gap-4">
        <h2 class="text-base leading-snug font-semibold tracking-tight">{g.title}</h2>
        <div class="flex items-center gap-3">
          <span class="text-muted-foreground text-xs tabular-nums">{groupSub(g)}</span>
          {#if (g.kind === "episode" || g.kind === "track") && done.length}
            <Button variant="outline" size="sm" onclick={() => playIds(done)}>Play all</Button>
          {/if}
        </div>
      </div>
      <Card.Root class="gap-0 py-0">
        <Card.Content class="divide-y px-0">
          {#each g.items as d (d.id)}
            {@const m = d.item}
            {@const rk = m.ratingKey}
            <Item.Root class="rounded-none border-0">
              <Item.Media class={["bg-muted h-12 overflow-hidden rounded-sm", plex.kind(m) === "track" ? "size-12" : "w-8"]}>
                {#if d.poster}<Img class="size-full object-cover" src={downloadImg(d.id)} />{/if}
              </Item.Media>
              <Item.Content class="min-w-0">
                <Item.Title class={["max-w-full", (d.state === "queued" || d.state === "failed") && "text-muted-foreground"]}>
                  {#if linkable(d) && rk}
                    <button class="truncate text-left underline-offset-4 hover:underline" onclick={() => nav.push({ name: "item", ratingKey: rk })}>{rowTitle(m)}</button>
                  {:else}
                    <span class="truncate">{rowTitle(m)}</span>
                  {/if}
                </Item.Title>
                {#if d.state === "failed"}
                  <Item.Description class="text-destructive/80 text-xs" title={d.error}>{d.error ?? "Download failed"}</Item.Description>
                {:else}
                  <Item.Description class="text-xs tabular-nums">{rowSub(d)}</Item.Description>
                {/if}
                {#if d.state === "downloading"}
                  <Progress value={pct(d)} class="mt-1.5 max-w-90 **:data-[slot=progress-indicator]:bg-tint" />
                {/if}
              </Item.Content>
              <Item.Actions class="gap-1">
                {#if d.state === "queued" || d.state === "downloading"}
                  <Button variant="ghost" size="icon" title="Pause download" onclick={() => act(api.pauseDownload(d.id))}><Pause /></Button>
                {:else if d.state === "paused"}
                  <Badge variant="secondary">Paused</Badge>
                  <Button variant="ghost" size="icon" title="Resume download" onclick={() => act(api.retryDownload(d.id))}><Play /></Button>
                {/if}
                {#if d.state === "downloading"}
                  <span class="text-muted-foreground min-w-12 pr-1 text-right text-xs tabular-nums">{pct(d)}%</span>
                {:else if d.state === "queued"}
                  <span class="text-muted-foreground min-w-12 pr-1 text-right text-xs">Waiting</span>
                {:else if d.state === "failed"}
                  <Button variant="ghost" size="sm" onclick={() => act(api.retryDownload(d.id))}><RotateCw />Retry</Button>
                {:else if d.state === "done"}
                  <Button variant="ghost" size="icon" title="Play" onclick={() => playIds([d.id])}><Play /></Button>
                {/if}
                <Button variant="ghost" size="icon" title={d.state === "done" ? "Delete download" : "Remove"} onclick={() => remove(d)}>
                  <Trash />
                </Button>
              </Item.Actions>
            </Item.Root>
          {/each}
        </Card.Content>
      </Card.Root>
    {/each}
  {/if}
</div>
