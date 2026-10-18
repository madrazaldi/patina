<script lang="ts">
  // "Add to collection" dialog, shown while `app.collectionTarget` is set.
  import { Check, Plus } from "@lucide/svelte";
  import * as Alert from "$lib/components/ui/alert";
  import { Button } from "$lib/components/ui/button";
  import * as Empty from "$lib/components/ui/empty";
  import { Input } from "$lib/components/ui/input";
  import * as Item from "$lib/components/ui/item";
  import { Spinner } from "$lib/components/ui/spinner";
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
      <Alert.Root variant="destructive"><Alert.Description class="whitespace-pre-wrap select-text">{error}</Alert.Description></Alert.Root>
    {:else if lists === null}
      <div class="flex justify-center py-6"><Spinner class="text-muted-foreground" /></div>
    {:else if lists.length === 0}
      <Empty.Root class="border border-dashed p-6">
        <Empty.Header>
          <Empty.Title>No collections yet</Empty.Title>
          <Empty.Description>This library has no collections yet.</Empty.Description>
        </Empty.Header>
      </Empty.Root>
    {:else}
      <Item.Group role="group" aria-label="Collections" class="max-h-[300px] overflow-auto has-data-[size=sm]:gap-1">
        {#each lists as c (c.ratingKey)}
          {@const has = member.includes(c.title ?? "")}
          <Item.Root size="sm" class="hover:bg-muted focus-visible:bg-muted cursor-pointer text-left disabled:cursor-default disabled:opacity-55 disabled:hover:bg-transparent">
            {#snippet child({ props })}
              <button type="button" {...props} disabled={has} onclick={() => add({ collection: c.ratingKey }, `Added to “${c.title}”`)}>
                <Item.Media variant="image" class="h-12! w-8!">
                  <Img src={img(c.thumb ?? c.composite, 96, 144)} />
                </Item.Media>
                <Item.Content>
                  <Item.Title>{c.title}</Item.Title>
                  <Item.Description>{has ? "Already in this collection" : plex.subtitle(c)}</Item.Description>
                </Item.Content>
                <Item.Actions>
                  {#if has}<Check class="text-muted-foreground size-4" />{:else}<Plus class="text-muted-foreground size-4" />{/if}
                </Item.Actions>
              </button>
            {/snippet}
          </Item.Root>
        {/each}
      </Item.Group>
    {/if}

    <div class="mt-4 flex gap-2">
      <Input
        class="flex-1"
        placeholder="New collection"
        bind:value={name}
        onkeydown={(e) => e.key === "Enter" && create()}
      />
      <Button variant="outline" disabled={!name.trim()} onclick={create}>Create</Button>
    </div>
    <div class="foot"><Button variant="outline" onclick={close}>Cancel</Button></div>
  </dialog>
{/if}
