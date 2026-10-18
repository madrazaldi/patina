<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  import { Skeleton } from "$lib/components/ui/skeleton";
  import Card from "./Card.svelte";
  import type { Metadata } from "../lib/types";
  import type { Shape } from "../lib/plex";
  let { total, at, load, shape = "poster", selecting = false, selected, toggle, onfocus, restoreFocus }: {
    total: number; at: (index: number) => Metadata | undefined; load: (start: number, end: number) => void;
    shape?: Shape; selecting?: boolean; selected: Set<string>; toggle: (item: Metadata) => void;
    onfocus: (index: number) => void; restoreFocus?: number;
  } = $props();
  let root: HTMLDivElement;
  let columns = $state(5), rowHeight = $state(300), start = $state(0), end = $state(40);
  let scroller: HTMLElement | null = null;
  let pendingFocus = untrack(() => restoreFocus);
  const gap = 20;
  function measure() {
    if (!root || !scroller) return;
    const width = root.clientWidth;
    const poster = parseFloat(getComputedStyle(root).getPropertyValue("--poster")) || 160;
    columns = Math.max(1, Math.floor((width + gap) / ((shape === "landscape" ? poster * 1.75 : poster) + gap)));
    const cellWidth = (width - (columns - 1) * gap) / columns;
    rowHeight = cellWidth * (shape === "poster" ? 1.5 : shape === "landscape" ? 9 / 16 : 1) + 64;
    const top = Math.max(0, scroller.getBoundingClientRect().top - root.getBoundingClientRect().top);
    start = Math.max(0, Math.floor(top / rowHeight) - 2) * columns;
    end = Math.min(total, (Math.ceil((top + scroller.clientHeight) / rowHeight) + 2) * columns);
  }
  export async function jump(index: number, onlyIfHidden = false) {
    if (!scroller || !root) return;
    const top = root.getBoundingClientRect().top - scroller.getBoundingClientRect().top + scroller.scrollTop;
    const rowTop = top + Math.floor(index / columns) * rowHeight;
    const rowBottom = rowTop + rowHeight;
    if (!onlyIfHidden || rowTop < scroller.scrollTop) scroller.scrollTop = rowTop;
    else if (rowBottom > scroller.scrollTop + scroller.clientHeight) {
      scroller.scrollTop = Math.min(rowTop, rowBottom - scroller.clientHeight);
    }
    measure();
    await tick();
  }
  onMount(() => {
    scroller = document.getElementById("content");
    const observer = new ResizeObserver(measure);
    observer.observe(root);
    if (scroller) observer.observe(scroller);
    scroller?.addEventListener("scroll", measure, { passive: true });
    measure();
    return () => { observer.disconnect(); scroller?.removeEventListener("scroll", measure); };
  });
  $effect(() => { total; shape; untrack(measure); });
  $effect(() => { load(start, end); });
  let visible = $derived(Array.from({ length: Math.max(0, Math.min(total, end) - start) }, (_, n) => ({ index: start + n, item: at(start + n) })));
  $effect(() => {
    visible;
    if (pendingFocus !== undefined) {
      const index = pendingFocus;
      tick().then(() => {
        const card = root?.querySelector<HTMLElement>(`[data-index="${index}"] .card`);
        if (card) { card.focus({ preventScroll: true }); pendingFocus = undefined; }
      });
    }
  });
  async function key(e: KeyboardEvent, index: number) {
    if (e.target !== (e.currentTarget as HTMLElement).querySelector(".card")) return;
    const delta = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : e.key === "ArrowDown" ? columns : e.key === "ArrowUp" ? -columns : 0;
    if (!delta) return;
    e.preventDefault();
    const next = Math.max(0, Math.min(total - 1, index + delta));
    pendingFocus = next;
    await jump(next, true);
    root.querySelector<HTMLElement>(`[data-index="${next}"] .card`)?.focus({ preventScroll: true });
  }
</script>

<div class="virtual-grid" bind:this={root} style:height="{Math.ceil(total / columns) * rowHeight}px" aria-label="Library titles">
  <div class="window" style:top="{Math.floor(start / columns) * rowHeight}px" style:grid-template-columns="repeat({columns}, minmax(0, 1fr))" style:grid-auto-rows="{rowHeight}px">
    {#each visible as { index, item } (index)}
      <!-- Keyboard arrows move focus between cards in this virtualized group. -->
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <div data-index={index} onfocusin={() => onfocus(index)} onkeydown={(e) => key(e, index)} role="group" aria-label={`Item ${index + 1} of ${total}`}>
        {#if item}
          <Card {item} {shape} {selecting} selected={selected.has(item.ratingKey ?? "")} ontoggle={() => toggle(item)} />
        {:else}
          <Skeleton class="h-[calc(100%-64px)] rounded-lg" aria-label="Loading title" />
        {/if}
      </div>
    {/each}
  </div>
</div>
<style>
  .virtual-grid { position: relative; min-height: 100px; }
  .window { position: absolute; left: 0; right: 0; display: grid; column-gap: 20px; }
</style>
