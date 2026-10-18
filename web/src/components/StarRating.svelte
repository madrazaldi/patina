<script lang="ts">
  // Your rating, 1–5 stars (Plex stores 0–10). Clicking the current value clears it.
  import { Star } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button";

  let { value, onrate }: { value: number; onrate: (v: number | null) => void } = $props();
  let hover = $state<number | null>(null);
  let stars = $derived(Math.round(value / 2));
  let shown = $derived(hover ?? stars);
</script>

<div class="flex items-center" title="Your rating" role="group" onmouseleave={() => (hover = null)}>
  {#each [1, 2, 3, 4, 5] as n (n)}
    <Button
      variant="ghost"
      size="icon-sm"
      class={["size-6 rounded-md hover:bg-transparent dark:hover:bg-transparent", n <= shown ? "on text-tint hover:text-tint" : "text-muted-foreground/60 hover:text-tint"]}
      aria-label="{n} star{n === 1 ? '' : 's'}"
      onmouseenter={() => (hover = n)}
      onclick={() => onrate(n === stars ? null : n * 2)}
    >
      <Star class="size-4" fill={n <= shown ? "currentColor" : "none"} />
    </Button>
  {/each}
</div>
