<script lang="ts">
  // Live TV from the server's DVR: what's on now and next per channel, and
  // scheduled recordings with their rules.
  import { ChevronDown, CircleDot, Play, Trash } from "@lucide/svelte";
  import Img from "../components/Img.svelte";
  import Status from "../components/Status.svelte";
  import { api } from "../lib/api";
  import { img } from "../lib/images";
  import { resource } from "../lib/resource.svelte";
  import { act, app } from "../lib/state.svelte";
  import type { LiveChannel, LiveProgram } from "../lib/types";

  let tab = $state<"guide" | "recordings">("guide");

  /** Unix seconds, advanced every minute so finished programs drop off. */
  let now = $state(Date.now() / 1000);
  /** Bumped every 15 minutes to refetch the guide. */
  let refetch = $state(0);
  $effect(() => {
    const a = setInterval(() => (now = Date.now() / 1000), 60_000);
    const b = setInterval(() => refetch++, 15 * 60_000);
    return () => (clearInterval(a), clearInterval(b));
  });

  const guide = resource(() => {
    refetch;
    return tab === "guide" ? api.livetvGuide(6) : Promise.resolve(null);
  });
  const recordings = resource(() => {
    app.revision;
    return tab === "recordings" ? api.recordings() : Promise.resolve(null);
  });

  /** Programs not yet over; the first is on air when it has begun. */
  const upcoming = (ch: LiveChannel) => ch.programs.filter((p) => p.endsAt > now);
  const onAir = (p: LiveProgram | undefined) => p != null && p.beginsAt <= now;
  const elapsed = (p: LiveProgram) => Math.min(100, Math.max(0, ((now - p.beginsAt) / (p.endsAt - p.beginsAt)) * 100));

  const time = (s: number) => new Date(s * 1000).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  const range = (p: LiveProgram) => `${time(p.beginsAt)} – ${time(p.endsAt)}`;
  const when = (s: number) =>
    new Date(s * 1000).toLocaleString([], { weekday: "short", month: "short", day: "numeric", hour: "numeric", minute: "2-digit" });

  /** Channel whose later listings are showing. */
  let open = $state<string | null>(null);
  const toggle = (id: string) => (open = open === id ? null : id);

  function watch(ch: LiveChannel) {
    app.toast("Tuning…");
    api.livetvWatch(ch.dvr, ch.id, ch.title).catch((e) => app.error(e));
  }

  const record = (guid: string) => act(api.record(guid), "Recording scheduled");

  async function deleteRule(id: string, title: string) {
    const ok = await app.confirm("Delete recording rule?", {
      text: `${title} will no longer be recorded. Existing recordings are kept.`,
      action: "Delete",
      danger: true,
    });
    if (ok && (await act(api.deleteRecordingRule(id), "Recording rule deleted"))) recordings.reload();
  }
</script>

{#snippet recordBtn(p: LiveProgram)}
  {@const guid = p.guid}
  {#if guid}
    <button
      class="icon-btn rec"
      title="Record"
      onclick={(e) => {
        e.stopPropagation();
        record(guid);
      }}><CircleDot size={14} /></button
    >
  {/if}
{/snippet}

<div class="page">
  <div class="page-head">
    <div>
      <h1>Live TV</h1>
      <div class="sub">Watch and record from your server's tuner.</div>
    </div>
    <div class="tabs">
      <button class={["tab", { active: tab === "guide" }]} onclick={() => (tab = "guide")}>Guide</button>
      <button class={["tab", { active: tab === "recordings" }]} onclick={() => (tab = "recordings")}>Recordings</button>
    </div>
  </div>

  {#if tab === "guide"}
    {@const channels = guide.data}
    {#if !channels}
      <Status loading={!guide.error} error={guide.error} />
    {:else if channels.length === 0}
      <Status title="No channels" text="Set up a tuner and scan for channels in Plex's Live TV & DVR settings." />
    {:else}
      <div class="rows guide">
        {#each channels as ch (ch.dvr + ch.id)}
          {@const list = upcoming(ch)}
          {@const cur = onAir(list[0]) ? list[0] : undefined}
          {@const next = cur ? list[1] : list[0]}
          {@const later = list.slice(cur ? 2 : 1)}
          {@const expanded = open === ch.dvr + ch.id}
          <div class="channel">
            <div
              class="line"
              role="button"
              tabindex="0"
              onclick={() => toggle(ch.dvr + ch.id)}
              onkeydown={(e) => e.key === "Enter" && toggle(ch.dvr + ch.id)}
            >
              <div class="who">
                <span class="number num faint">{ch.number ?? ""}</span>
                <span class="logo"><Img src={img(ch.thumb, 80, 80)} /></span>
                <span class="ellipsis">{ch.title}</span>
              </div>

              <div class="now">
                {#if cur}
                  <div class="row-flex">
                    <span class="t ellipsis">{cur.title}</span>
                    {@render recordBtn(cur)}
                  </div>
                  <div class="s ellipsis">
                    <span class="num">{range(cur)}</span>{#if cur.subtitle} · {cur.subtitle}{/if}
                  </div>
                  <div class="bar"><i style:width="{elapsed(cur)}%"></i></div>
                {:else}
                  <span class="faint">{list.length ? "Off air" : "No guide data"}</span>
                {/if}
              </div>

              <div class="next">
                {#if next}
                  <div class="row-flex">
                    <span class="label num">{time(next.beginsAt)}</span>
                    <span class="ellipsis">{next.title}</span>
                    {@render recordBtn(next)}
                  </div>
                  {#if next.subtitle}<div class="s ellipsis">{next.subtitle}</div>{/if}
                {/if}
              </div>

              <div class="actions">
                <button
                  class="btn sm"
                  onclick={(e) => {
                    e.stopPropagation();
                    watch(ch);
                  }}><Play size={13} />Watch</button
                >
                <span class={["chev", { expanded }]}><ChevronDown size={14} /></span>
              </div>
            </div>

            {#if expanded}
              <div class="later">
                {#each later as p (p.beginsAt)}
                  <div class="program">
                    <div class="label num">{range(p)}</div>
                    <div class="ellipsis">{p.title}</div>
                    <div class="s ellipsis">{p.subtitle ?? ""}</div>
                    {#if p.guid}
                      {@const guid = p.guid}
                      <button class="link small" onclick={() => record(guid)}>Record</button>
                    {/if}
                  </div>
                {:else}
                  <span class="faint small">No later listings in the next six hours.</span>
                {/each}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  {:else}
    {@const r = recordings.data}
    {#if !r}
      <Status loading={!recordings.error} error={recordings.error} />
    {:else}
      <div class="section-head"><h2>Upcoming</h2></div>
      {#if r.upcoming.length === 0}
        <p class="faint">Nothing scheduled. Record a program from the guide.</p>
      {:else}
        <div class="rows">
          {#each r.upcoming as u (u.id)}
            <div class="list-row upcoming">
              <span class="when num faint">{u.beginsAt ? when(u.beginsAt) : ""}</span>
              <div class="grow">
                <div class="t">{u.title}</div>
                <div class="s">{[u.subtitle, u.channel].filter(Boolean).join(" · ")}</div>
              </div>
              <div class="end">{#if u.status}<span class="tag">{u.status}</span>{/if}</div>
            </div>
          {/each}
        </div>
      {/if}

      <div class="section-head"><h2>Rules</h2></div>
      {#if r.rules.length === 0}
        <p class="faint">No recording rules.</p>
      {:else}
        <div class="rows">
          {#each r.rules as rule (rule.id)}
            <div class="list-row rule">
              <div class="grow">
                <div class="t">{rule.title}</div>
                {#if rule.kind}<div class="s">{rule.kind}</div>{/if}
              </div>
              <div class="end">
                <button class="icon-btn" title="Delete rule" onclick={() => deleteRule(rule.id, rule.title)}>
                  <Trash size={14} />
                </button>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    {/if}
  {/if}

  <p class="faint small note">Live TV needs a tuner connected to a Plex Pass server.</p>
</div>

<style>
  .small {
    font-size: 12px;
  }
  .note {
    margin-top: 28px;
  }
  .s {
    color: var(--text-3);
    font-size: 12px;
  }
  .label {
    flex: none;
    font-size: 11px;
    color: var(--text-3);
  }

  .line {
    display: grid;
    grid-template-columns: 200px minmax(0, 1.4fr) minmax(0, 1fr) auto;
    align-items: center;
    gap: 20px;
    min-height: 64px;
    padding: 10px 4px;
    cursor: pointer;
  }
  .line:hover {
    background: rgba(255, 255, 255, 0.025);
  }
  .who {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    font-weight: 500;
  }
  .number {
    width: 34px;
    flex: none;
    font-size: 12px;
    font-weight: 400;
    text-align: right;
  }
  .logo {
    width: 32px;
    height: 32px;
    flex: none;
    border-radius: 3px;
    overflow: hidden;
    background: var(--surface);
  }
  .logo :global(img) {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  .now,
  .next {
    min-width: 0;
  }
  .now .row-flex,
  .next .row-flex {
    gap: 6px;
    min-height: 22px;
  }
  .now .t {
    font-weight: 500;
  }
  .now .bar {
    max-width: 280px;
    margin-top: 7px;
  }
  .next {
    color: var(--text-2);
  }
  .rec {
    width: 22px;
    height: 22px;
    flex: none;
    opacity: 0;
  }
  .line:hover .rec,
  .rec:focus-visible {
    opacity: 1;
  }
  .rec:hover:not(:disabled) {
    color: var(--danger);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .chev {
    display: inline-flex;
    color: var(--text-3);
    transition: transform 0.15s var(--ease);
  }
  .chev.expanded {
    transform: rotate(180deg);
  }

  /* Later listings of an expanded channel, as a horizontal strip. */
  .later {
    display: flex;
    gap: 1px;
    overflow-x: auto;
    padding: 0 4px 14px 224px;
  }
  .program {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: none;
    width: 190px;
    padding: 8px 12px;
    border-left: 1px solid var(--line);
  }
  .program .link {
    align-self: flex-start;
    margin-top: 4px;
  }

  .upcoming {
    grid-template-columns: 170px 1fr auto;
  }
  .rule {
    grid-template-columns: 1fr auto;
  }
  .list-row .grow {
    min-width: 0;
  }
</style>
