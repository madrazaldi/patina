<script lang="ts">
  // Advanced settings of one library, rendered like the server settings.
  import { Settings2 } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button";
  import * as Empty from "$lib/components/ui/empty";
  import { Label } from "$lib/components/ui/label";
  import { Switch } from "$lib/components/ui/switch";
  import ServerPrefRow from "./ServerPrefRow.svelte";
  import Status from "./Status.svelte";
  import { modal } from "../lib/dialog";
  import { api } from "../lib/api";
  import { resource } from "../lib/resource.svelte";
  import { act, captureScope } from "../lib/state.svelte";
  import type { Directory, Pref } from "../lib/types";

  let { section, onclose }: { section: Directory; onclose: () => void } = $props();

  const id = $derived(section.key ?? "");
  const guard = captureScope();
  const prefs = resource(() => api.libraryPrefs(id));
  let advanced = $state(false);
  let hasAdvanced = $derived((prefs.data ?? []).some((p) => p.advanced));
  let shown = $derived((prefs.data ?? []).filter((p) => advanced || !p.advanced));

  async function save(p: Pref, value: string) {
    if (!guard.valid()) { onclose(); return false; }
    const ok = await act(api.setLibraryPref(id, p.id, value), "Setting saved");
    if (ok) p.value = value;
    return ok;
  }
</script>

<dialog class="modal wide w-[min(760px,calc(100vw-48px))]" use:modal={onclose} aria-labelledby="library-prefs-title">
  <div class="flex items-start justify-between gap-4">
    <div>
      <h2 id="library-prefs-title">{section.title} settings</h2>
      <p class="desc">Changes are saved as you make them.</p>
    </div>
    {#if hasAdvanced}
      <div class="flex items-center gap-2 pt-1">
        <Switch id="library-prefs-advanced" size="sm" bind:checked={advanced} />
        <Label for="library-prefs-advanced" class="text-muted-foreground font-normal">Show advanced</Label>
      </div>
    {/if}
  </div>
  {#if !prefs.data}
    <Status loading={!prefs.error} error={prefs.error} />
  {:else if shown.length === 0}
    <Empty.Root class="border">
      <Empty.Header>
        <Empty.Media variant="icon"><Settings2 /></Empty.Media>
        <Empty.Title>No settings</Empty.Title>
        <Empty.Description>This library has no settings.</Empty.Description>
      </Empty.Header>
    </Empty.Root>
  {:else}
    <div class="mt-4 max-h-[60vh] divide-y overflow-auto rounded-lg border">
      {#each shown as p (p.id)}
        <ServerPrefRow pref={p} showId={false} onsave={(v) => save(p, v)} />
      {/each}
    </div>
  {/if}
  <div class="foot"><Button variant="ghost" onclick={onclose}>Done</Button></div>
</dialog>
