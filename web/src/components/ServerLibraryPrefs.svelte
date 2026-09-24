<script lang="ts">
  // Advanced settings of one library, rendered like the server settings.
  import ServerPrefRow from "./ServerPrefRow.svelte";
  import Status from "./Status.svelte";
  import { api } from "../lib/api";
  import { resource } from "../lib/resource.svelte";
  import { act } from "../lib/state.svelte";
  import type { Directory, Pref } from "../lib/types";

  let { section, onclose }: { section: Directory; onclose: () => void } = $props();

  const id = $derived(section.key ?? "");
  const prefs = resource(() => api.libraryPrefs(id));
  let advanced = $state(false);
  let hasAdvanced = $derived((prefs.data ?? []).some((p) => p.advanced));
  let shown = $derived((prefs.data ?? []).filter((p) => advanced || !p.advanced));

  async function save(p: Pref, value: string) {
    const ok = await act(api.setLibraryPref(id, p.id, value), "Setting saved");
    if (ok) p.value = value;
    return ok;
  }
</script>

<div class="modal-backdrop" role="presentation" onclick={onclose}>
  <div class="modal wide" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.key === "Escape" && onclose()}>
    <div class="head">
      <div>
        <h2>{section.title} settings</h2>
        <p class="desc">Changes are saved as you make them.</p>
      </div>
      {#if hasAdvanced}
        <button class={["chip", { active: advanced }]} onclick={() => (advanced = !advanced)}>Show advanced</button>
      {/if}
    </div>
    {#if !prefs.data}
      <Status loading={!prefs.error} error={prefs.error} />
    {:else if shown.length === 0}
      <p class="faint">This library has no settings.</p>
    {:else}
      <div class="rows">
        {#each shown as p (p.id)}
          <ServerPrefRow pref={p} showId={false} onsave={(v) => save(p, v)} />
        {/each}
      </div>
    {/if}
    <div class="foot"><button class="btn quiet" onclick={onclose}>Done</button></div>
  </div>
</div>

<style>
  .modal.wide {
    width: min(760px, calc(100vw - 48px));
  }
  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
  }
</style>
