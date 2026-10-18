<script lang="ts">
  // A titled, horizontally scrolling row of cards.
  import { ChevronLeft, ChevronRight } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button";
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
      <div class="flex items-center gap-1.5">
        {#if more}
          <Button variant="ghost" size="sm" class="text-muted-foreground" onclick={() => more && nav.push(more)}>See all</Button>
        {/if}
        {#if !(atStart && atEnd)}
          <Button variant="outline" size="icon-sm" title="Scroll left" disabled={atStart} onclick={() => page(-1)}><ChevronLeft /></Button>
          <Button variant="outline" size="icon-sm" title="Scroll right" disabled={atEnd} onclick={() => page(1)}><ChevronRight /></Button>
        {/if}
      </div>
    </div>
    <div class="row mx-[calc(var(--gutter)*-1)] flex gap-5 overflow-x-auto overflow-y-hidden px-(--gutter) pb-1 [scroll-padding:0_var(--gutter)] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden" bind:this={scroller} onscroll={update}>
      {#each items as item, i (`${item.ratingKey}:${item.key}:${i}`)}
        <Card {item} shape={rowShape} {useArt} fixed />
      {/each}
    </div>
  </section>
{/if}

<svelte:window onresize={update} />
