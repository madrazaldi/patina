<script lang="ts">
  // Volume, speed, repeat, sleep timer, chapters and playback diagnostics, in the panel over the now-playing bar.
  import { Repeat1, Repeat as RepeatIcon, Volume2, VolumeX } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button";
  import { Separator } from "$lib/components/ui/separator";
  import { Slider } from "$lib/components/ui/slider";
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

  const label = "text-muted-foreground w-12 flex-none text-[10.5px] font-semibold tracking-wider uppercase";
  const heading = "text-muted-foreground text-[10.5px] font-semibold tracking-wider uppercase";
  const chip = "text-muted-foreground hover:text-foreground rounded-full px-2.5";
  const on = "border-tint/55 text-tint hover:text-tint";

  // Open with the playing chapter in view.
  let chapters: HTMLDivElement | undefined = $state();
  $effect(() => {
    const el = chapters?.querySelector<HTMLElement>(".current");
    if (chapters && el) chapters.scrollTop = el.offsetTop - el.offsetHeight;
  });
</script>

<div class="flex flex-col gap-4 overflow-y-auto p-4">
  <section class="flex flex-col gap-3">
    <div class="flex items-center gap-2.5">
      <Button
        variant="ghost"
        size="icon-sm"
        class={["text-muted-foreground", { on: ps.muted }, ps.muted && "text-tint"]}
        title={ps.muted ? "Unmute" : "Mute"}
        onclick={() => send({ kind: "mute" })}
      >
        {#if ps.muted}<VolumeX />{:else}<Volume2 />{/if}
      </Button>
      <div class="flex flex-1" role="group" aria-label="Volume">
        <Slider type="single" min={0} max={130} step={1} value={ps.volume} onValueChange={(v) => send({ kind: "volume", value: v })} />
      </div>
      <span class="num text-muted-foreground w-10 text-right text-sm">{Math.round(ps.volume)}%</span>
    </div>
    <div class="flex items-center gap-2.5">
      <span class={label}>Speed</span>
      <div class="flex flex-wrap gap-1">
        {#each speeds as s (s)}
          <Button variant="outline" size="xs" class={[chip, ps.speed === s && on]} onclick={() => send({ kind: "speed", value: s })}>{s}x</Button>
        {/each}
      </div>
    </div>
    <div class="flex items-center gap-2.5">
      <span class={label}>Repeat</span>
      <div class="flex flex-wrap gap-1">
        {#each repeats as r (r.mode)}
          <Button variant="outline" size="xs" class={[chip, ps.repeat === r.mode && on]} onclick={() => send({ kind: "repeat", mode: r.mode })}>
            {#if r.mode === "one"}<Repeat1 />{:else if r.mode === "all"}<RepeatIcon />{/if}{r.label}
          </Button>
        {/each}
      </div>
    </div>
    <div class="flex items-start gap-2.5">
      <span class={[label, "pt-1"]}>Sleep</span>
      <div class="flex flex-wrap gap-1">
        <Button variant="outline" size="xs" class={[chip, !ps.sleep && on]} onclick={() => send({ kind: "sleepOff" })}>Off</Button>
        {#each sleeps as m (m)}
          <Button variant="outline" size="xs" class={chip} onclick={() => send({ kind: "sleepMinutes", minutes: m })}>{m}m</Button>
        {/each}
        <Button variant="outline" size="xs" class={[chip, ps.sleep?.mode === "endOfItem" && on]} onclick={() => send({ kind: "sleepEndOfItem" })}>End of item</Button>
      </div>
    </div>
    {#if left != null}
      <div class="text-muted-foreground text-xs">Playback stops in {leftLabel(left)}.</div>
    {:else if ps.sleep?.mode === "endOfItem"}
      <div class="text-muted-foreground text-xs">Playback stops when this item ends.</div>
    {/if}
  </section>

  {#if ps.chapters.length}
    <Separator />
    <section class="flex flex-col gap-2">
      <div class={heading}>Chapters</div>
      <div class="-mx-1 flex max-h-36 flex-col overflow-y-auto" bind:this={chapters}>
        {#each ps.chapters as c, i (i)}
          <Button
            variant="ghost"
            size="sm"
            class={["chapter h-auto justify-start gap-2.5 px-2 py-1.5 font-normal", ps.chapter === i && "current text-tint"]}
            onclick={() => send({ kind: "chapter", index: i })}
          >
            <span class="ellipsis grow text-left">{c.title || `Chapter ${i + 1}`}</span>
            <span class="num text-muted-foreground">{clock(c.timeMs)}</span>
          </Button>
        {/each}
      </div>
    </section>
  {/if}

  <Separator />
  <section class="flex flex-col gap-2">
    <div class={heading}>Diagnostics</div>
    <dl class="m-0 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-xs">
      <dt class="text-muted-foreground">Connection</dt>
      <dd class="m-0 min-w-0">{d.connection ? connection[d.connection.kind] : "Unknown"}{#if d.connection?.address}<span class="text-muted-foreground"> · {d.connection.address}</span>{/if}</dd>
      <dt class="text-muted-foreground">Buffer</dt>
      <dd class="m-0 min-w-0">
        {#if d.buffering}
          Buffering{#if d.bufferPercent != null}, {d.bufferPercent}% full{/if}
        {:else if d.bufferedSecs != null}
          {d.bufferedSecs.toFixed(1)} s ahead
        {:else}
          <span class="text-muted-foreground">n/a</span>
        {/if}
      </dd>
      {#if app.nowPlaying?.item.type !== "track"}
        <dt class="text-muted-foreground">Decoder</dt>
        <dd class="m-0 min-w-0">{d.hwdec == null ? "Unknown" : d.hwdec === "no" ? "Software" : `Hardware (${d.hwdec})`}</dd>
        <dt class="text-muted-foreground">Dropped frames</dt>
        <dd class="m-0 min-w-0">{d.droppedFrames} output · {d.decoderDroppedFrames} decoder</dd>
      {/if}
      {#if d.recoveredFromMs != null}
        <dt class="text-muted-foreground">Recovered</dt>
        <dd class="m-0 min-w-0">Resumed from {clock(d.recoveredFromMs)} after the connection failed</dd>
      {/if}
    </dl>
  </section>
</div>
