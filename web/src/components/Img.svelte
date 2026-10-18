<script lang="ts">
  // Artwork that fades in once decoded.
  import { cn } from "$lib/utils";

  let { src, alt = "", class: klass = "" }: { src?: string; alt?: string; class?: string } = $props();
  let loaded = $state(false);
</script>

{#if src}
  <img
    {src}
    {alt}
    class={cn("transition-[opacity,filter] duration-300", loaded ? "opacity-100" : "opacity-0", klass)}
    loading="lazy"
    decoding="async"
    draggable="false"
    onload={() => (loaded = true)}
  />
{/if}
