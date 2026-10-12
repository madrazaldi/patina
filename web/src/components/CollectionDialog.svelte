<script lang="ts">
  // "Add to collection" dialog, shown while `app.collectionTarget` is set.
  import { Check, Plus } from "@lucide/svelte";
  import Img from "./Img.svelte";
  import { modal } from "../lib/dialog";
  import { api } from "../lib/api";
  import { img } from "../lib/images";
  import * as plex from "../lib/plex";
  import { act, app, captureScope, message } from "../lib/state.svelte";
  import type { Metadata } from "../lib/types";

  let item = $derived(app.collectionTarget);
  // The account and server the dialog opened on: responses and writes for any other one are dropped.
  let guard = captureScope();
  let lists = $state<Metadata[] | null>(null);
  let error = $state<string>();
  let name = $state("");
  // Titles of collections the item is already in.
  let member = $state<string[]>([]);

  async function load(target: Metadata) {
    // Menu items may be partial; the full item knows its section and collections.
    const full = target.librarySectionID != null && target.Collection.length ? target : await api.metadata(target.ratingKey ?? "");
    const section = full?.librarySectionID ?? target.librarySectionID;
    if (section == null) throw "This item isn't in a library.";
    const found = await api.collections(String(section));
    return { found, member: (full?.Collection ?? []).map((t) => t.tag ?? "") };
  }

  $effect(() => {
    const target = item;
    lists = null;
    error = undefined;
    name = "";
    member = [];
    if (!target) return;
    const opened = (guard = captureScope());
    load(target).then(
      (r) => {
        if (app.collectionTarget !== target || !opened.valid()) return;
        lists = r.found;
        member = r.member;
      },
      (e) => app.collectionTarget === target && opened.valid() && (error = message(e)),
    );
  });

  const close = () => (app.collectionTarget = null);

  function add(target: { collection?: string; title?: string }, ok: string) {
    if (!item || !guard.valid()) return close();
    const rk = item.ratingKey ?? "";
    close();
    act(api.addToCollection(rk, target), ok, true);
  }

  function create() {
    const title = name.trim();
    if (title) add({ title }, `Created collection “${title}”`);
  }
</script>

{#if item}
  <dialog class="modal" use:modal={close} aria-labelledby="collection-title">
    <h2 id="collection-title">Add to collection</h2>
    <p class="desc">{plex.fullTitle(item)}</p>

    {#if error}
      <div class="error-box">{error}</div>
    {:else if lists === null}
      <div class="spinner"></div>
    {:else if lists.length === 0}
      <p class="faint">This library has no collections yet.</p>
    {:else}
      <div class="rows lists">
        {#each lists as c (c.ratingKey)}
          {@const has = member.includes(c.title ?? "")}
          <button
            class="list-row clickable"
            disabled={has}
            onclick={() => add({ collection: c.ratingKey }, `Added to “${c.title}”`)}
          >
            <div class="thumb"><Img src={img(c.thumb ?? c.composite, 96, 144)} /></div>
            <div class="grow">
              <div class="t">{c.title}</div>
              <div class="s">{has ? "Already in this collection" : plex.subtitle(c)}</div>
            </div>
            <div class="end">{#if has}<Check />{:else}<Plus />{/if}</div>
          </button>
        {/each}
      </div>
    {/if}

    <div class="new">
      <input
        class="input grow"
        placeholder="New collection"
        bind:value={name}
        onkeydown={(e) => e.key === "Enter" && create()}
      />
      <button class="btn" disabled={!name.trim()} onclick={create}>Create</button>
    </div>
    <div class="foot"><button class="btn quiet" onclick={close}>Cancel</button></div>
  </dialog>
{/if}

<style>
  .lists {
    max-height: 300px;
    overflow: auto;
  }
  .lists button {
    width: 100%;
    border-left: 0;
    border-right: 0;
    border-bottom: 0;
    background: none;
    text-align: left;
  }
  .lists button:disabled {
    cursor: default;
    opacity: 0.55;
  }
  .new {
    display: flex;
    gap: 8px;
    margin-top: 18px;
  }
</style>
