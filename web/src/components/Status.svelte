<script lang="ts">
  // Loading, empty and error placeholders.
  let {
    loading = false,
    error,
    title,
    text,
    fill = false,
  }: { loading?: boolean; error?: string; title?: string; text?: string; fill?: boolean } = $props();
</script>

<div class={["status", { fill }]}>
  {#if loading}
    <div class="spinner"></div>
  {:else if error}
    <h3>Something went wrong</h3>
    <div class="error-box">{error}</div>
  {:else}
    {#if title}<h3>{title}</h3>{/if}
    {#if text}<p>{text}</p>{/if}
  {/if}
</div>

<style>
  .status {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    min-height: 180px;
    padding: 40px 20px;
    text-align: center;
  }
  .status.fill {
    min-height: 60vh;
  }
  h3 {
    font: 400 20px/1.2 var(--serif);
  }
  p {
    max-width: 44ch;
    color: var(--text-2);
  }
  .error-box {
    max-width: 560px;
    text-align: left;
  }
</style>
