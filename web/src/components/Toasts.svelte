<script lang="ts">
  import { app } from "../lib/state.svelte";
</script>

<div class="toasts" aria-live="polite">
  {#each app.toasts as t (t.id)}
    <div class={["toast", { error: t.error }]}>
      <span>{t.text}</span>
      {#if t.undo !== undefined}
        {@const entry = t.undo}
        <button class="btn sm undo" onclick={() => app.undo(entry)}>Undo</button>
      {/if}
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 20px;
    bottom: 76px;
    z-index: 70;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 8px;
    pointer-events: none;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 12px;
    max-width: 420px;
    padding: 9px 14px;
    border-radius: 5px;
    border: 1px solid var(--line-2);
    background: #1a1a1a;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
    font-size: 12.5px;
    pointer-events: auto;
    animation: rise 0.2s var(--ease);
    user-select: text;
    -webkit-user-select: text;
  }
  .undo {
    flex: none;
  }
  .toast.error {
    border-color: color-mix(in srgb, var(--danger) 40%, transparent);
    color: #f0c4be;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }
</style>
