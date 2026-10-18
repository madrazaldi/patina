<script lang="ts">
  // Remote for the running mpv: what's playing, transport controls, position, and the queue/lyrics panel.
  import { ListMusic, MicVocal, Pause, Play, RotateCcw, SkipBack, SkipForward, SlidersHorizontal, Square } from "@lucide/svelte";
  import { Badge } from "$lib/components/ui/badge";
  import { Button } from "$lib/components/ui/button";
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

  // Floating surfaces anchored to the bar's top-right, styled like a popover.
  const surface = "bg-popover text-popover-foreground absolute right-3 bottom-[calc(100%+8px)] z-40 rounded-xl shadow-lg ring-1 ring-foreground/10";

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
  <footer class="now-playing bg-background relative grid h-[60px] grid-cols-[minmax(0,1fr)_auto_minmax(0,1.5fr)] items-center gap-6 border-t px-4" bind:this={footer}>
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

    <button class="flex min-w-0 cursor-pointer items-center gap-3 border-0 bg-transparent p-0 text-left" onclick={() => app.session && nav.push(routeFor(item))}>
      <div class="bg-muted size-10 flex-none overflow-hidden rounded-md ring-1 ring-foreground/10 [&_img]:size-full [&_img]:object-cover">
        <Img src={art} />
      </div>
      <div class="grow">
        <div class="ellipsis text-sm font-medium">{title}</div>
        <div class="ellipsis text-muted-foreground text-xs">{sub}</div>
      </div>
    </button>
    <div class="flex items-center gap-1">
      <Button variant="ghost" size="icon" class="text-muted-foreground" title="Previous" disabled={np.queuePos === 0} onclick={send({ kind: "previous" })}><SkipBack /></Button>
      <Button variant="ghost" size="icon" class="text-muted-foreground" title="Back 10 seconds" onclick={send({ kind: "seekRelative", seconds: -10 })}><RotateCcw /></Button>
      <Button size="icon-lg" class="mx-1 rounded-full" title={np.paused ? "Play" : "Pause"} onclick={send({ kind: "togglePause" })}>
        {#if np.paused}<Play fill="currentColor" />{:else}<Pause fill="currentColor" />{/if}
      </Button>
      <Button variant="ghost" size="icon" class="text-muted-foreground" title="Stop" onclick={send({ kind: "stop" })}><Square /></Button>
      <Button variant="ghost" size="icon" class="text-muted-foreground" title="Next" disabled={np.queuePos + 1 >= np.queueLen} onclick={send({ kind: "next" })}><SkipForward /></Button>
    </div>
    <!-- When the chips don't fit, the oldest ones fade out under the left edge. -->
    <div class="num text-muted-foreground flex min-w-0 items-center justify-end gap-2.5 overflow-hidden text-xs whitespace-nowrap [mask-image:linear-gradient(to_right,transparent,black_28px)] *:shrink-0">
      {#if np.stopAfter}
        <span class="flex items-center gap-2">
          <Badge variant="outline">Stops after this</Badge>
          <button class="link text-xs" onclick={edit({ kind: "stopAfter", on: false })}>Undo</button>
        </span>
      {/if}
      {#each chips as chip (chip)}<Badge variant="outline">{chip}</Badge>{/each}
      <Badge variant="outline" class={np.live ? "border-tint/40 bg-tint/10 text-tint" : ""}>{status}</Badge>
      {#if np.queueLen > 1}<span>{np.queuePos + 1} of {np.queueLen}</span>{/if}
      <span>{clock(position)}{#if !np.live}&nbsp;/ {clock(np.durationMs)}{/if}</span>
      {#if queue || isTrack || playback}
        <span class="-mr-1.5 ml-1 flex gap-0.5">
          {#if playback}
            <Button variant="ghost" size="icon" class={["text-muted-foreground", { on: panel === "player" }, panel === "player" && "text-tint"]} title="Player controls and diagnostics" onclick={toggle("player")}><SlidersHorizontal /></Button>
          {/if}
          {#if isTrack}
            <Button variant="ghost" size="icon" class={["text-muted-foreground", { on: panel === "lyrics" }, panel === "lyrics" && "text-tint"]} title="Lyrics" onclick={toggle("lyrics")}><MicVocal /></Button>
          {/if}
          {#if queue}
            <Button variant="ghost" size="icon" class={["text-muted-foreground", { on: panel === "queue" }, panel === "queue" && "text-tint"]} title="Queue" onclick={toggle("queue")}><ListMusic /></Button>
          {/if}
        </span>
      {/if}
    </div>

    {#if panel}
      <div class={[surface, "fade-in flex max-h-[60vh] w-[380px] flex-col overflow-hidden", panel === "lyrics" && "h-[60vh]"]}>
        {#if panel === "queue" && queue}<Queue {queue} />{:else if panel === "lyrics"}<Lyrics {np} />{:else if panel === "player" && playback}<PlayerPanel ps={playback} />{/if}
      </div>
    {:else if np.ending && next && !np.stopAfter}
      {@const [nextTitle, nextSub] = plex.playingLines(next)}
      <div class={[surface, "fade-in flex w-[440px] items-center gap-3 py-2.5 pr-2.5 pl-3"]}>
        <div class="bg-muted size-8 flex-none overflow-hidden rounded-sm [&_img]:size-full [&_img]:object-cover"><Img src={img(plex.poster(next) ?? next.thumb, 96, 96)} /></div>
        <div class="grow">
          <div class="text-muted-foreground text-[10.5px] font-semibold tracking-wider uppercase">Up next</div>
          <div class="ellipsis text-sm">{nextTitle}{#if nextSub}<span class="text-muted-foreground"> · {nextSub}</span>{/if}</div>
        </div>
        <Button variant="outline" size="sm" onclick={send({ kind: "next" })}>Play now</Button>
        <Button variant="ghost" size="sm" onclick={edit({ kind: "stopAfter", on: true })}>Stop after this</Button>
      </div>
    {/if}
  </footer>
{/if}

<style>
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
    background: var(--tint);
    transition: width 0.9s linear;
  }
  .seek.scrubbing .line i {
    transition: none;
  }
</style>
