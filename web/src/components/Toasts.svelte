<script lang="ts">
  import { CircleAlert, CircleCheck } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button";
  import { app } from "../lib/state.svelte";
</script>

<div class="toasts pointer-events-none fixed right-5 bottom-[76px] z-70 flex flex-col items-end gap-2" aria-live="polite">
  {#each app.toasts as t (t.id)}
    <div
      class={[
        "toast bg-popover text-popover-foreground animate-in fade-in-0 slide-in-from-bottom-2 pointer-events-auto flex max-w-[420px] items-center gap-3 rounded-lg border p-3 text-sm shadow-lg duration-200 select-text",
        { error: t.error },
      ]}
    >
      {#if t.error}
        <CircleAlert class="text-destructive size-4 shrink-0" aria-hidden="true" />
      {:else}
        <CircleCheck class="text-tint size-4 shrink-0" aria-hidden="true" />
      {/if}
      <span class={["min-w-0 flex-1", { "text-destructive": t.error }]}>{t.text}</span>
      {#if t.undo !== undefined}
        {@const entry = t.undo}
        <Button variant="outline" size="sm" class="undo" onclick={() => app.undo(entry)}>Undo</Button>
      {/if}
    </div>
  {/each}
</div>
