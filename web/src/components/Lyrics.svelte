<script lang="ts">
  // Lyrics for the playing track, in the panel over the now-playing bar. Synced lines follow playback.
  import * as Alert from "$lib/components/ui/alert";
  import * as Empty from "$lib/components/ui/empty";
  import { Spinner } from "$lib/components/ui/spinner";
  import { api } from "../lib/api";
  import { app, message } from "../lib/state.svelte";
  import type { Lyrics, NowPlaying } from "../lib/types";

  let { np }: { np: NowPlaying } = $props();

  let key = $derived(np.item.ratingKey ?? "");
  let lyrics = $state<Lyrics | null | undefined>(undefined);
  let error = $state<string>();
  /** The reader's own scrolling holds auto-scroll off until then. */
  let heldUntil = 0;
  /** Whether the current line has been scrolled to since the lyrics loaded. */
  let placed = false;

  $effect(() => {
    const k = key;
    let live = true;
    lyrics = undefined;
    error = undefined;
    placed = false;
    api.lyrics(k).then(
      (l) => live && (lyrics = l),
      (e) => live && (error = message(e)),
    );
    return () => (live = false);
  });

  let synced = $derived(!!lyrics?.synced);

  // Position ticks arrive about once a second; interpolate between them.
  let anchor = $derived({ pos: np.positionMs, at: performance.now(), paused: np.paused });
  let now = $state(performance.now());
  $effect(() => {
    if (!synced) return;
    const t = setInterval(() => (now = performance.now()), 200);
    return () => clearInterval(t);
  });
  let pos = $derived(anchor.paused ? anchor.pos : anchor.pos + Math.min(Math.max(0, now - anchor.at), 1500));

  let active = $derived.by(() => {
    if (!synced || !lyrics) return -1;
    let found = -1;
    lyrics.lines.forEach((l, i) => {
      if (l.timeMs != null && l.timeMs <= pos) found = i;
    });
    return found;
  });

  let box: HTMLDivElement | undefined = $state();
  $effect(() => {
    const i = active;
    if (!box || i < 0 || performance.now() < heldUntil) return;
    const el = box.querySelector<HTMLElement>(`[data-i="${i}"]`);
    if (!el) return;
    box.scrollTo({
      top: el.offsetTop - box.clientHeight / 2 + el.offsetHeight / 2,
      behavior: placed ? "smooth" : "instant",
    });
    placed = true;
  });

  function seek(ms: number | undefined) {
    if (ms == null) return;
    heldUntil = 0;
    api.control({ kind: "seekTo", seconds: ms / 1000 }).catch((e) => app.error(e));
  }
</script>

<header class="flex h-11 min-w-0 items-center gap-1.5 border-b px-4">
  <span class="text-foreground flex-none text-xs font-semibold tracking-wider uppercase">Lyrics</span>
  <span class="text-muted-foreground ellipsis min-w-0 text-xs">· {np.item.title}</span>
</header>

{#if lyrics === undefined && !error}
  <div class="grid place-items-center px-4 py-7"><Spinner /></div>
{:else if error}
  <div class="p-4"><Alert.Root variant="destructive"><Alert.Description class="whitespace-pre-wrap">{error}</Alert.Description></Alert.Root></div>
{:else if !lyrics || lyrics.lines.length === 0}
  <Empty.Root class="border-0 p-8">
    <Empty.Description>No lyrics for this track</Empty.Description>
  </Empty.Root>
{:else}
  <div
    class={["lines", { synced }]}
    bind:this={box}
    onwheel={() => (heldUntil = performance.now() + 4000)}
  >
    {#each lyrics.lines as line, i (i)}
      {#if synced}
        <button
          class={["line", { active: i === active }]}
          data-i={i}
          disabled={line.timeMs == null}
          onclick={() => seek(line.timeMs)}>{line.text || " "}</button
        >
      {:else}
        <p class="line">{line.text || " "}</p>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .lines {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 18px 22px 24px;
    font-size: 16px;
    line-height: 1.55;
    user-select: text;
    -webkit-user-select: text;
  }
  .lines.synced {
    padding-block: 22vh;
    user-select: none;
    -webkit-user-select: none;
  }
  .line {
    display: block;
    width: 100%;
    margin: 0;
    padding: 3px 0;
    border: 0;
    background: none;
    color: var(--text-2);
    font: inherit;
    text-align: left;
  }
  .synced .line {
    color: var(--text-3);
    cursor: pointer;
    transition: color 0.3s var(--ease);
  }
  .synced .line:hover:not(:disabled) {
    color: var(--text-2);
  }
  .synced .line:disabled {
    cursor: default;
  }
  .synced .line.active {
    color: var(--text);
  }
</style>
