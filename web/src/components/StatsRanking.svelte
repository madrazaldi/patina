<script lang="ts">
  // Ranked list with a thin bar per row, scaled to the largest entry.
  import { ChartNoAxesColumn } from "@lucide/svelte";
  import * as Empty from "$lib/components/ui/empty";
  import { bytes } from "../lib/format";

  let { rows, limit = 5, empty = "Nothing yet." }: { rows: { name: string; bytes: number }[]; limit?: number; empty?: string } = $props();

  let top = $derived([...rows].sort((a, b) => b.bytes - a.bytes).slice(0, limit));
  let max = $derived(Math.max(1, ...top.map((r) => r.bytes)));
  let total = $derived(rows.reduce((n, r) => n + r.bytes, 0));
</script>

{#if top.length === 0}
  <Empty.Root class="border-0 p-4">
    <Empty.Header>
      <Empty.Media variant="icon"><ChartNoAxesColumn /></Empty.Media>
      <Empty.Description>{empty}</Empty.Description>
    </Empty.Header>
  </Empty.Root>
{:else}
  <ol class="m-0 flex list-none flex-col gap-3.5 p-0">
    {#each top as r, i (`${r.name}:${i}`)}
      <li title="{Math.round((r.bytes / Math.max(1, total)) * 100)}% of all data">
        <div class="mb-1.5 flex justify-between gap-3 text-sm">
          <span class="ellipsis font-medium">{r.name}</span>
          <span class="text-muted-foreground flex-none text-xs tabular-nums">{bytes(r.bytes)}</span>
        </div>
        <div class="bg-muted h-1.5 overflow-hidden rounded-full">
          <i class="bg-tint block h-full min-w-0.5 rounded-full" style:width="{(r.bytes / max) * 100}%"></i>
        </div>
      </li>
    {/each}
  </ol>
{/if}
