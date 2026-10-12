<script lang="ts">
  // Volume, speed, repeat, sleep timer, chapters and playback diagnostics, in the panel over the now-playing bar.
  import { Repeat1, Repeat as RepeatIcon, Volume2, VolumeX } from "@lucide/svelte";
  import { api, type Tune } from "../lib/api";
  import { clock } from "../lib/format";
  import { app } from "../lib/state.svelte";
  import type { PlaybackState } from "../lib/types";

  let { ps }: { ps: PlaybackState } = $props();

  const send = (c: Tune) => api.tune(c).catch((e) => app.error(e));

  const speeds = [0.5, 0.75, 1, 1.25, 1.5, 2];
  const sleeps = [15, 30, 45, 60, 90];
  const repeats = [
    { mode: "off", label: "Off" },
    { mode: "all", label: "All" },
    { mode: "one", label: "One" },
  ] as const;

  // The sleep timer counts down on its own; tick while it runs.
  let now = $state(Date.now());
  $effect(() => {
    if (ps.sleep?.endsAtMs == null) return;
    const t = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(t);
  });
  let left = $derived(ps.sleep?.endsAtMs != null ? Math.max(0, ps.sleep.endsAtMs - now) : null);
  const leftLabel = (ms: number) => {
    const s = Math.ceil(ms / 1000);
    return s >= 60 ? `${Math.ceil(s / 60)} min` : `${s}s`;
  };

  let d = $derived(ps.diagnostics);
  const connection = {
    local: "Local network",
    remote: "Remote",
    relay: "Plex relay",
    offline: "Downloaded file",
  } as const;

  // Open with the playing chapter in view.
  let chapters: HTMLDivElement | undefined = $state();
  $effect(() => {
    const el = chapters?.querySelector<HTMLElement>(".current");
    if (chapters && el) chapters.scrollTop = el.offsetTop - el.offsetHeight;
  });
</script>

<div class="body">
  <section>
    <div class="row">
      <button class={["icon-btn", { on: ps.muted }]} title={ps.muted ? "Unmute" : "Mute"} onclick={() => send({ kind: "mute" })}>
        {#if ps.muted}<VolumeX size={16} />{:else}<Volume2 size={16} />{/if}
      </button>
      <input
        class="slider"
        type="range"
        min="0"
        max="130"
        step="1"
        aria-label="Volume"
        value={ps.volume}
        oninput={(e) => send({ kind: "volume", value: Number(e.currentTarget.value) })}
      />
      <span class="num value">{Math.round(ps.volume)}%</span>
    </div>
    <div class="row">
      <span class="label">Speed</span>
      <div class="chips">
        {#each speeds as s (s)}
          <button class={["chip", { on: ps.speed === s }]} onclick={() => send({ kind: "speed", value: s })}>{s}x</button>
        {/each}
      </div>
    </div>
    <div class="row">
      <span class="label">Repeat</span>
      <div class="chips">
        {#each repeats as r (r.mode)}
          <button class={["chip", { on: ps.repeat === r.mode }]} onclick={() => send({ kind: "repeat", mode: r.mode })}>
            {#if r.mode === "one"}<Repeat1 size={12} />{:else if r.mode === "all"}<RepeatIcon size={12} />{/if}{r.label}
          </button>
        {/each}
      </div>
    </div>
    <div class="row">
      <span class="label">Sleep</span>
      <div class="chips">
        <button class={["chip", { on: !ps.sleep }]} onclick={() => send({ kind: "sleepOff" })}>Off</button>
        {#each sleeps as m (m)}
          <button class="chip" onclick={() => send({ kind: "sleepMinutes", minutes: m })}>{m}m</button>
        {/each}
        <button class={["chip", { on: ps.sleep?.mode === "endOfItem" }]} onclick={() => send({ kind: "sleepEndOfItem" })}>End of item</button>
      </div>
    </div>
    {#if left != null}
      <div class="faint note">Playback stops in {leftLabel(left)}.</div>
    {:else if ps.sleep?.mode === "endOfItem"}
      <div class="faint note">Playback stops when this item ends.</div>
    {/if}
  </section>

  {#if ps.chapters.length}
    <section>
      <div class="label">Chapters</div>
      <div class="chapters" bind:this={chapters}>
        {#each ps.chapters as c, i (i)}
          <button class={["chapter", { current: ps.chapter === i }]} onclick={() => send({ kind: "chapter", index: i })}>
            <span class="ellipsis grow">{c.title || `Chapter ${i + 1}`}</span>
            <span class="num faint">{clock(c.timeMs)}</span>
          </button>
        {/each}
      </div>
    </section>
  {/if}

  <section>
    <div class="label">Diagnostics</div>
    <dl>
      <dt>Connection</dt>
      <dd>{d.connection ? connection[d.connection.kind] : "Unknown"}{#if d.connection?.address}<span class="faint"> · {d.connection.address}</span>{/if}</dd>
      <dt>Buffer</dt>
      <dd>
        {#if d.buffering}
          Buffering{#if d.bufferPercent != null}, {d.bufferPercent}% full{/if}
        {:else if d.bufferedSecs != null}
          {d.bufferedSecs.toFixed(1)} s ahead
        {:else}
          <span class="faint">n/a</span>
        {/if}
      </dd>
      {#if app.nowPlaying?.item.type !== "track"}
        <dt>Decoder</dt>
        <dd>{d.hwdec == null ? "Unknown" : d.hwdec === "no" ? "Software" : `Hardware (${d.hwdec})`}</dd>
        <dt>Dropped frames</dt>
        <dd>{d.droppedFrames} output · {d.decoderDroppedFrames} decoder</dd>
      {/if}
      {#if d.recoveredFromMs != null}
        <dt>Recovered</dt>
        <dd>Resumed from {clock(d.recoveredFromMs)} after the connection failed</dd>
      {/if}
    </dl>
  </section>
</div>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 14px;
    overflow-y: auto;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .label {
    flex: none;
    width: 48px;
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  section > .label {
    width: auto;
  }
  .slider {
    flex: 1;
    accent-color: var(--accent);
  }
  .value {
    width: 40px;
    text-align: right;
    color: var(--text-2);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 22px;
    padding: 0 8px;
    border: 1px solid var(--line-2);
    border-radius: 11px;
    background: transparent;
    color: var(--text-2);
    font-size: 11.5px;
    cursor: pointer;
  }
  .chip:hover {
    color: var(--text);
  }
  .chip.on {
    color: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 55%, transparent);
  }
  .note {
    font-size: 12px;
  }
  .chapters {
    display: flex;
    flex-direction: column;
    max-height: 140px;
    overflow-y: auto;
  }
  .chapter {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 5px 8px;
    border: 0;
    border-radius: 3px;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  .chapter:hover {
    background: rgba(255, 255, 255, 0.05);
  }
  .chapter.current {
    color: var(--accent);
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 4px 14px;
    margin: 0;
    font-size: 12px;
  }
  dt {
    color: var(--text-3);
  }
  dd {
    margin: 0;
    min-width: 0;
  }
</style>
