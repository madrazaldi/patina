<script lang="ts">
  // Add a library, or rename one and change its folders when `section` is set.
  import { Plus, X } from "@lucide/svelte";
  import { untrack } from "svelte";
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

  <div class="form">
    <label class="field">
      <span>Name</span>
      <input class="input" data-initial-focus placeholder={initial ? "" : label} bind:value={name} />
    </label>
    {#if !initial}
      <label class="field">
        <span>Type</span>
        <select class="select" bind:value={kind}>
          {#each KINDS as [k, l] (k)}<option value={k}>{l}</option>{/each}
        </select>
      </label>
      <label class="field">
        <span>Agent</span>
        <select class="select" bind:value={agent} disabled={!choices?.agents.length}>
          {#each choices?.agents ?? [] as [id, title] (id)}<option value={id}>{title || id}</option>{/each}
        </select>
      </label>
      <label class="field">
        <span>Scanner</span>
        <select class="select" bind:value={scanner} disabled={!choices?.scanners.length}>
          {#each choices?.scanners ?? [] as s (s)}<option value={s}>{s}</option>{/each}
        </select>
      </label>
      <label class="field">
        <span>Language</span>
        <input class="input" placeholder="en-US" bind:value={language} />
      </label>
      {#if choicesError}<div class="error-box full">{choicesError}</div>{/if}
    {/if}
  </div>

  <div class="folders">
    <div class="field-head">
      <span>Folders</span>
      <span class="faint">Paths as the server sees them, e.g. /data/movies</span>
    </div>
    {#each locations, i}
      <div class="folder">
        <input class="input grow" placeholder="/path/to/media" bind:value={locations[i]} onkeydown={(e) => e.key === "Enter" && submit()} />
        <button class="icon-btn" title="Remove folder" disabled={locations.length === 1} onclick={() => locations.splice(i, 1)}>
          <X size={14} />
        </button>
      </div>
    {/each}
    <button class="btn sm quiet add" onclick={() => locations.push("")}><Plus size={14} />Add folder</button>
  </div>

  <div class="foot">
    <button class="btn quiet" onclick={onclose}>Cancel</button>
    <button class="btn primary" disabled={busy || paths.length === 0 || (!initial && !choices)} onclick={submit}>
      {initial ? "Save" : "Add library"}
    </button>
  </div>
</dialog>

<style>
  .form {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px 16px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .field:first-child,
  .full {
    grid-column: 1 / -1;
  }
  .field > span,
  .field-head > span:first-child {
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .folders {
    margin-top: 18px;
  }
  .field-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 6px;
    font-size: 12px;
  }
  .folder {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-bottom: 6px;
  }
  .add {
    margin-left: -10px;
  }
</style>
