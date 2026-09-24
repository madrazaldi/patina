<script lang="ts">
  // Infinite-scroll sentinel: calls `onmore` when it scrolls near the viewport.
  let { onmore, loading = false }: { onmore: () => void; loading?: boolean } = $props();
  let el: HTMLDivElement | undefined = $state();

  $effect(() => {
    if (!el || loading) return;
    const io = new IntersectionObserver((entries) => entries.some((e) => e.isIntersecting) && onmore(), {
      root: document.getElementById("content"),
      rootMargin: "1200px 0px",
    });
    io.observe(el);
    return () => io.disconnect();
  });
</script>

<div bind:this={el} class="more">
  {#if loading}<div class="spinner"></div>{/if}
</div>

<style>
  .more {
    display: grid;
    place-items: center;
    min-height: 60px;
  }
</style>
