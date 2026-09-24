<script lang="ts">
  // One server or library setting: a toggle, a select, or text with Save.
  // `onsave` resolves to whether the new value was stored.
  import type { Pref } from "../lib/types";

  let { pref, onsave, showId = true }: { pref: Pref; onsave: (value: string) => Promise<boolean>; showId?: boolean } = $props();

  /** Unsaved text edit. */
  let edit = $state<string>();

  const on = (v: string) => v === "true" || v === "1";

  async function save(value: string) {
    if (await onsave(value)) edit = undefined;
  }
</script>

<div class="pref">
  <div class="label">
    <b>{pref.label}</b>
    {#if showId}<code>{pref.id}</code>{/if}
    {#if pref.summary}<span>{pref.summary}</span>{/if}
  </div>
  <div class="control">
    {#if pref.kind === "bool"}
      <button class={["toggle", { on: on(pref.value) }]} aria-label={pref.label} onclick={() => save(on(pref.value) ? "false" : "true")}></button>
    {:else if pref.options.length}
      <select class="select" value={pref.value} onchange={(e) => save(e.currentTarget.value)}>
        {#each pref.options as [v, l] (v)}<option value={v}>{l}</option>{/each}
      </select>
    {:else}
      <input
        class="input"
        value={edit ?? pref.value}
        oninput={(e) => (edit = e.currentTarget.value)}
        onkeydown={(e) => e.key === "Enter" && edit !== undefined && save(edit)}
      />
      {#if edit !== undefined && edit !== pref.value}
        <button class="btn sm primary" onclick={() => edit !== undefined && save(edit)}>Save</button>
      {/if}
    {/if}
  </div>
</div>

<style>
  .pref {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(180px, 320px);
    gap: 24px;
    align-items: center;
    padding: 12px 0;
  }
  b {
    font-weight: 500;
  }
  code {
    margin-left: 8px;
    font: 11px var(--mono);
    color: var(--text-3);
  }
  span {
    display: block;
    margin-top: 3px;
    font-size: 12px;
    color: var(--text-3);
  }
  .control {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .input,
  .select {
    width: 100%;
  }
</style>
