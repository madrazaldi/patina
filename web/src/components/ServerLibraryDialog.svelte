<script lang="ts">
  // Add a library, or rename one and change its folders when `section` is set.
  import { Plus, X } from "@lucide/svelte";
  import { untrack } from "svelte";
  import * as Alert from "$lib/components/ui/alert";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import * as NativeSelect from "$lib/components/ui/native-select";
  import { modal } from "../lib/dialog";
  import { api } from "../lib/api";
  import { act, captureScope, message } from "../lib/state.svelte";
  import type { Directory, LibraryChoices } from "../lib/types";

  let { section, onclose }: { section?: Directory; onclose: () => void } = $props();

  const KINDS: [string, string][] = [
    ["movie", "Movies"],
    ["show", "TV Shows"],
    ["artist", "Music"],
    ["photo", "Photos"],
  ];

  const initial = untrack(() => section);
  // Library ids and folders belong to the server this opened on; never send them to another one.
  const guard = captureScope();
  let name = $state(initial?.title ?? "");
  let locations = $state(initial?.Location.map((l) => l.path ?? "").filter(Boolean) ?? []);
  if (locations.length === 0) locations.push("");
  let kind = $state("movie");
  let agent = $state("");
  let scanner = $state("");
  let language = $state("en-US");
  let choices = $state<LibraryChoices | null>(null);
  let choicesError = $state<string>();
  let busy = $state(false);

  let label = $derived(KINDS.find(([k]) => k === kind)?.[1] ?? "Library");
  let paths = $derived(locations.map((l) => l.trim()).filter(Boolean));

  // Agents and scanners depend on the library type; preselect the first of each.
  $effect(() => {
    if (initial) return;
    const k = kind;
    choices = null;
    choicesError = undefined;
    api.libraryChoices(k).then(
      (c) => {
        if (untrack(() => kind) !== k || !guard.valid()) return;
        choices = c;
        agent = c.agents[0]?.[0] ?? "";
        scanner = c.scanners[0] ?? "";
      },
      (e) => untrack(() => kind) === k && guard.valid() && (choicesError = message(e)),
    );
  });

  async function submit() {
    if (busy || paths.length === 0) return;
    if (!guard.valid()) return onclose();
    const title = name.trim() || label;
    busy = true;
    const ok = initial
      ? await act(api.editLibrary(initial.key ?? "", title, paths), "Library updated")
      : await act(
          api.createLibrary({ name: title, kind, agent, scanner, language: language.trim() || "en-US", locations: paths }),
          `Added “${title}”`,
        );
    busy = false;
    if (ok) onclose();
  }
</script>

<dialog class="modal wide" use:modal={onclose} aria-labelledby="library-edit-title">
  <h2 id="library-edit-title">{initial ? `Edit ${initial.title}` : "Add library"}</h2>
  <p class="desc">{initial ? "Rename the library or change the folders it scans." : "Point Plex at folders of media on the server."}</p>

  <div class="grid grid-cols-2 gap-x-4 gap-y-3.5">
    <div class="col-span-2 flex flex-col gap-2">
      <Label for="library-name">Name</Label>
      <Input id="library-name" data-initial-focus placeholder={initial ? "" : label} bind:value={name} />
    </div>
    {#if !initial}
      <div class="flex flex-col gap-2">
        <Label for="library-type">Type</Label>
        <NativeSelect.Root id="library-type" class="w-full" bind:value={kind}>
          {#each KINDS as [k, l] (k)}<NativeSelect.Option value={k}>{l}</NativeSelect.Option>{/each}
        </NativeSelect.Root>
      </div>
      <div class="flex flex-col gap-2">
        <Label for="library-agent">Agent</Label>
        <NativeSelect.Root id="library-agent" class="w-full" bind:value={agent} disabled={!choices?.agents.length}>
          {#each choices?.agents ?? [] as [id, title] (id)}<NativeSelect.Option value={id}>{title || id}</NativeSelect.Option>{/each}
        </NativeSelect.Root>
      </div>
      <div class="flex flex-col gap-2">
        <Label for="library-scanner">Scanner</Label>
        <NativeSelect.Root id="library-scanner" class="w-full" bind:value={scanner} disabled={!choices?.scanners.length}>
          {#each choices?.scanners ?? [] as s (s)}<NativeSelect.Option value={s}>{s}</NativeSelect.Option>{/each}
        </NativeSelect.Root>
      </div>
      <div class="flex flex-col gap-2">
        <Label for="library-language">Language</Label>
        <Input id="library-language" placeholder="en-US" bind:value={language} />
      </div>
      {#if choicesError}
        <Alert.Root variant="destructive" class="col-span-2"><Alert.Description class="whitespace-pre-wrap select-text">{choicesError}</Alert.Description></Alert.Root>
      {/if}
    {/if}
  </div>

  <div class="mt-5">
    <div class="mb-2 flex items-baseline justify-between gap-3">
      <Label>Folders</Label>
      <span class="text-muted-foreground text-xs">Paths as the server sees them, e.g. /data/movies</span>
    </div>
    {#each locations, i}
      <div class="mb-1.5 flex items-center gap-1">
        <Input class="flex-1" placeholder="/path/to/media" bind:value={locations[i]} onkeydown={(e) => e.key === "Enter" && submit()} />
        <Button variant="ghost" size="icon" title="Remove folder" disabled={locations.length === 1} onclick={() => locations.splice(i, 1)}>
          <X />
        </Button>
      </div>
    {/each}
    <Button variant="ghost" size="sm" class="-ml-1.5" onclick={() => locations.push("")}><Plus />Add folder</Button>
  </div>

  <div class="foot">
    <Button variant="ghost" onclick={onclose}>Cancel</Button>
    <Button disabled={busy || paths.length === 0 || (!initial && !choices)} onclick={submit}>
      {initial ? "Save" : "Add library"}
    </Button>
  </div>
</dialog>
