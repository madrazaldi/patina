<script lang="ts">
  import { Activity, ChevronLeft, ChevronRight, History, MonitorPlay, Trash } from "@lucide/svelte";
  import { Badge } from "$lib/components/ui/badge";
  import { Button } from "$lib/components/ui/button";
  import * as Card from "$lib/components/ui/card";
  import * as Empty from "$lib/components/ui/empty";
  import { Progress } from "$lib/components/ui/progress";
  import Img from "../components/Img.svelte";
  import Status from "../components/Status.svelte";
  import { api } from "../lib/api";
  import { ago, bitrate, clock, initials } from "../lib/format";
  import { img } from "../lib/images";
  import * as plex from "../lib/plex";
  import { resource } from "../lib/resource.svelte";
  import { nav, routeFor } from "../lib/router.svelte";
  import { act, app } from "../lib/state.svelte";
  import type { TranscodeSession } from "../lib/types";

  let tab = $state<"playing" | "history" | "tasks">("playing");
  let page = $state(0);

  /** A counter that ticks every `secs` seconds while the tab shows. Sessions
   *  refetch on server events; the slow tick only keeps their progress moving.
   *  Transcodes have no events, so they poll. */
  let tick = $state(0);
  $effect(() => {
    const secs = tab === "playing" ? 15 : tab === "tasks" ? 5 : 0;
    if (!secs) return;
    const t = setInterval(() => tick++, secs * 1000);
    return () => clearInterval(t);
  });

  const sessions = resource(() => {
    tick;
    app.sessionsTick;
    return tab === "playing" ? api.sessions() : Promise.resolve(null);
  });
  const history = resource(() => {
    app.revision;
    const p = page;
    return tab === "history" ? api.history(p) : Promise.resolve(null);
  });
  const transcodes = resource(() => {
    tick;
    return tab === "tasks" ? api.tasks().then((t) => t.transcodes) : Promise.resolve(null);
  });

  function decision(t?: TranscodeSession | null): [string, boolean] {
    if (!t) return ["Direct play", false];
    if (t.videoDecision === "transcode") {
      const hw = t.transcodeHwRequested ? " (HW)" : "";
      return [`Transcode ${(t.sourceVideoCodec ?? "").toUpperCase()} → ${(t.videoCodec ?? "").toUpperCase()}${hw}`, true];
    }
    if (t.audioDecision === "transcode") return ["Audio transcode", true];
    return ["Direct stream", false];
  }

  const up = (s?: string | null) => (s ?? "").toUpperCase();
</script>
{#if transcodes.error && transcodes.data}<Status error={transcodes.error} retry={() => transcodes.reload()} stale />{/if}
{#if history.error && history.data}<Status error={history.error} retry={() => history.reload()} stale />{/if}
{#if sessions.error && sessions.data}<Status error={sessions.error} retry={() => sessions.reload()} stale />{/if}

<div class="page">
  <div class="page-head">
    <div>
      <h1>Activity</h1>
      <div class="sub">What's playing on the server, watch history, and background work.</div>
    </div>
    <div class="tabs">
      <button class={["tab", { active: tab === "playing" }]} onclick={() => (tab = "playing")}>Now playing</button>
      <button class={["tab", { active: tab === "history" }]} onclick={() => (tab = "history")}>History</button>
      <button class={["tab", { active: tab === "tasks" }]} onclick={() => (tab = "tasks")}>Server tasks</button>
    </div>
  </div>

  {#if tab === "playing"}
    {@const list = sessions.data}
    {#if !list}
      <Status loading={!sessions.error} error={sessions.error} retry={() => sessions.reload()} />
    {:else if list.length === 0}
      <Empty.Root class="border">
        <Empty.Header>
          <Empty.Media variant="icon"><MonitorPlay /></Empty.Media>
          <Empty.Title>Nothing is playing</Empty.Title>
          <Empty.Description>Streams from every Plex app on this server show up here live.</Empty.Description>
        </Empty.Header>
      </Empty.Root>
    {:else}
      <div class="mb-6 grid grid-cols-1 gap-4 sm:grid-cols-3">
        {#each [[list.length, "Active streams"], [list.filter((m) => m.TranscodeSession?.videoDecision === "transcode").length, "Transcoding"], [bitrate(list.reduce((n, m) => n + (m.Session?.bandwidth ?? 0), 0)), "Total bandwidth"]] as [value, label] (label)}
          <Card.Root size="sm">
            <Card.Header>
              <Card.Description>{label}</Card.Description>
              <Card.Title class="text-2xl font-semibold tracking-tight tabular-nums group-data-[size=sm]/card:text-2xl">{value}</Card.Title>
            </Card.Header>
          </Card.Root>
        {/each}
      </div>
      <div class="grid grid-cols-1 gap-4 xl:grid-cols-2">
        {#each list as m, i (m.sessionKey ?? i)}
          {@const [title, sub] = plex.playingLines(m)}
          {@const [label, transcoding] = decision(m.TranscodeSession)}
          {@const user = m.User?.title ?? "Unknown"}
          {@const sid = m.Session?.id}
          <Card.Root>
            <Card.Content class="flex gap-4">
              <button
                class="bg-muted focus-visible:ring-ring/50 aspect-[2/3] w-20 flex-none cursor-pointer self-start overflow-hidden rounded-lg border-0 p-0 outline-none focus-visible:ring-3 [&_img]:size-full [&_img]:object-cover"
                onclick={() => nav.push(routeFor(m))}
              >
                <Img src={img(plex.poster(m), 200, 300)} />
              </button>
              <div class="grow">
                <div class="row-flex flex-wrap text-sm">
                  <span class="text-tint text-[10.5px] font-semibold tracking-widest uppercase">{m.Player?.state ?? ""}</span>
                  <span class="text-muted-foreground">·</span>
                  <span class="avatar size-5 text-[8px]">{#if m.User?.thumb}<Img src={img(m.User.thumb, 48, 48)} />{:else}{initials(user)}{/if}</span>
                  <span>{user}</span>
                  <span class="text-muted-foreground">on {m.Player?.title ?? ""} ({m.Player?.product ?? ""})</span>
                </div>
                <h3 class="mt-2 text-lg leading-tight font-semibold tracking-tight">{title}</h3>
                <div class="text-muted-foreground text-sm">{sub}</div>
                <div class="my-3 flex flex-wrap gap-1.5">
                  <Badge variant="outline" class={transcoding ? "border-tint/40 bg-tint/10 text-tint" : ""}>{label}</Badge>
                  {#if m.Session?.location}<Badge variant="outline">{up(m.Session.location)}</Badge>{/if}
                  {#if m.Session?.bandwidth}<Badge variant="outline">{bitrate(m.Session.bandwidth)}</Badge>{/if}
                  {#if m.Player?.address}<Badge variant="outline">{m.Player.address}</Badge>{/if}
                </div>
                <Progress value={(plex.progress(m) ?? 0) * 100} class="[&_[data-slot=progress-indicator]]:bg-tint" />
                <div class="mt-2 flex items-center justify-between text-xs tabular-nums">
                  <span class="text-muted-foreground">{clock(m.viewOffset ?? 0)} / {clock(m.duration ?? 0)}</span>
                  {#if app.owned && sid}
                    <Button variant="destructive" size="xs" onclick={() => act(api.terminateSession(sid), "Stream stopped")}>Stop stream</Button>
                  {/if}
                </div>
              </div>
            </Card.Content>
          </Card.Root>
        {/each}
      </div>
    {/if}
  {:else if tab === "history"}
    {@const h = history.data}
    {#if !h}
      <Status loading={!history.error} error={history.error} retry={() => history.reload()} />
    {:else if h.items.length === 0 && page === 0}
      <Empty.Root class="border">
        <Empty.Header>
          <Empty.Media variant="icon"><History /></Empty.Media>
          <Empty.Title>No history yet</Empty.Title>
        </Empty.Header>
      </Empty.Root>
    {:else}
      <div class="rows">
        {#each h.items as m, i (m.historyKey ?? i)}
          {@const who = m.accountID != null ? h.accounts[String(m.accountID)] : undefined}
          {@const key = m.historyKey}
          <div class="list-row">
            <div class="thumb"><Img src={img(plex.poster(m), 96, 144)} /></div>
            <div class="grow">
              <div class="t">{plex.fullTitle(m)}</div>
              <div class="s">{who ? `${who} · ` : ""}{m.viewedAt ? ago(m.viewedAt) : ""}</div>
            </div>
            <div class="end">
              {#if app.owned && key}
                <Button variant="ghost" size="icon-sm" title="Delete from history" onclick={() => act(api.deleteHistory(key), undefined, true)}>
                  <Trash />
                </Button>
              {/if}
            </div>
          </div>
        {/each}
      </div>
      <div class="mt-5 flex items-center justify-center gap-3.5">
        <Button variant="ghost" size="sm" disabled={page === 0} onclick={() => page--}><ChevronLeft />Newer</Button>
        <span class="text-muted-foreground text-sm tabular-nums">Page {page + 1}</span>
        <Button variant="ghost" size="sm" disabled={!h.hasMore} onclick={() => page++}>Older<ChevronRight /></Button>
      </div>
    {/if}
  {:else}
    <div class="flex flex-col gap-6">
      <Card.Root class="gap-0 py-0">
        <Card.Header class="border-b py-4">
          <Card.Title><h2>Background activity</h2></Card.Title>
        </Card.Header>
        {#if app.activities.length === 0}
          <Empty.Root class="border-0">
            <Empty.Header>
              <Empty.Media variant="icon"><Activity /></Empty.Media>
              <Empty.Description>The server is idle.</Empty.Description>
            </Empty.Header>
          </Empty.Root>
        {/if}
        <div class="divide-y">
          {#each app.activities as a (a.uuid)}
            {@const uuid = a.uuid}
            <div class="setting px-4">
              <div class="label grow">
                <b>{a.title}</b>
                <span>{a.subtitle ?? ""}</span>
                <Progress value={a.progress} class="mt-2.5 max-w-120 [&_[data-slot=progress-indicator]]:bg-tint" />
              </div>
              <div class="control">
                <span class="text-muted-foreground text-sm tabular-nums">{a.progress}%</span>
                {#if a.cancellable}
                  <Button variant="outline" size="sm" onclick={() => act(api.cancelActivity(uuid))}>Cancel</Button>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      </Card.Root>

      <Card.Root class="gap-0 py-0">
        <Card.Header class="border-b py-4">
          <Card.Title><h2>Transcoder sessions</h2></Card.Title>
        </Card.Header>
        {@const list = transcodes.data}
        {#if !list}
          <Status loading={!transcodes.error} error={transcodes.error} retry={() => transcodes.reload()} />
        {:else}
          {#if list.length === 0}
            <Empty.Root class="border-0">
              <Empty.Header>
                <Empty.Media variant="icon"><MonitorPlay /></Empty.Media>
                <Empty.Description>No transcodes running.</Empty.Description>
              </Empty.Header>
            </Empty.Root>
          {/if}
          <div class="divide-y">
            {#each list as tr, i (tr.key ?? i)}
              {@const key = tr.key}
              <div class="setting px-4">
                <div class="label grow">
                  <b>{up(tr.sourceVideoCodec)} → {up(tr.videoCodec)} · {up(tr.audioCodec)}</b>
                  <span>
                    {tr.context ?? ""} · {tr.protocol ?? ""} · speed {(tr.speed ?? 0).toFixed(1)}×{tr.throttled ? " (throttled)" : ""}
                  </span>
                  <Progress value={tr.progress ?? 0} class="mt-2.5 max-w-120 [&_[data-slot=progress-indicator]]:bg-tint" />
                </div>
                {#if key}
                  <div class="control">
                    <Button variant="destructive" size="sm" onclick={() => act(api.killTranscode(key))}>Stop</Button>
                  </div>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </Card.Root>
    </div>
  {/if}
</div>
