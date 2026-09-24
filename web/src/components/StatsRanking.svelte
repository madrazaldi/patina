<script lang="ts">
  // Ranked list with a thin bar per row, scaled to the largest entry.
  import { bytes } from "../lib/format";

  let { rows, limit = 5, empty = "Nothing yet." }: { rows: { name: string; bytes: number }[]; limit?: number; empty?: string } = $props();

  let top = $derived([...rows].sort((a, b) => b.bytes - a.bytes).slice(0, limit));
  let max = $derived(Math.max(1, ...top.map((r) => r.bytes)));
  let total = $derived(rows.reduce((n, r) => n + r.bytes, 0));
</script>

{#if top.length === 0}
  <p class="faint">{empty}</p>
{:else}
  <ol>
    {#each top as r, i (`${r.name}:${i}`)}
      <li title="{Math.round((r.bytes / Math.max(1, total)) * 100)}% of all data">
        <div class="line">
          <span class="ellipsis">{r.name}</span>
          <span class="num faint">{bytes(r.bytes)}</span>
        </div>
        <div class="track"><i style:width="{(r.bytes / max) * 100}%"></i></div>
      </li>
    {/each}
  </ol>
{/if}

<style>
  ol {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    padding: 7px 0;
  }
  .line {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 6px;
  }
  .line .num {
    flex: none;
    font-size: 12px;
  }
  .track i {
    display: block;
    min-width: 2px;
    height: 4px;
    border-radius: 0 2px 2px 0;
    background: var(--accent);
  }
</style>
