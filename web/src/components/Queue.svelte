<script lang="ts">
  // The running play queue, shown in the panel over the now-playing bar. Rows reorder by dragging.
  import { AudioLines, CircleStop, Shuffle, X } from "@lucide/svelte";
  import Img from "./Img.svelte";
  import { api, type QueueCommand } from "../lib/api";
  import { clock, duration, plural } from "../lib/format";
  import { img } from "../lib/images";
  import * as plex from "../lib/plex";
  import { app } from "../lib/state.svelte";
  import type { QueueView } from "../lib/types";

  let { queue }: { queue: QueueView } = $props();

  let list: HTMLDivElement | undefined = $state();
  let current = $derived(queue.items.findIndex((q) => q.uid === queue.current));
  let upcoming = $derived(queue.items.length - current - 1);

  // Pointer drag rather than HTML5 DnD, which the Tauri webview swallows on Windows.
  let drag = $state<{ uid: number; y: number; moved: boolean } | null>(null);
  /** Entry the dragged one would land before; null is the end. */
  let before = $state<number | null | undefined>(undefined);
  let dropped = false;

  const send = (c: QueueCommand) => api.queue(c).catch((e) => app.error(e));

  // Open with the playing entry near the top, one earlier entry above it.
  $effect(() => {
    const el = list?.querySelector<HTMLElement>(".current");
    if (list && el) list.scrollTop = el.offsetTop - el.offsetHeight;
  });

  function down(e: PointerEvent, uid: number) {
    if (e.button !== 0) return;
    drag = { uid, y: e.clientY, moved: false };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function move(e: PointerEvent) {
    if (!drag || !list) return;
    if (!drag.moved && Math.abs(e.clientY - drag.y) < 4) return;
    drag.moved = true;
    const box = list.getBoundingClientRect();
    if (e.clientY < box.top + 24) list.scrollTop -= 8;
    else if (e.clientY > box.bottom - 24) list.scrollTop += 8;
    const rows = [...list.querySelectorAll<HTMLElement>("[data-uid]")];
    const hit = rows.find((r) => {
      const b = r.getBoundingClientRect();
      return e.clientY < b.top + b.height / 2;
    });
    before = hit ? Number(hit.dataset.uid) : null;
  }

  function up() {
    if (drag?.moved && before !== undefined) {
      const from = queue.items.findIndex((q) => q.uid === drag?.uid);
      const to = before === null ? queue.items.length : queue.items.findIndex((q) => q.uid === before);
      if (to !== from && to !== from + 1) send({ kind: "move", uid: drag.uid, before });
      dropped = true;
      setTimeout(() => (dropped = false));
    }
    drag = null;
    before = undefined;
  }

  function jump(uid: number) {
    if (dropped || uid === queue.current) return;
    send({ kind: "jump", uid });
  }
</script>

<header class="head">
  <div class="grow ellipsis">
    <span class="label">Up next</span>
    <span class="faint">· {upcoming > 0 ? plural(upcoming, "item") : "Nothing"}</span>
  </div>
  <button
    class={["icon-btn", { on: queue.shuffled }]}
    title={queue.shuffled ? "Unshuffle" : "Shuffle"}
    onclick={() => send({ kind: "shuffle", on: !queue.shuffled })}><Shuffle size={15} /></button
  >
  <button
    class={["icon-btn", { on: queue.stopAfter }]}
    title={queue.stopAfter ? "Keep playing after this" : "Stop after this"}
    onclick={() => send({ kind: "stopAfter", on: !queue.stopAfter })}><CircleStop size={15} /></button
  >
</header>

<div class="list" bind:this={list}>
  {#each queue.items as { uid, item }, i (uid)}
    {@const [t, s] = plex.playingLines(item)}
    {@const playing = i === current}
    {@const last = i === queue.items.length - 1}
    <div
      class={[
        "row",
        {
          current: playing,
          past: current >= 0 && i < current,
          dragging: drag?.moved && drag.uid === uid,
          "drop-above": drag?.moved && before === uid,
          "drop-below": drag?.moved && last && before === null,
        },
      ]}
      data-uid={uid}
      role="button"
      tabindex="0"
      onpointerdown={(e) => down(e, uid)}
      onpointermove={move}
      onpointerup={up}
      onpointercancel={up}
      onclick={() => jump(uid)}
      onkeydown={(e) => e.key === "Enter" && jump(uid)}
    >
      <div class={["art", { square: plex.shapeFor(item) === "square" }]}>
        <Img src={img(plex.poster(item) ?? item.thumb, 96, 96)} />
      </div>
      <div class="grow">
        <div class="t ellipsis">{t}</div>
        <div class="s ellipsis">{s}</div>
      </div>
      <div class="end num">
        {#if playing}
          <span class="playing"><AudioLines size={14} /></span>
        {:else}
          {#if item.duration}
            <span>{plex.kind(item) === "track" ? clock(item.duration) : duration(item.duration)}</span>
          {/if}
          <button
            class="icon-btn rm"
            title="Remove from queue"
            onpointerdown={(e) => e.stopPropagation()}
            onclick={(e) => {
              e.stopPropagation();
              send({ kind: "remove", uid });
            }}><X size={14} /></button
          >
        {/if}
      </div>
    </div>
  {:else}
    <p class="empty faint">The queue is empty.</p>
  {/each}
</div>

<style>
  .head {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 6px 6px 6px 14px;
    border-bottom: 1px solid var(--line);
  }
  .label {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--text-2);
  }
  .head .faint {
    font-size: 12px;
  }
  .list {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 4px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 48px;
    padding: 0 6px 0 8px;
    border-radius: var(--r);
    touch-action: none;
    cursor: pointer;
  }
  .row:hover {
    background: rgba(255, 255, 255, 0.04);
  }
  .row.current {
    background: rgba(255, 255, 255, 0.06);
    cursor: default;
  }
  .row.past {
    opacity: 0.45;
  }
  .row.dragging {
    opacity: 0.3;
  }
  .row.drop-above {
    box-shadow: inset 0 1px 0 var(--accent);
  }
  .row.drop-below {
    box-shadow: inset 0 -1px 0 var(--accent);
  }
  .art {
    width: 24px;
    height: 36px;
    flex: none;
    border-radius: 2px;
    overflow: hidden;
    background: var(--surface);
  }
  .art.square {
    width: 36px;
  }
  .art :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .t {
    font-size: 12.5px;
  }
  .current .t {
    font-weight: 500;
  }
  .s {
    font-size: 11.5px;
    color: var(--text-3);
  }
  .end {
    display: flex;
    align-items: center;
    gap: 2px;
    flex: none;
    font-size: 11.5px;
    color: var(--text-3);
  }
  .playing {
    display: flex;
    padding-right: 5px;
    color: var(--accent);
  }
  .rm {
    width: 24px;
    height: 24px;
    opacity: 0;
  }
  .row:hover .rm,
  .rm:focus-visible {
    opacity: 1;
  }
  .empty {
    padding: 20px 10px;
    text-align: center;
  }
</style>
