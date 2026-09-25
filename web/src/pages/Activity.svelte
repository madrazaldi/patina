<script lang="ts">
  import { ChevronLeft, ChevronRight, Trash } from "@lucide/svelte";
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
      <Status title="Nothing is playing" text="Streams from every Plex app on this server show up here live." />
    {:else}
      <div class="stats">
        <div><b>{list.length}</b><span>Active streams</span></div>
        <div><b>{list.filter((m) => m.TranscodeSession?.videoDecision === "transcode").length}</b><span>Transcoding</span></div>
        <div><b>{bitrate(list.reduce((n, m) => n + (m.Session?.bandwidth ?? 0), 0))}</b><span>Total bandwidth</span></div>
      </div>
      <div class="rows sessions">
        {#each list as m, i (m.sessionKey ?? i)}
          {@const [title, sub] = plex.playingLines(m)}
          {@const [label, transcoding] = decision(m.TranscodeSession)}
          {@const user = m.User?.title ?? "Unknown"}
          {@const sid = m.Session?.id}
          <div class="session">
            <button class="poster" onclick={() => nav.push(routeFor(m))}><Img src={img(plex.poster(m), 200, 300)} /></button>
            <div class="grow">
              <div class="row-flex">
                <span class="state">{m.Player?.state ?? ""}</span>
                <span class="faint">·</span>
                <span class="avatar small">{#if m.User?.thumb}<Img src={img(m.User.thumb, 48, 48)} />{:else}{initials(user)}{/if}</span>
                <span>{user}</span>
                <span class="faint">on {m.Player?.title ?? ""} ({m.Player?.product ?? ""})</span>
              </div>
              <h3>{title}</h3>
              <div class="faint">{sub}</div>
              <div class="tags">
                <span class={["tag", { accent: transcoding }]}>{label}</span>
                {#if m.Session?.location}<span class="tag">{up(m.Session.location)}</span>{/if}
                {#if m.Session?.bandwidth}<span class="tag">{bitrate(m.Session.bandwidth)}</span>{/if}
                {#if m.Player?.address}<span class="tag">{m.Player.address}</span>{/if}
              </div>
              <div class="bar"><i style:width="{(plex.progress(m) ?? 0) * 100}%"></i></div>
              <div class="foot num">
                <span class="faint">{clock(m.viewOffset ?? 0)} / {clock(m.duration ?? 0)}</span>
                {#if app.owned && sid}
                  <button class="link" onclick={() => act(api.terminateSession(sid), "Stream stopped")}>Stop stream</button>
                {/if}
              </div>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  {:else if tab === "history"}
    {@const h = history.data}
    {#if !h}
      <Status loading={!history.error} error={history.error} retry={() => history.reload()} />
    {:else if h.items.length === 0 && page === 0}
      <Status title="No history yet" />
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
                <button class="icon-btn" title="Delete from history" onclick={() => act(api.deleteHistory(key), undefined, true)}>
                  <Trash size={14} />
                </button>
              {/if}
            </div>
          </div>
        {/each}
      </div>
      <div class="pager">
        <button class="btn sm quiet" disabled={page === 0} onclick={() => page--}><ChevronLeft size={14} />Newer</button>
        <span class="faint num">Page {page + 1}</span>
        <button class="btn sm quiet" disabled={!h.hasMore} onclick={() => page++}>Older<ChevronRight size={14} /></button>
      </div>
    {/if}
  {:else}
    <div class="section-head"><h2>Background activity</h2></div>
    {#if app.activities.length === 0}<p class="faint">The server is idle.</p>{/if}
    <div class="rows">
      {#each app.activities as a (a.uuid)}
        {@const uuid = a.uuid}
        <div class="setting">
          <div class="label grow">
            <b>{a.title}</b>
            <span>{a.subtitle ?? ""}</span>
            <div class="bar task"><i style:width="{a.progress}%"></i></div>
          </div>
          <div class="control">
            <span class="faint num">{a.progress}%</span>
            {#if a.cancellable}
              <button class="btn sm" onclick={() => act(api.cancelActivity(uuid))}>Cancel</button>
            {/if}
          </div>
        </div>
      {/each}
    </div>

    <div class="section-head"><h2>Transcoder sessions</h2></div>
    {@const list = transcodes.data}
    {#if !list}
      <Status loading={!transcodes.error} error={transcodes.error} retry={() => transcodes.reload()} />
    {:else}
      {#if list.length === 0}<p class="faint">No transcodes running.</p>{/if}
      <div class="rows">
        {#each list as tr, i (tr.key ?? i)}
          {@const key = tr.key}
          <div class="setting">
            <div class="label grow">
              <b>{up(tr.sourceVideoCodec)} → {up(tr.videoCodec)} · {up(tr.audioCodec)}</b>
              <span>
                {tr.context ?? ""} · {tr.protocol ?? ""} · speed {(tr.speed ?? 0).toFixed(1)}×{tr.throttled ? " (throttled)" : ""}
              </span>
              <div class="bar task"><i style:width="{tr.progress ?? 0}%"></i></div>
            </div>
            {#if key}
              <div class="control">
                <button class="btn sm danger" onclick={() => act(api.killTranscode(key))}>Stop</button>
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  {/if}
</div>

<style>
  .stats {
    margin-bottom: 24px;
  }
  .session {
    display: flex;
    gap: 20px;
    padding: 18px 0;
  }
  .poster {
    flex: none;
    width: 80px;
    aspect-ratio: 2 / 3;
    padding: 0;
    border: 0;
    border-radius: var(--r);
    overflow: hidden;
    background: var(--surface);
    cursor: pointer;
  }
  .poster :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .state {
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--accent);
  }
  .avatar.small {
    width: 18px;
    height: 18px;
    font-size: 8px;
  }
  h3 {
    margin-top: 8px;
    font: 400 20px/1.2 var(--serif);
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 12px 0;
  }
  .foot {
    display: flex;
    justify-content: space-between;
    margin-top: 8px;
    font-size: 12px;
  }
  .bar.task {
    max-width: 480px;
    margin-top: 10px;
  }
  .pager {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 14px;
    margin-top: 20px;
  }
</style>
