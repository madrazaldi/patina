<script lang="ts">
  // A titled, horizontally scrolling row of cards.
  import { ChevronLeft, ChevronRight } from "@lucide/svelte";
  import Card from "./Card.svelte";
  import { commonShape, type Shape } from "../lib/plex";
  import { nav, type Route } from "../lib/router.svelte";
  import type { Metadata } from "../lib/types";

  let {
    title,
    items,
    shape,
    useArt = false,
    more,
  }: { title: string; items: Metadata[]; shape?: Shape; useArt?: boolean; more?: Route } = $props();

  let rowShape = $derived(shape ?? commonShape(items));

  let scroller: HTMLDivElement | undefined = $state();
  let atStart = $state(true);
  let atEnd = $state(false);

  function update() {
    if (!scroller) return;
    atStart = scroller.scrollLeft <= 2;
    atEnd = scroller.scrollLeft + scroller.clientWidth >= scroller.scrollWidth - 2;
  }

  function page(dir: number) {
    scroller?.scrollBy({ left: dir * scroller.clientWidth * 0.8, behavior: "smooth" });
  }

  $effect(() => {
    items;
    requestAnimationFrame(update);
  });
</script>

{#if items.length}
  <section>
    <div class="section-head">
      <h2>{title}</h2>
      <div class="row-flex controls">
        {#if more}
          <button class="more" onclick={() => more && nav.push(more)}>See all</button>
        {/if}
        {#if !(atStart && atEnd)}
          <button class="icon-btn" title="Scroll left" disabled={atStart} onclick={() => page(-1)}><ChevronLeft /></button>
          <button class="icon-btn" title="Scroll right" disabled={atEnd} onclick={() => page(1)}><ChevronRight /></button>
        {/if}
      </div>
    </div>
    <div class="row" bind:this={scroller} onscroll={update}>
      {#each items as item, i (`${item.ratingKey}:${item.key}:${i}`)}
        <Card {item} shape={rowShape} {useArt} fixed />
      {/each}
    </div>
  </section>
{/if}

<svelte:window onresize={update} />

<style>
  .controls {
    gap: 2px;
    margin: -8px -6px -6px 0;
  }
  .controls .more {
    margin-right: 10px;
  }
  .controls .icon-btn {
    width: 24px;
    height: 24px;
  }
  .row {
    display: flex;
    gap: 18px;
    overflow-x: auto;
    overflow-y: hidden;
    margin: 0 calc(var(--gutter) * -1);
    padding: 0 var(--gutter) 4px;
    scroll-padding: 0 var(--gutter);
    scrollbar-width: none;
  }
  .row::-webkit-scrollbar {
    display: none;
  }
</style>
