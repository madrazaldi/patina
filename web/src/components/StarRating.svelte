<script lang="ts">
  // Your rating, 1–5 stars (Plex stores 0–10). Clicking the current value clears it.
  import { Star } from "@lucide/svelte";

  let { value, onrate }: { value: number; onrate: (v: number | null) => void } = $props();
  let hover = $state<number | null>(null);
  let stars = $derived(Math.round(value / 2));
  let shown = $derived(hover ?? stars);
</script>

<div class="stars" title="Your rating" role="group" onmouseleave={() => (hover = null)}>
  {#each [1, 2, 3, 4, 5] as n (n)}
    <button
      class={{ on: n <= shown }}
      aria-label="{n} star{n === 1 ? '' : 's'}"
      onmouseenter={() => (hover = n)}
      onclick={() => onrate(n === stars ? null : n * 2)}
    >
      <Star size={15} fill={n <= shown ? "currentColor" : "none"} />
    </button>
  {/each}
</div>

<style>
  .stars {
    display: flex;
    margin-left: 6px;
  }
  button {
    display: grid;
    place-items: center;
    width: 22px;
    height: 30px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-3);
    cursor: pointer;
  }
  button.on {
    color: var(--accent);
  }
</style>
