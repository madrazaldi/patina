<script lang="ts">
  // The running play queue, shown in the panel over the now-playing bar. Rows reorder by dragging.
  import { AudioLines, CircleStop, Shuffle, X } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button";
  import * as Empty from "$lib/components/ui/empty";
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

<header class="flex items-center gap-0.5 border-b py-1.5 pr-1.5 pl-4">
  <div class="grow ellipsis">
    <span class="text-foreground text-xs font-semibold tracking-wider uppercase">Up next</span>
    <span class="text-muted-foreground text-xs">· {upcoming > 0 ? plural(upcoming, "item") : "Nothing"}</span>
  </div>
  <Button
    variant="ghost"
    size="icon-sm"
    class={["text-muted-foreground", { on: queue.shuffled }, queue.shuffled && "text-tint"]}
    title={queue.shuffled ? "Unshuffle" : "Shuffle"}
    onclick={() => send({ kind: "shuffle", on: !queue.shuffled })}><Shuffle /></Button
  >
  <Button
    variant="ghost"
    size="icon-sm"
    class={["text-muted-foreground", { on: queue.stopAfter }, queue.stopAfter && "text-tint"]}
    title={queue.stopAfter ? "Keep playing after this" : "Stop after this"}
    onclick={() => send({ kind: "stopAfter", on: !queue.stopAfter })}><CircleStop /></Button
  >
</header>

<div class="list relative min-h-0 flex-1 overflow-y-auto p-1" bind:this={list}>
  {#each queue.items as { uid, item }, i (uid)}
    {@const [t, s] = plex.playingLines(item)}
    {@const playing = i === current}
    {@const last = i === queue.items.length - 1}
    <div
      class={[
        "row group/row",
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
      <div class={["bg-muted h-9 flex-none overflow-hidden rounded-sm [&_img]:size-full [&_img]:object-cover", plex.shapeFor(item) === "square" ? "w-9" : "w-6"]}>
        <Img src={img(plex.poster(item) ?? item.thumb, 96, 96)} />
      </div>
      <div class="grow">
        <div class={["ellipsis text-[12.5px]", playing && "font-medium"]}>{t}</div>
        <div class="ellipsis text-muted-foreground text-xs">{s}</div>
      </div>
      <div class="num text-muted-foreground flex flex-none items-center gap-0.5 text-xs">
        {#if playing}
          <span class="flex pr-1.5 text-tint"><AudioLines class="size-4" /></span>
        {:else}
          {#if item.duration}
            <span>{plex.kind(item) === "track" ? clock(item.duration) : duration(item.duration)}</span>
          {/if}
          <Button
            variant="ghost"
            size="icon-xs"
            class="text-muted-foreground opacity-0 group-hover/row:opacity-100 focus-visible:opacity-100"
            title="Remove from queue"
            onpointerdown={(e) => e.stopPropagation()}
            onclick={(e) => {
              e.stopPropagation();
              send({ kind: "remove", uid });
            }}><X /></Button
          >
        {/if}
      </div>
    </div>
  {:else}
    <Empty.Root class="border-0 p-6">
      <Empty.Description>The queue is empty.</Empty.Description>
    </Empty.Root>
  {/each}
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 48px;
    padding: 0 6px 0 8px;
    border-radius: var(--radius-md);
    touch-action: none;
    cursor: pointer;
  }
  .row:hover {
    background: color-mix(in srgb, var(--muted) 60%, transparent);
  }
  .row.current {
    background: var(--muted);
    cursor: default;
  }
  .row.past {
    opacity: 0.45;
  }
  .row.dragging {
    opacity: 0.3;
  }
  .row.drop-above {
    box-shadow: inset 0 1px 0 var(--tint);
  }
  .row.drop-below {
    box-shadow: inset 0 -1px 0 var(--tint);
  }
</style>
