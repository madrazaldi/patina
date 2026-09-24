<script lang="ts">
  // Remote for the running mpv: what's playing, transport controls, position.
  import { Pause, Play, RotateCcw, SkipBack, SkipForward, Square } from "@lucide/svelte";
  import Img from "./Img.svelte";
  import { api, type Control } from "../lib/api";
  import { clock } from "../lib/format";
  import { img } from "../lib/images";
  import * as plex from "../lib/plex";
  import { nav, routeFor } from "../lib/router.svelte";
  import { app } from "../lib/state.svelte";

  let np = $derived(app.nowPlaying);
  const send = (c: Control) => () => api.control(c).catch((e) => app.error(e));
</script>

{#if np}
  {@const item = np.item}
  {@const [title, sub] = plex.playingLines(item)}
  {@const pct = np.durationMs > 0 ? (np.positionMs / np.durationMs) * 100 : 0}
  <footer class="now-playing">
    <div class="line"><i style:width="{pct}%"></i></div>
    <button class="info" onclick={() => nav.push(routeFor(item))}>
      <div class="art">
        <Img src={img(plex.kind(item) === "episode" ? item.thumb : plex.poster(item), 120, 120)} />
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
      <span class="tag">{np.transcoding ? "Transcode" : "Direct play"}</span>
      {#if np.queueLen > 1}<span>{np.queuePos + 1} of {np.queueLen}</span>{/if}
      <span>{clock(np.positionMs)} / {clock(np.durationMs)}</span>
    </div>
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
  .line {
    position: absolute;
    top: -1px;
    left: 0;
    right: 0;
    height: 1px;
  }
  .line i {
    display: block;
    height: 100%;
    background: var(--accent);
    transition: width 0.9s linear;
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
    color: var(--text-3);
    font-size: 12px;
  }
</style>
