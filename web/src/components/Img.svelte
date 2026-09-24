<script lang="ts">
  // Artwork that fades in once decoded.
  let { src, alt = "", class: klass = "" }: { src?: string; alt?: string; class?: string } = $props();
  let loaded = $state(false);
</script>

{#if src}
  <img
    {src}
    {alt}
    class={[klass, { loaded }]}
    loading="lazy"
    decoding="async"
    draggable="false"
    onload={() => (loaded = true)}
  />
{/if}

<style>
  img {
    opacity: 0;
    transition: opacity 0.3s var(--ease);
  }
  img.loaded {
    opacity: 1;
  }
</style>
