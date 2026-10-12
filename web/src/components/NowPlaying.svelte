<script lang="ts">
  // Remote for the running mpv: what's playing, transport controls, position, and the queue/lyrics panel.
  import { ListMusic, MicVocal, Pause, Play, RotateCcw, SkipBack, SkipForward, SlidersHorizontal, Square } from "@lucide/svelte";
  import Img from "./Img.svelte";
  import Lyrics from "./Lyrics.svelte";
  import PlayerPanel from "./PlayerPanel.svelte";
  import Queue from "./Queue.svelte";
  import { api, type Control, type QueueCommand } from "../lib/api";
  import { clock } from "../lib/format";
  import { downloadImg, img } from "../lib/images";
  import * as plex from "../lib/plex";
  import { nav, routeFor } from "../lib/router.svelte";
  import { app } from "../lib/state.svelte";

  let np = $derived(app.nowPlaying);
  let queue = $derived(app.queue);
  let isTrack = $derived(np != null && plex.kind(np.item) === "track");
  let playback = $derived(app.playback);
  let panel = $derived(
    !np
      ? null
      : app.panel === "queue" && queue
        ? "queue"
        : app.panel === "lyrics" && isTrack
          ? "lyrics"
          : app.panel === "player" && playback
            ? "player"
            : null,
  );

  // Volume, speed, repeat, sleep timer and buffering, when they differ from the defaults.
  let chips = $derived.by(() => {
    if (!np || !playback) return [];
    void np.positionMs; // the sleep countdown refreshes with the position
    const out: string[] = [];
    if (playback.diagnostics.buffering) out.push(playback.diagnostics.bufferPercent != null ? `Buffering ${playback.diagnostics.bufferPercent}%` : "Buffering");
    if (playback.muted) out.push("Muted");
    else if (Math.abs(playback.volume - 100) > 0.5) out.push(`Vol ${Math.round(playback.volume)}%`);
    if (Math.abs(playback.speed - 1) > 0.001) out.push(`${playback.speed}x`);
    if (playback.repeat !== "off") out.push(playback.repeat === "one" ? "Repeat one" : "Repeat all");
    if (playback.sleep?.endsAtMs != null) out.push(`Sleep ${Math.max(1, Math.ceil((playback.sleep.endsAtMs - Date.now()) / 60000))}m`);
    return out;
  });

  let next = $derived.by(() => {
    if (!np || !queue) return undefined;
    const uid = np.uid;
    const i = queue.items.findIndex((q) => q.uid === uid);
    return i >= 0 ? queue.items[i + 1]?.item : undefined;
  });

  // Offline there is no server to ask for artwork; use what the download saved.
  let art = $derived.by(() => {
    if (!np) return undefined;
    const item = np.item;
    if (np.local) {
      const d = app.downloads.find((d) => d.item.ratingKey === item.ratingKey && d.poster);
      return d ? downloadImg(d.id) : undefined;
    }
    return img(plex.kind(item) === "episode" ? item.thumb : plex.poster(item), 120, 120);
  });

  const send = (c: Control) => () => api.control(c).catch((e) => app.error(e));
  const edit = (c: QueueCommand) => () => api.queue(c).catch((e) => app.error(e));
  const toggle = (p: "queue" | "lyrics" | "player") => () => (app.panel = app.panel === p ? null : p);

  let footer: HTMLElement | undefined = $state();

  function outside(e: PointerEvent) {
    if (app.panel && !footer?.contains(e.target as Node)) app.panel = null;
  }

  // Scrubbing: the fraction under the pointer, held after release until playback catches up.
  let seek: HTMLDivElement | undefined = $state();
  let scrub = $state<number | null>(null);
  let scrubbing = false;

  const fraction = (e: PointerEvent) => {
    const r = seek!.getBoundingClientRect();
    return Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
  };

  function release() {
    if (!np || scrub == null || !scrubbing) return;
    scrubbing = false;
    send({ kind: "seekTo", seconds: (scrub * np.durationMs) / 1000 })();
    setTimeout(() => !scrubbing && (scrub = null), 2000);
  }

  // Keyboard: arrows seek 5 s, PageUp/PageDown 60 s, Home/End jump to the ends. Repeated presses build on the
  // pending target rather than on the lagging playback position.
  let scrubTimer: ReturnType<typeof setTimeout> | undefined;
  function seekKey(e: KeyboardEvent) {
    if (!np || np.durationMs <= 0 || e.altKey || e.ctrlKey || e.metaKey) return;
    const now = scrub != null ? scrub * np.durationMs : np.positionMs;
    const step: Record<string, number> = { ArrowLeft: -5000, ArrowDown: -5000, ArrowRight: 5000, ArrowUp: 5000, PageDown: -60000, PageUp: 60000 };
    let to: number;
    if (e.key in step) to = now + step[e.key];
    else if (e.key === "Home") to = 0;
    // Stop just short of the end so End never skips to the next item by itself.
    else if (e.key === "End") to = np.durationMs - 1000;
    else return;
    e.preventDefault();
    to = Math.min(Math.max(0, to), Math.max(0, np.durationMs - 1000));
    scrub = to / np.durationMs;
    send({ kind: "seekTo", seconds: to / 1000 })();
    clearTimeout(scrubTimer);
    scrubTimer = setTimeout(() => !scrubbing && (scrub = null), 2000);
  }

  $effect(() => {
    const pos = app.nowPlaying?.positionMs ?? 0;
    const target = scrub != null && np ? scrub * np.durationMs : null;
    if (!scrubbing && target != null && Math.abs(pos - target) < 2500) scrub = null;
  });
</script>

<svelte:window onpointerdown={outside} />

{#if np}
  {@const item = np.item}
  {@const [title, sub] = plex.playingLines(item)}
  {@const position = scrub != null ? scrub * np.durationMs : np.positionMs}
  {@const pct = np.durationMs > 0 ? (position / np.durationMs) * 100 : 0}
  {@const status = np.live ? "Live" : np.local ? "Offline" : np.transcoding ? "Transcode" : "Direct play"}
  <footer class="now-playing" bind:this={footer}>
    {#if !np.live && np.durationMs > 0}
      <div
        class={["seek", { scrubbing: scrub != null }]}
        bind:this={seek}
        role="slider"
        tabindex="0"
        aria-label="Seek"
        aria-orientation="horizontal"
        aria-valuemin={0}
        aria-valuemax={Math.round(np.durationMs / 1000)}
        aria-valuenow={Math.round(position / 1000)}
        aria-valuetext="{clock(position)} of {clock(np.durationMs)}"
        onkeydown={seekKey}
        onpointerdown={(e) => {
          if (e.button !== 0) return;
          e.currentTarget.setPointerCapture(e.pointerId);
          scrubbing = true;
          scrub = fraction(e);
        }}
        onpointermove={(e) => scrubbing && (scrub = fraction(e))}
        onpointerup={release}
        onpointercancel={() => {
          scrubbing = false;
          scrub = null;
        }}
      >
        <div class="line"><i style:width="{pct}%"></i></div>
      </div>
    {/if}

    <button class="info" onclick={() => app.session && nav.push(routeFor(item))}>
      <div class="art">
        <Img src={art} />
      </div>
      <div class="grow">
        <div class="ellipsis t">{title}</div>
        <div class="ellipsis s">{sub}</div>
      </div>
    </button>
    <div class="controls">
      <button class="icon-btn" title="Previous" disabled={np.queuePos === 0} onclick={send({ kind: "previous" })}><SkipBack /></button>
      <button class="icon-btn" title="Back 10 seconds" onclick={send({ kind: "seekRelative", seconds: -10 })}><RotateCcw /></button>
      <button class="icon-btn toggle-play" title={np.paused ? "Play" : "Pause"} onclick={send({ kind: "togglePause" })}>
        {#if np.paused}<Play size={16} fill="currentColor" />{:else}<Pause size={16} fill="currentColor" />{/if}
      </button>
      <button class="icon-btn" title="Stop" onclick={send({ kind: "stop" })}><Square size={14} /></button>
      <button class="icon-btn" title="Next" disabled={np.queuePos + 1 >= np.queueLen} onclick={send({ kind: "next" })}><SkipForward /></button>
    </div>
    <div class="meta num">
      {#if np.stopAfter}
        <span class="stops">
          <span class="tag">Stops after this</span>
          <button class="link" onclick={edit({ kind: "stopAfter", on: false })}>Undo</button>
        </span>
      {/if}
      {#each chips as chip (chip)}<span class="tag">{chip}</span>{/each}
      <span class={["tag", { accent: np.live }]}>{status}</span>
      {#if np.queueLen > 1}<span>{np.queuePos + 1} of {np.queueLen}</span>{/if}
      <span>{clock(position)}{#if !np.live}&nbsp;/ {clock(np.durationMs)}{/if}</span>
      {#if queue || isTrack || playback}
        <span class="tools">
          {#if playback}
            <button class={["icon-btn", { on: panel === "player" }]} title="Player controls and diagnostics" onclick={toggle("player")}><SlidersHorizontal /></button>
          {/if}
          {#if isTrack}
            <button class={["icon-btn", { on: panel === "lyrics" }]} title="Lyrics" onclick={toggle("lyrics")}><MicVocal /></button>
          {/if}
          {#if queue}
            <button class={["icon-btn", { on: panel === "queue" }]} title="Queue" onclick={toggle("queue")}><ListMusic /></button>
          {/if}
        </span>
      {/if}
    </div>

    {#if panel}
      <div class={["surface panel fade-in", { tall: panel === "lyrics" }]}>
        {#if panel === "queue" && queue}<Queue {queue} />{:else if panel === "lyrics"}<Lyrics {np} />{:else if panel === "player" && playback}<PlayerPanel ps={playback} />{/if}
      </div>
    {:else if np.ending && next && !np.stopAfter}
      {@const [nextTitle, nextSub] = plex.playingLines(next)}
      <div class="surface upnext fade-in">
        <div class="art small"><Img src={img(plex.poster(next) ?? next.thumb, 96, 96)} /></div>
        <div class="grow">
          <div class="kicker">Up next</div>
          <div class="ellipsis">{nextTitle}{#if nextSub}<span class="faint"> · {nextSub}</span>{/if}</div>
        </div>
        <button class="btn sm" onclick={send({ kind: "next" })}>Play now</button>
        <button class="btn sm quiet" onclick={edit({ kind: "stopAfter", on: true })}>Stop after this</button>
      </div>
    {/if}
  </footer>
{/if}

<style>
  .now-playing {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: center;
    gap: 24px;
    height: 60px;
    padding: 0 16px;
    border-top: 1px solid var(--line);
    background: var(--bg);
  }
  /* A few pixels of hit area around the 1px line. */
  .seek {
    position: absolute;
    top: -5px;
    left: 0;
    right: 0;
    z-index: 1;
    height: 9px;
    cursor: pointer;
    touch-action: none;
  }
  .line {
    position: absolute;
    top: 4px;
    left: 0;
    right: 0;
    height: 1px;
  }
  .seek:hover .line,
  .seek:focus-visible .line,
  .seek.scrubbing .line {
    background: var(--line-2);
  }
  .seek:focus-visible .line {
    height: 3px;
    top: 3px;
  }
  .line i {
    display: block;
    height: 100%;
    background: var(--accent);
    transition: width 0.9s linear;
  }
  .seek.scrubbing .line i {
    transition: none;
  }
  .info {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
    padding: 0;
    border: 0;
    background: none;
    text-align: left;
    cursor: pointer;
  }
  .art {
    width: 38px;
    height: 38px;
    flex: none;
    border-radius: 3px;
    overflow: hidden;
    background: var(--surface);
  }
  .art.small {
    width: 32px;
    height: 32px;
    border-radius: 2px;
  }
  .art :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .t {
    font-weight: 500;
  }
  .s {
    font-size: 12px;
    color: var(--text-3);
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .toggle-play {
    width: 34px;
    height: 34px;
    border-radius: 50%;
    color: var(--text);
    border: 1px solid var(--line-2);
  }
  .meta {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 14px;
    min-width: 0;
    color: var(--text-3);
    font-size: 12px;
    white-space: nowrap;
  }
  .stops {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .stops .link {
    font-size: 12px;
  }
  .tools {
    display: flex;
    gap: 2px;
    margin-right: -6px;
  }

  /* Floating surfaces anchored to the bar's top-right. */
  .surface {
    position: absolute;
    right: 12px;
    bottom: calc(100% + 8px);
    z-index: 40;
    border-radius: 6px;
    border: 1px solid var(--line-2);
    background: #161616;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.5);
  }
  .panel {
    display: flex;
    flex-direction: column;
    width: 380px;
    max-height: 60vh;
    overflow: hidden;
  }
  .panel.tall {
    height: 60vh;
  }
  .upnext {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 440px;
    padding: 10px 10px 10px 12px;
  }
  .kicker {
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--text-3);
  }
</style>
