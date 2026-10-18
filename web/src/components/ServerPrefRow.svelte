<script lang="ts">
  // One server or library setting: a switch, a select, or text with Save.
  // `onsave` resolves to whether the new value was stored.
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import * as NativeSelect from "$lib/components/ui/native-select";
  import { Switch } from "$lib/components/ui/switch";
  import type { Pref } from "../lib/types";

  let { pref, onsave, showId = true }: { pref: Pref; onsave: (value: string) => Promise<boolean>; showId?: boolean } = $props();

  /** Unsaved text edit. */
  let edit = $state<string>();

  const on = (v: string) => v === "true" || v === "1";

  async function save(value: string) {
    if (await onsave(value)) edit = undefined;
  }
</script>

<div class="grid grid-cols-[minmax(0,1fr)_minmax(180px,320px)] items-center gap-6 px-4 py-3.5">
  <div class="min-w-0">
    <div class="text-sm font-medium">
      {pref.label}
      {#if showId}<code class="text-muted-foreground ml-2 font-mono text-[11px] font-normal">{pref.id}</code>{/if}
    </div>
    {#if pref.summary}<p class="text-muted-foreground mt-0.5 text-[13px] leading-snug">{pref.summary}</p>{/if}
  </div>
  <div class="flex items-center justify-end gap-2">
    {#if pref.kind === "bool"}
      <Switch aria-label={pref.label} bind:checked={() => on(pref.value), (v) => void save(v ? "true" : "false")} />
    {:else if pref.options.length}
      <NativeSelect.Root class="w-full" value={pref.value} onchange={(e) => save(e.currentTarget.value)}>
        {#each pref.options as [v, l] (v)}<NativeSelect.Option value={v}>{l}</NativeSelect.Option>{/each}
      </NativeSelect.Root>
    {:else}
      <Input
        value={edit ?? pref.value}
        oninput={(e) => (edit = e.currentTarget.value)}
        onkeydown={(e) => e.key === "Enter" && edit !== undefined && save(edit)}
      />
      {#if edit !== undefined && edit !== pref.value}
        <Button size="sm" onclick={() => edit !== undefined && save(edit)}>Save</Button>
      {/if}
    {/if}
  </div>
</div>
